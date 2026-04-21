# WinVibe MVP-1 设计规范：Hook Server + HUD 单审批闭环

- 文档日期：2026-04-20
- 状态：设计定稿，待实施计划
- 作者：协作 brainstorming 产出
- 范围：MVP-1 垂直切片（Windows-only）

## 0. 文档导览

| 章节 | 主题 |
|---|---|
| §1 | 范围与审批状态机 |
| §2 | 工程结构与依赖矩阵 |
| §3 | 运行时与并发 |
| §4 | 错误、审计、诊断、tracing、trace 透传 |
| §5 | 测试策略 |
| 附录 | 已知限制、术语表 |

本规范是后续 implementation plan 的输入。所有跨 crate 接口签名、稳定 error code、HTTP / IPC payload schema 在本文中定稿；实施计划负责把这些契约转化为分阶段任务清单。

---

## §1 范围与审批状态机

### 1.1 MVP-1 范围（in-scope）

垂直切片，覆盖**单审批闭环**：

1. Hook Server（HTTP，绑定 127.0.0.1）
2. winvibe-hookcli（同步 Rust 二进制，由 Claude Code PreToolUse hook 调用）
3. 最小 HUD（Tauri + React，仅审批卡片与 approve / deny 操作）
4. 单实例守护、graceful shutdown
5. Audit JSONL 日落（终态事实）
6. Diagnostic JSONL 日落（按 approval_id 一文件一事件）
7. tracing 全链路 + W3C trace-id 端到端透传

### 1.2 不在 MVP-1 范围（out-of-scope）

- 自动配置 Claude Code（手动写 `.claude/settings.json`）
- 终端跳转
- 全屏检测
- 系统通知
- 交互式 Q&A
- 用量监控
- Mascot
- 多平台（仅 Windows，Linux / macOS 留待 MVP-2+）
- macOS / Linux runner、UI 视觉回归、性能压测、fuzz、mutation testing
- W3C `tracestate`、OTLP exporter

### 1.3 平台与工具链

- 操作系统：Windows 11
- Rust toolchain：stable（MSRV 在 §2 依赖版本定稿后再冻结）
- Tauri：2.x
- 前端：React + Vite + TypeScript（vitest 做组件测试）
- HTTP server：axum + tokio
- HTTP client (hookcli)：ureq（**禁止**引入 tokio / hyper / reqwest）

### 1.4 hookcli 交付形态与协议假设

- hookcli 由 Claude Code 同步调用（`stdin` 接收 PreToolUse payload，`stdout` 返回 decision）
- hookcli 是阻塞二进制，进程内不引入 tokio runtime
- hookcli 与 Hook Server 走 HTTP `localhost:59999`（默认端口，可配置），鉴权用 Bearer Token
- 同一 Claude Code 会话最多一个 active 审批；新审批入队若已存在 active，返回 `409 busy_another_active`

### 1.5 审批状态机

```
       enqueue
   ┌──────────────┐
   │              ▼
[空] ─→ Pending ─decide(Approved|Denied)──→ Decided(Approved|Denied)
   │       │
   │       ├──expire_due_pending(timeout)──→ Decided(TimedOut)
   │       │
   │       └──cancel(reason)──────────────→ Decided(Cancelled{reason})
   │
   └─（终态后保留至 max_cached 上限，FIFO 淘汰）
```

**Decision 枚举**：

```rust
pub enum Decision {
    Approved  { feedback: Option<String> },
    Denied    { feedback: Option<String> },
    TimedOut,
    Cancelled { reason: CancelReason },
}

pub enum CancelReason {
    StopHook,        // /v1/hook/stop 触发
    AppExit,         // Tauri 关停
    UserAbort,       // HUD 主动取消（MVP-1 暂不暴露 UI，预留）
}
```

终态后状态不可变，重复 `decide` 返回 `AlreadyDecided { id, current }`。

### 1.6 单 active 审批不变量

- ApprovalStore 内显式 `active: Option<ApprovalId>` 字段
- 幂等键是 `approval_id`（不是 fingerprint）。`enqueue` 决策顺序：
  1. 若 `by_id.get(approval_id)` 命中：
     - 若已存的 fingerprint 与本次相同 → `EnqueueOutcome::Existing { approval, revision }`（幂等重试命中，无论该审批当前是 active 还是 cached）
     - 否则 → `EnqueueError::DuplicateIdConflict { id }`（同 id 不同 payload，hookcli 视为 fatal）
  2. 否则若 `active.is_some()` → `EnqueueError::BusyAnotherActive { active }`（与新请求的 fingerprint 是否相同**无关**，单 active 不变量优先）
  3. 否则正常创建 → `EnqueueOutcome::Created { approval, revision }`
- 终态后 `active` 清零，新审批可入队
- 历史终态保留在 `cached`，按 `max_cached` FIFO 淘汰；幂等查找仍命中 cached（终态返回 `Existing`，hookcli 直接消费决策）

### 1.7 幂等键与 fingerprint

- `approval_id`：UUID v4，由 hookcli 在请求前生成；作为幂等重试 key，不充当安全令牌
- `fingerprint`：SHA256 摘要，输入按以下顺序拼接（length-prefix + 版本 + domain separator，避免歧义）：
  ```
  "winvibe-fp\x00"
  || u8(fingerprint_version=1)
  || u32_be(len(session_id))   || session_id_bytes
  || u32_be(len(tool_name))    || tool_name_bytes
  || u32_be(len(canonical_tool_input)) || canonical_tool_input_bytes
  ```
- `canonical_tool_input`：键序排序、数字规范化（ryu/itoa）、不保留无意义空白；不修改字符串语义
- 同 `session_id + tool_name + canonical_tool_input` → 同 fingerprint → 触发幂等

### 1.8 时钟分离

- `WallClock`：墙上时钟（`SystemTime`），仅用于 audit 时间戳；可被时区/校时影响
- `MonotonicClock`：单调时钟（`u64` 毫秒计数），用于 TTL / timeout / `expires_at_mono_ms`；不受时钟回拨影响
- 两个 trait 注入 ApprovalStore，测试用 fake；详见 §5.D test clock harness

---

## §2 工程结构与依赖矩阵

### 2.1 仓库布局（Cargo workspace，路径 A）

```
winvibe/
├─ Cargo.toml                    # [workspace]
├─ rust-toolchain.toml           # 暂用 stable，MSRV 待定
├─ deny.toml                     # cargo-deny 配置
├─ scripts/
│  ├─ check-deps.ps1             # cargo metadata 依赖矩阵校验
│  ├─ check-tracing.ps1          # tracing 字段启发式校验
│  └─ check-ts-drift.ps1         # ts-rs 漂移校验
├─ crates/
│  ├─ winvibe-core/              # 纯同步、纯逻辑
│  │  └─ src/{lib.rs, protocol.rs, approval/, session/, config.rs, clock.rs, trace.rs, error.rs}
│  ├─ winvibe-hook-server/       # axum + tokio
│  │  └─ src/{lib.rs, runtime.rs, handlers/, middleware/, error.rs}
│  ├─ winvibe-hookcli/           # ureq + clap，[lib] + [[bin]]
│  │  └─ src/{lib.rs, main.rs, commands/, http_client.rs, trace_ctx.rs, exit_code.rs, config_loader.rs}
│  ├─ winvibe-app/               # tauri + tokio，[lib] + [[bin]]
│  │  └─ src/{lib.rs, main.rs, commands.rs, events.rs, tray.rs,
│  │           audit/, diagnostics/, redact/, ipc_error.rs,
│  │           close_orchestration.rs, config_loader.rs}
│  ├─ winvibe-contract-tests/    # publish=false，仅测试
│  │  └─ src/lib.rs（占位）+ tests/
│  └─ winvibe-e2e/               # publish=false，仅测试
│     └─ src/lib.rs（占位）+ tests/
└─ web/                          # React + Vite
```

### 2.2 依赖方向矩阵

允许（→ 表示「可依赖」）：

```
winvibe-core         → （无依赖任何 winvibe-* crate）
winvibe-hook-server  → winvibe-core
winvibe-hookcli      → winvibe-core
winvibe-app          → winvibe-core + winvibe-hook-server
winvibe-contract-tests → winvibe-core + winvibe-hook-server + winvibe-hookcli (dev-dep)
winvibe-e2e          → winvibe-core + winvibe-hook-server + winvibe-hookcli + winvibe-app (dev-dep)
```

**禁止**：
- `winvibe-core` 依赖 tokio / axum / tauri / ureq / toml / std::io::Error 作 `#[from]`
- `winvibe-hook-server` 依赖 tauri
- `winvibe-hookcli` 依赖 tokio / hyper / reqwest
- 任何非 e2e/contract crate 依赖 `winvibe-app`
- 任何 crate 反向依赖 `winvibe-core` 之外的 sibling

### 2.3 关键三方依赖

| crate | 三方依赖 |
|---|---|
| winvibe-core | serde / serde_json (raw_value) / thiserror / uuid / sha2 / ryu / itoa / rand |
| winvibe-hook-server | + axum / tokio / tower / tower-http / tracing / async-trait |
| winvibe-hookcli | + ureq / clap / tracing / tracing-subscriber |
| winvibe-app | + tauri / tokio / async-trait / tracing / tracing-subscriber / toml / ts-rs (dev/feature) |

### 2.4 CI 依赖矩阵校验

`scripts/check-deps.ps1`：
- 通过 `cargo metadata --format-version 1` 解析 packages 与 dependencies
- 用 package id → name 的映射判定（避免 grep root row 误匹配）
- 对每个 winvibe-* crate 检查其 deps 是否仅在 §2.2 允许列表内
- 对禁用三方依赖（如 hookcli 的 tokio）做反向断言

### 2.5 ts-rs 类型导出

- 在 winvibe-app 启用 feature `ts-export`，触发 `ts-rs` 把 IPC 类型导出到 `web/src/types/generated/*.ts`
- CI 跑 `cargo test --features ts-export` 后 `git diff --exit-code web/src/types/generated/`，漂移即失败

---

## §3 运行时与并发

### 3.1 ApprovalStore（winvibe-core，纯同步）

```rust
pub struct ApprovalStore {
    active: Option<ApprovalId>,
    by_id: HashMap<ApprovalId, Approval>,
    fingerprint_index: HashMap<Fingerprint, ApprovalId>,
    cached_order: VecDeque<ApprovalId>,
    limits: ApprovalStoreLimits,
    wall: Arc<dyn WallClock>,
    mono: Arc<dyn MonotonicClock>,
}

pub struct ApprovalStoreLimits {
    pub max_active: usize,    // MVP-1 固定 1
    pub max_cached: usize,    // MVP-1 默认 64，可配置
}

pub enum EnqueueOutcome {
    Created  { approval: Approval, revision: u64 },
    Existing { approval: Approval, revision: u64 },
}

pub enum EnqueueError {
    BusyAnotherActive  { active: ApprovalId },
    DuplicateIdConflict { id: ApprovalId },
    StoreFull,
}

pub enum DecideError {
    NotFound       { id: ApprovalId },
    AlreadyDecided { id: ApprovalId, current: Decision },
}

pub enum CancelError {
    NotFound       { id: ApprovalId },
    AlreadyDecided { id: ApprovalId, current: Decision },
}
```

所有方法：
- 内部 `Mutex` 锁；
- 状态变更后立即返回该次操作对应的 `revision` 值（递增 u64）；
- `expire_due_pending(now_mono_ms)` 返回 `Vec<(ApprovalId, u64)>`：超时被淘汰的 id 与新 revision；
- canonical_json 与 fingerprint 计算在 lock-free 阶段完成，**不**持锁做 SHA256。

### 3.2 ApprovalRuntime（winvibe-hook-server）

包装 ApprovalStore 引入 tokio 异步与 watch 通道：

```rust
pub struct ApprovalRuntime {
    store: Arc<Mutex<ApprovalStore>>,
    watchers: Arc<Mutex<HashMap<ApprovalId, watch::Sender<RevisionTick>>>>,
    sink: Arc<dyn ApprovalLifecycleSink>,
    accepting: AtomicBool,    // begin_shutdown 后 false，仅阻塞新 Pending 创建
    wall: Arc<dyn WallClock>,
    mono: Arc<dyn MonotonicClock>,
}

impl ApprovalRuntime {
    pub fn begin_shutdown(&self);

    pub async fn submit_pre_tool_use(
        &self,
        trace: TraceCtx,
        raw: PreToolUsePayload,
        max_wait: Duration,
    ) -> Result<WaitOutcome, RuntimeError>;

    pub async fn poll_decision(
        &self,
        trace: TraceCtx,
        id: ApprovalId,
        max_wait: Duration,
    ) -> Result<WaitOutcome, RuntimeError>;

    pub async fn decide(
        &self,
        trace: TraceCtx,
        id: ApprovalId,
        decision: Decision,
    ) -> Result<(), RuntimeError>;

    pub async fn cancel_session(
        &self,
        trace: TraceCtx,
        session_id: SessionId,
        reason: CancelReason,
    ) -> Result<CancelSummary, RuntimeError>;

    pub async fn cancel_all_pending(
        &self,
        trace: TraceCtx,
        reason: CancelReason,
    ) -> CancelSummary;

    pub async fn snapshot(&self) -> ApprovalListSnapshot;
}

pub enum WaitOutcome {
    Decided  { approval: Approval, revision: u64 },
    Pending  { id: ApprovalId, revision: u64 },     // 25s 超时返回，hookcli 重新 poll
    Existing { approval: Approval, revision: u64 },
    Created  { approval: Approval, revision: u64 },
}
```

### 3.3 短轮询协议

- 单次 `submit` 或 `poll_decision` 服务端最多挂 25 秒（`max_wait`）；
- 25 秒超时返回 `WaitOutcome::Pending`，hookcli 复用相同 `approval_id` 重新 `poll_decision`；
- `expire_due_pending` 在所有 state-touching 入口（submit / poll / decide / cancel / snapshot）首先调用，保证服务端定义的「审批 TTL」与「每次轮询窗口」一致；
- `expires_at_mono_ms = created_mono_ms + approval_ttl_ms`，TTL 默认 120 秒（可配置）。

### 3.4 watch 通道与 wake 顺序

- 每个 active approval 关联一个 `watch::Sender<RevisionTick>`；
- `Existing(Pending)` 路径在 store lock 内订阅 rx，规避 subscribe-after-event race；
- 状态变更时先更新 store + 计算新 revision，再 `watcher.send_replace(RevisionTick { revision })`；
- `poll_decision` 的查询顺序：`by_id` → 若 Pending 则订阅 watch → 等待 `tokio::select! { changed, sleep(max_wait) }`；
- 等待中若 `changed` 触发，再次读 store 拿最新状态；
- 若查询时 id 不存在 → `RuntimeError::NotFound`，**不**自我修复（曾误为 self-heal，已废）。

### 3.5 ServerHandle 生命周期

```rust
pub struct ServerHandle {
    inner: Arc<Mutex<Option<ServerHandleInner>>>,   // 用 take() 模拟移动语义
    shutting_down: AtomicBool,
}

impl ServerHandle {
    pub async fn shutdown(&self) -> Result<(), ShutdownError>;
}
```

- `oneshot::Sender` 与 `JoinHandle` 封装在 `ServerHandleInner` 内，外部不可见；
- 第二次 `shutdown` 返回 `ShutdownError::AlreadyShuttingDown`；
- AppExit 关闭顺序（必须按此严格执行，否则挂起中的 poll 客户端会先丢连接、后被取消）：
  1. `runtime.begin_shutdown()` —— 仅阻断**新** Pending 入队（`accepting=false`），不动 in-flight 与已挂起的 long-poll；
  2. `runtime.cancel_all_pending(AppExit).await` —— 把所有 Pending 推到 `Cancelled{AppExit}` 终态，唤醒挂起的 watch；in-flight long-poll 在同一 tick 内拿到决策并正常 200 响应；
  3. `ServerHandle::shutdown().await` —— 等待 in-flight 请求完成回包后停 axum；
  4. `audit.flush().await` + `audit.shutdown().await` —— 把 cancel 产生的终态行落盘后才允许进程退出；
  5. Tauri `prevent_close` 解除，事件循环退出。

### 3.6 ApprovalLifecycleSink

```rust
// winvibe-hook-server
pub trait ApprovalLifecycleSink: Send + Sync {
    fn approval_pushed(
        &self,
        trace: TraceCtx,
        parent_span: tracing::Span,
        id: ApprovalId,
        revision: u64,
    );
    fn approval_resolved(
        &self,
        trace: TraceCtx,
        parent_span: tracing::Span,
        approval: Approval,
        revision: u64,
    );
}
```

- 所有 push / resolved 通知统一走此 trait；audit、IPC 事件、diagnostics 都是它的实现 / 下游；
- 显式传递 `parent_span`，禁止 `Span::current()`。

### 3.7 IPC 事件契约

`approval_pushed` / `approval_resolved` payload **仅**含两字段：

```jsonc
{ "approval_id": "uuid", "revision": 42 }
```

HUD 收到事件后调 `snapshot()` 拉取详情，详细字段在 §4 与 §3.8 列出。

### 3.8 ApprovalListSnapshot

```rust
pub struct ApprovalListSnapshot {
    pub active: Option<Approval>,
    pub cached: Vec<Approval>,
    pub revision: u64,
}
```

序列化到 IPC 时 `Approval` 包含：
- `id`, `session_hash`, `tool_name`
- `fingerprint`, `fingerprint_version`, `tool_input_raw_sha256`, `tool_input_canonical_sha256`, `tool_input_original_bytes`
- `created_wall`, `expires_at_mono_ms`
- `state`, `decision`（若已决）
- `trace_id`, `approval_entry_span_id`
- 不含原始 `session_id`、不含 `tool_input` 原文、不含 `caller_cwd`（除非用户开启 opt-in）

---

## §4 错误、审计、诊断、tracing、trace 透传

### 4.1 稳定错误码矩阵

HTTP 错误返回**扁平** JSON `{ "code": "...", "message": "...", "trace_id": "...", "approval_id": "..."? }`。code 字段在 MVP-1 内**冻结**（增加新值不视为破坏性变更，但既有 code 语义不可变）。所有 code 用 `snake_case`，与 hookcli/contract-tests 共享同一常量表（在 `winvibe-core::protocol::error_code`）：

| HTTP | code | 含义 | 备注 |
|---|---|---|---|
| 200 | — | 决策已返回（含 Approved/Denied/TimedOut/Cancelled） | hookcli 据 decision 字段产出 Claude Code hook JSON，进程仍 exit 0 |
| 202 | — | 短轮询窗口超时，仍 Pending | hookcli 复用 approval_id 重新 poll，无 exit |
| 400 | `invalid_request` | 请求结构错（JSON 反序列化失败、缺字段、字段类型不符等） | message 含具体原因 |
| 401 | `unauthorized` | Bearer Token 缺失或不匹配 | |
| 403 | `origin_forbidden` | Origin / Host 校验失败（非 loopback、非白名单） | |
| 409 | `busy_another_active` | 已有 active 审批 | hookcli fail-open，stdout 输出 allow JSON，exit 0 |
| 409 | `duplicate_id` | 同 approval_id 已存在但 fingerprint 不一致 | hookcli 视为 fatal，exit 2 |
| 413 | `payload_too_large` | tool_input 超过 max_tool_input_bytes（默认 1 MiB） | hookcli fatal，exit 2 |
| 422 | `payload_unprocessable` | 通过反序列化但语义校验失败（tool_input 非 object 等） | hookcli fatal，exit 2 |
| 500 | `internal_error` | 服务端不变量被破坏 | 同时输出 diagnostic |
| 503 | `shutting_down` | 服务端进入 begin_shutdown 后拒绝新 Pending | hookcli fatal，exit 2 |

### 4.2 hookcli exit code 与 stdout 协议

hookcli 进程退出码语义**与决策无关**，仅表达「能否安全交还控制给 Claude Code」：

- `0`：协议层成功——服务端给出确定结果（Decision 或 busy_another_active 或网络层 fail-open），hookcli 已经把对应的 Claude Code hook JSON 写到 stdout。Claude Code 据 stdout JSON 决定 allow/block。
- `2`：基础设施失败——必须 fail-closed（block 该工具调用），不写 stdout JSON。命中场景：401 / 403 / 400 / 413 / 422 / 409 duplicate_id / 500 / 503 / 协议反序列化失败 / 配置加载失败。

显式枚举：

| 服务端结果 | stdout (Claude Code hook JSON) | exit |
|---|---|---|
| 200 Approved | `{ "decision": "approve", "reason": "..." }` | 0 |
| 200 Denied | `{ "decision": "block", "reason": "..." }` | 0 |
| 200 TimedOut | `{ "decision": "approve", "reason": "winvibe: timed_out, fail-open" }` | 0 |
| 200 Cancelled | `{ "decision": "approve", "reason": "winvibe: cancelled:<reason>" }` | 0 |
| 409 busy_another_active | `{ "decision": "approve", "reason": "winvibe: another approval active" }` | 0 |
| 网络错误 / 连不上 server | 不写 stdout | 2 (fail-closed) |
| 401/403/400/413/422/409 duplicate_id/500/503 | 不写 stdout | 2 |

stderr 始终承载诊断信息；diagnostic JSONL 是结构化版本，stderr 是人类可读版本。网络错误**必须** fail-closed（exit 2），不可为了便利 fail-open。

### 4.3 tracing 与 W3C trace-id

- TraceCtx 结构（winvibe-core）：

  ```rust
  pub struct TraceCtx {
      pub trace_id: TraceId,         // 16 bytes，序列化为 32 hex
      pub entry_span_id: SpanId,     // 8 bytes，序列化为 16 hex
      pub source: TraceSource,
  }

  pub enum TraceSource {
      HookCliRequest,                // hookcli 入口
      HudIpc,                        // HUD → app 命令入口
      System(SystemTraceSource),     // 服务端自发起
  }

  pub enum SystemTraceSource {
      AppExitCancel,                 // AppExit 触发的 cancel_all_pending
      Sweeper,                       // expire_due_pending 内部触发
  }
  ```

- TraceId / SpanId 手写 serde（hex 字符串），不 derive，避免上游误用 byte array。
- HTTP middleware（tower-http + 自定义 layer）：
  - 入站：解析 `traceparent` header（`00-{trace_id}-{span_id}-{flags}`），缺失或非法则**生成新 trace_id + entry_span_id**，并在 response header 回写规范 `traceparent`。
  - 启发式回退而非强保证：若 traceparent flags 不识别，按 `01` 处理；不实现 tracestate。
  - 每请求建一个 root tracing span，**仅**记录白名单字段：`http.method`, `http.route`, `http.status`, `trace_id`, `approval_id`（若已知）。
- 业务跨度通过显式 `parent_span: tracing::Span` 参数传递，禁止 `Span::current()`。
- WARN/ERROR 级别仅用于：服务端不变量破坏（500）、配置加载失败、watcher 异常。409/202 等业务期望状态记 INFO。

### 4.4 audit JSONL

- 路径：`%LOCALAPPDATA%\WinVibe\audit\YYYY-MM-DD.jsonl`，**按 UTC 日期切分**（避免本地时区切换/夏令时跳变期间出现重复或缺失日切；时间戳本身仍含完整偏移）。
- 一条记录 = 一次审批终态（仅事实摘要，不含原始反馈正文）：

  ```jsonc
  {
    "schema": "winvibe.audit.v1",
    "approval_id": "uuid",
    "session_hash": "16-hex",
    "tool_name": "string",
    "fingerprint": "64-hex",
    "fingerprint_version": 1,
    "decision": {
      "kind": "Approved" | "Denied" | "TimedOut" | "Cancelled",
      "cancel_reason": "StopHook" | "AppExit" | "UserAbort",   // 仅 Cancelled
      "feedback_present": true,                                 // bool，是否带过 feedback
      "feedback_sha256": "64-hex",                              // optional, 仅 present 时
      "feedback_preview": "first ≤80 chars, line-broken stripped, NFC normalized"  // optional
    },
    "created_wall": "RFC3339 UTC",
    "decided_wall": "RFC3339 UTC",
    "approval_trace_id": "32-hex",
    "decision_trace_id": "32-hex",
    "tool_input_raw_sha256": "64-hex",
    "tool_input_canonical_sha256": "64-hex"
  }
  ```

- **不**写 `decision.feedback` 原文。仅落 `feedback_sha256` + `feedback_preview`（≤80 字符截断）。如用户在配置中显式 opt-in `audit.full_feedback = true`，可改写完整正文，但默认关闭。
- 不写中间 Pending 事件；只写终态事实。
- AuditSink trait（async-trait，含 `flush`/`shutdown`）：实现支持 spawn-thread blocking writer + bounded mpsc 缓冲；缓冲满则丢弃并 INC `audit_dropped_total`（仅 metrics，无 panic）。
- AppExit 流程必须按 §3.5 顺序完成 `flush + shutdown`，否则视为不变量破坏（500）。

### 4.5 diagnostic JSONL

- 路径：`%LOCALAPPDATA%\WinVibe\diagnostics\{approval_id}.jsonl`，一审批一文件，每事件一行。
- 用于人工排障，不参与业务逻辑。仅当 `diagnostics_enabled = true`（默认 true）时写入。
- 字段示例：

  ```jsonc
  {
    "ts_wall": "RFC3339",
    "ts_mono_ms": 12345,
    "kind": "hookcli.attempt" | "server.received" | "server.decided" | "ipc.snapshot" | "error",
    "trace_id": "32-hex",
    "span_id": "16-hex",
    "approval_id": "uuid",                  // optional
    "approval_trace_id": "32-hex",          // optional, skip_serializing_if
    "approval_entry_span_id": "16-hex",     // optional
    "message": "free text",
    "extra": { "...": "..." }               // 结构化扩展
  }
  ```

- hookcli 重试时同一 trace_id + 新 span_id；不复用 span_id。
- IPC 事件 `approval_pushed`/`approval_resolved` 仅含 `{ approval_id, revision }`；HUD 收到后调 `snapshot()` 拉详情，避免事件载荷膨胀。

### 4.6 redact 规则

- audit / diagnostic / span 字段均不得含原始 `session_id`、原始 `tool_input` 文本、`caller_cwd`、用户路径片段。
- `session_hash = SHA256(session_id || "winvibe-sess-v1")[..8]` 转 16 hex。
- IPC `Approval` 外发时执行同样的 redact pipeline；redact 规则集中在 `winvibe-app/src/redact/` 下。

---

## §5 测试策略

### 5.1 测试金字塔

| 层 | 位置 | 跑手 | 内容 |
|---|---|---|---|
| 单元 | 各 crate `src/` 内 `#[cfg(test)]` | `cargo test -p <crate>` | 纯函数、状态机迁移、canonical_json/fingerprint、redact、TraceCtx parse |
| 集成 | `crates/winvibe-contract-tests/tests/` | `cargo test -p winvibe-contract-tests` | HTTP 协议契约：错误码矩阵、短轮询、幂等、ipc snapshot 形状 |
| E2E | `crates/winvibe-e2e/tests/` | `cargo test -p winvibe-e2e -- --test-threads=1` | hookcli ↔ server ↔ app 三方真实 spawn，含 Tauri |
| 前端 | `web/` | `pnpm vitest` | React 组件、IPC 类型 |

### 5.2 winvibe-contract-tests 矩阵

针对 §4.1 表中**每个 (HTTP, code) 组合**至少一个用例。最小集合：

- `200` Approved / Denied / TimedOut / Cancelled（决策路径全覆盖；断言对应 hookcli stdout JSON 与 exit 0）
- `202` 短轮询超时 → hookcli 复用 id 二次 poll，无 exit
- `400 invalid_request`
- `401 unauthorized`、`403 origin_forbidden`
- `409 busy_another_active`（hookcli fail-open 写 allow JSON、exit 0）
- `409 duplicate_id`（同 id 不同 fingerprint，hookcli exit 2 无 stdout）
- `413 payload_too_large`（构造 > 1 MiB tool_input）
- `422 payload_unprocessable`（tool_input 非 object 等）
- `500 internal_error`（fault injection sink）
- `503 shutting_down`（先 begin_shutdown 再发新 Pending）
- 网络层：server 不可达 → hookcli exit 2，不写 stdout（fail-closed 回归）

幂等回归（与 §1.6 对齐）：
- 同 `approval_id` + 同 fingerprint 多次提交 → `Existing`，仅一条 audit；
- 同 `approval_id` + 不同 fingerprint → `409 duplicate_id`；
- 不同 `approval_id`（且当前有 active）+ 同 fingerprint → `409 busy_another_active`（**不**幂等合并）。

### 5.3 winvibe-app + winvibe-hookcli 的 lib + bin 形态

为让 contract-tests / e2e 能复用代码：

```toml
# winvibe-hookcli/Cargo.toml
[lib]
name = "winvibe_hookcli"
path = "src/lib.rs"

[[bin]]
name = "winvibe-hookcli"
path = "src/main.rs"
```

```toml
# winvibe-app/Cargo.toml
[lib]
name = "winvibe_app"
path = "src/lib.rs"

[[bin]]
name = "winvibe-app"
path = "src/main.rs"
```

- `main.rs` 仅做 CLI 解析 / Tauri 启动调用 `lib::run(...)`；
- 测试 crate 通过 `dev-dependencies` 引入 lib，spawn 子进程时仍用 `cargo_bin!("winvibe-hookcli")`。

### 5.4 TestClockHarness

```rust
pub struct TestClockHarness {
    wall: Arc<FakeWallClock>,
    mono: Arc<FakeMonotonicClock>,
}

impl TestClockHarness {
    /// 构造时一次性 pause tokio 时间。后续不得再次 pause()，
    /// 否则 tokio 内部计时器状态会被打乱。
    pub fn new() -> Self {
        tokio::time::pause();
        Self {
            wall: Arc::new(FakeWallClock::default()),
            mono: Arc::new(FakeMonotonicClock::default()),
        }
    }

    pub fn arc_wall(&self) -> Arc<dyn WallClock> { self.wall.clone() }
    pub fn arc_mono(&self) -> Arc<dyn MonotonicClock> { self.mono.clone() }

    /// 同步推进 tokio::time + mono clock + wall clock，避免 sleep race。
    /// 不重复 pause()。
    pub async fn advance(&self, d: Duration) {
        self.mono.advance(d);
        self.wall.advance(d);
        tokio::time::advance(d).await;
    }
}
```

所有涉及 TTL / 超时的测试**必须**用 harness（`#[tokio::test(start_paused = true)]` 或 `TestClockHarness::new()` 二选一，全程不再调 `pause()`），不得直接 `tokio::time::sleep` 真实等待。

### 5.5 配置加载与校验

两段式解析，原始字段保留 String 以便错误信息保稳定：

```rust
// winvibe-core/src/config.rs
#[derive(Debug, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("bind address must be loopback, got {raw}")]
    BindNotLoopback { raw: String },
    #[error("port must be 1..=65535, got {raw}")]
    PortOutOfRange { raw: String },
    #[error("port 0 is not allowed in production config")]
    PortZeroDisallowed,
    #[error("approval_ttl_ms ({got}) below minimum ({min})")]
    StaleTimeoutTooSmall { got: u64, min: u64 },
    #[error("auth_token format invalid (expect 32+ hex chars)")]
    AuthTokenFormatInvalid,
    #[error("auth_token missing")]
    MissingAuthToken,
}

pub struct WinvibeConfig {
    pub bind: IpAddr,                 // 必为 loopback
    pub port: u16,                    // 1..=65535
    pub auth_token: AuthToken,
    pub approval_ttl_ms: u64,
    pub max_cached: usize,
}

#[derive(Debug, serde::Deserialize)]
pub struct RawWinvibeConfig {
    #[serde(default = "default_bind")]
    pub bind: String,                 // 默认 "127.0.0.1"
    #[serde(default = "default_port")]
    pub port: String,                 // 默认 "59999"，String 以便 PortOutOfRange 携带原值
    pub auth_token: Option<String>,
    #[serde(default = "default_ttl")]
    pub approval_ttl_ms: u64,
    #[serde(default = "default_cached")]
    pub max_cached: usize,
}

impl RawWinvibeConfig {
    pub fn validate(self) -> Result<WinvibeConfig, ConfigValidationError> { /* ... */ }
}
```

App / hookcli 侧加 IO + toml 包装层（不入 core）：

```rust
// winvibe-app/src/config_loader.rs
#[derive(Debug, thiserror::Error)]
pub enum ConfigLoadError {
    #[error("io error reading {path}: {source}")]
    Io { path: PathBuf, #[source] source: std::io::Error },
    #[error("toml decode error: {0}")]
    TomlDecode(#[from] toml::de::Error),
    #[error(transparent)]
    Validation(#[from] ConfigValidationError),
}

/// App 启动主入口：负责首启 token 引导。clean install 不应直接报错，
/// 而应生成 32 hex 随机 token 写回配置文件后再走 validate。
pub fn load_or_init_config_app(path: &Path) -> Result<WinvibeConfig, ConfigLoadError> {
    ensure_default_config_file(path)?;       // 文件不存在则写默认骨架（含占位 auth_token = "")
    let bytes = std::fs::read_to_string(path)
        .map_err(|e| ConfigLoadError::Io { path: path.into(), source: e })?;
    let mut raw: RawWinvibeConfig = toml::from_str(&bytes)?;

    // 首启 token 引导：仅当字段缺失或为空时才生成并落盘；已有值则交给 validate 检查格式。
    if raw.auth_token.as_deref().map_or(true, str::is_empty) {
        let token = generate_auth_token_hex();   // 32 hex chars，CSPRNG
        persist_auth_token(path, &token)?;       // 原子写回 toml
        raw.auth_token = Some(token);
    }

    Ok(raw.validate()?)
}

/// 纯加载（无引导），供测试与 hookcli 侧使用。
/// 前置条件：调用方保证 raw.auth_token 已被填充；
/// 此处出现 None 视为编程错（不应作为运行时正常路径）。
pub fn load_config_strict(path: &Path) -> Result<WinvibeConfig, ConfigLoadError> {
    let bytes = std::fs::read_to_string(path)
        .map_err(|e| ConfigLoadError::Io { path: path.into(), source: e })?;
    let raw: RawWinvibeConfig = toml::from_str(&bytes)?;
    if raw.auth_token.is_none() {
        // 严格语义分离：缺失 → MissingAuthToken；非法格式由 validate() 抛 AuthTokenFormatInvalid
        return Err(ConfigValidationError::MissingAuthToken.into());
    }
    Ok(raw.validate()?)
}
```

- `load_or_init_config_app` 是 app 入口的唯一推荐路径；`load_config_strict` 仅供 hookcli 与测试使用，hookcli 不参与 token 生成（首启 token 必须由 app 落盘后 hookcli 才能读到）。
- `persist_auth_token` 必须用临时文件 + rename 原子替换，避免半写损坏配置文件。

### 5.6 安全相关测试

- 启动时 `bind != IpAddr::is_loopback()` → 立即拒绝（不绑端口、不写 audit、退出码 78 `EX_CONFIG`）。
- 缺失或畸形 Bearer Token → 401，不接续业务路径。
- Origin / Host header 非白名单 → 403 `origin_forbidden`。
- IPv6 `::1` 监听通过运行时探测开启：CI 在能绑 `::1` 的 runner 上跑双栈用例，否则 skip；不引入 cfg 区分平台。

### 5.7 ts-rs drift 校验

CI step：

1. `cargo test -p winvibe-app --features ts-export`
2. `git diff --exit-code web/src/types/generated/`

漂移即 fail；本地修复方式：跑 1 然后 commit 生成的 ts 文件。

### 5.8 不在 MVP-1 测试范围

- 视觉回归（Tauri webview 截图比对）
- 性能压测、fuzz、mutation testing
- macOS / Linux runner
- 真实 Claude Code 集成（仅契约层模拟其调用形态）

---

## 附录 A：已知限制

- Windows-only；macOS / Linux 留待 MVP-2+。
- 同会话仅支持 1 个 active 审批；多审批排队留待后续。
- 不实现 W3C `tracestate`、不实现 OTLP exporter；trace 仅本地 JSONL。
- HUD 不暴露 UserAbort UI（枚举预留）。
- `caller_cwd` 默认不收集，opt-in 后才进 audit。
- audit / diagnostic 不做加密；依赖文件系统 ACL（`%LOCALAPPDATA%` 用户私有）。

## 附录 B：术语表

| 术语 | 定义 |
|---|---|
| ApprovalId | UUID v4，hookcli 生成，幂等键，非安全令牌 |
| Fingerprint | SHA256(version + length-prefix(session_id, tool_name, canonical_tool_input))，幂等命中依据 |
| Active | ApprovalStore 中处于 Pending 状态的唯一审批 |
| Cached | 已终态、按 FIFO 留存于 store 的审批，供 snapshot 回溯 |
| TraceCtx | trace_id + entry_span_id + source，跨进程透传的最小单元 |
| Revision | ApprovalStore 单调递增 u64，每次状态变更 +1，watch 通道载荷 |
| Sweeper | `expire_due_pending` 调用方，在所有 state-touching 入口前置触发 |
| HookCli | 由 Claude Code PreToolUse hook 同步调用的阻塞 Rust 二进制 |
| HUD | Tauri + React 渲染的审批卡片窗口 |
