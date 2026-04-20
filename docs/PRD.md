# WinVibe 产品需求文档 (PRD)

**版本**: 0.9
**日期**: 2026-04-20
**变更**: 依据独立架构评审修订——审批协议改为短轮询 + 幂等重提交（防止长连接在休眠/代理断连时丢失）；安全叙事降级为"防重放 + 跨用户"，不再声称防同用户伪造；引入 Tauri IPC HMAC 握手；token 估算改为字符数口径，不再显示百分比；单实例端口偏移联动 hook 重写；HUD 最大宽度放宽以满足真实像素预算
**v0.8**: 抽离视觉规范至 `UI-SPEC.md`；明确折叠态字段优先级与降级、拖拽阈值、审批队列处理顺序与切换语义；补充审批 nonce、单实例作用域、WAITING_APPROVAL 心跳维持、token 估算、PreToolUse 阻塞期 UI 倒计时
**v0.7**: 新增 Agent 活动子状态、智能通知策略、交互式提问响应（预留）、用量监控、Mascot 彩蛋；更新配置管理

---

## 1. 产品背景

### 1.1 问题陈述

AI 编程 Agent（Claude Code、Codex、Gemini CLI 等）越来越普遍地在后台持续运行。开发者面临两难：

- **切换成本高**：频繁切换到终端查看进度，打断心流
- **错过审批时机**：Agent 等待权限审批时，开发者不知道，导致流程卡死
- **多 Agent 混乱**：同时跑多个 Agent，无统一视角

macOS 的 [Vibe Island](https://vibeisland.app) 用 Dynamic Island/Notch 解决了这个问题，但 Windows 和 Linux 开发者没有对等工具。

### 1.2 目标用户

- 使用 AI Coding Agent 辅助开发的工程师
- 同时运行多个 Agent 任务的重度用户
- Windows / Linux 主力开发环境的开发者

### 1.3 产品目标

| 目标     | 指标                     |
| ------ | ---------------------- |
| 减少切换次数 | 无需主动切换到终端查看 Agent 状态   |
| 零延迟审批  | Agent 请求审批后 HUD 立即弹出提示 |
| 低侵入性   | 折叠时占用屏幕空间 < 40px 高度    |
| 轻量运行   | 常驻内存 < 80MB            |
| 安全可靠   | 审批请求不丢失，超时/异常默认拒绝      |

---

## 2. 功能需求

### 2.1 HUD 悬浮窗（核心）

**描述**: 屏幕顶部居中的常驻悬浮窗，分为折叠态和展开态。

> **视觉规范**: 所有颜色、字体、间距、动画曲线、组件细节由 [`UI-SPEC.md`](UI-SPEC.md) 统一定义。本节仅描述功能与交互。

#### 折叠态（默认）

```
┌──────────────────────────────────────────────────────────┐
│  ● Claude Code  编辑中  fix auth bug  │  ● Codex²  等待中  db query  │
└──────────────────────────────────────────────────────────┘
```

- 高度 ≤ **32px**（逻辑像素，自动适配 DPI 缩放，详见 UI-SPEC §3.1）
- 显示每个 Agent 的状态点（颜色 + 脉冲）+ 名称 + 活动子状态 chip + 任务摘要
- 有 Agent 请求审批时，相应卡片状态点切换为橙色急促脉冲（频率 1.2s）
- **审批数量 badge**: 待审批数 ≥ 1 时在状态点右上角显示数字角标
- 点击任意卡片 → 展开该 Agent 详情

##### 字段优先级与宽度降级

折叠态承载多 Agent + 多字段（状态点 / 名称 / chip / 摘要 / 用量条 / 像素猫），单卡片宽度不足时按下表从右往左隐藏。**Agent 名称至少保留 6 个可见字符，绝不全部截断**。完整规则见 UI-SPEC §3.3。

| 优先级 | 字段 | 隐藏阈值（卡片宽度） |
|---|---|---|
| P0 必现 | 状态点、Agent 名（最多 12 字）、审批 badge | 永不 |
| P1 | 活动子状态 chip | < 180px |
| P2 | 任务摘要（最多 24 字） | < 220px |
| P3 | 用量进度条 mini | < 260px |
| P3 | 像素猫 | < 240px |

#### 展开态

```
┌──────────────────────────────────────────────────┐
│  ● Codex   db/queries.ts                  ⤴ 跳转 │
│  ─────────────────────────────────────────────── │
│  权限请求：Edit                            4:23  │
│  ┌──────────────────────────────────────────────┐│
│  │ + return await db.query(sql, params);        ││
│  │ - return db.query(sql + userInput);          ││
│  └──────────────────────────────────────────────┘│
│  [ Approve ]  [ Deny ]   Feedback...             │
│  ─── 队列 (2) ─────────────────  全部 Deny       │
│  │● Claude  Edit auth.ts                  查看  │
│   ● Gemini  Write tests/auth.test.ts      查看  │
└──────────────────────────────────────────────────┘
```

- 展开高度自适应内容，最大 400px
- Diff 高亮渲染（克制配色，详见 UI-SPEC §4.6）
- Markdown 渲染（Plan 预览、问题回答）
- **审批超时倒计时**：在按钮上方右对齐显示剩余时间（SF Mono），剩余 < 20% 时变红脉冲
- **审批队列**: 当多个 Agent 同时请求审批时，展示审批队列列表，支持切换查看；提供「全部 Deny」批量操作
- 点击外部区域 → 自动折叠

#### 交互规格

| 交互            | 行为                  |
| ------------- | ------------------- |
| 点击折叠态卡片       | 展开对应 Agent 详情       |
| 点击 [跳转]       | 聚焦到对应终端窗口/Tab       |
| 点击 [Approve]  | 返回审批通过，按下方"队列处理顺序"进入下一条或折叠 |
| 点击 [Deny]     | 返回拒绝，按下方"队列处理顺序"进入下一条或折叠 |
| 点击 [Feedback] | 弹出文本输入框，输入后发送       |
| 点击 [全部 Deny] | 二次确认后对队列内所有请求返回 Deny |
| 点击队列中的 [查看]   | 切换到该审批请求的详情，**当前查看项 = 当前操作目标** |
| 点击 HUD 外部     | 折叠                  |
| 拖拽 HUD        | 移动位置（详见下方"拖拽规则"） |
| 双击折叠态         | 完全隐藏，托盘图标可还原        |
| 键盘 `Y` / `N` | 展开态聚焦时等价 Approve / Deny |
| 键盘 `Esc` | 折叠 HUD |

#### 审批队列处理顺序

- 队列按 `received_at` 升序排列（FIFO）
- "当前查看项"由用户最后一次点击决定，默认 = 队首
- Approve / Deny 始终作用于"当前查看项"，操作完成后：
  1. 如队列仍有其他项 → 自动切换到队首（最早进入队列的）
  2. 如队列为空 → 折叠 HUD
- 「全部 Deny」需二次确认，确认后所有 pending 请求按配置的默认动作返回（默认 Deny）

#### 拖拽规则

- **拖拽阈值**: 鼠标按下后位移 > 5px 才识别为拖拽，否则视为点击（避免与展开手势冲突）
- 拖拽期间禁用展开 / 外部点击折叠
- HUD 顶部边缘吸附：距屏幕顶部 < 16px 时自动吸附到 8px 边距
- 跨显示器拖拽：跟随鼠标，松开后保存到目标显示器
- 触发 `prefers-reduced-motion` 时禁用吸附动画

#### 位置记忆安全

- 保存位置时同时记录显示器分辨率和名称
- 恢复位置时校验坐标是否仍在当前任一显示器的可见范围内
- 如果坐标超出范围（如外接显示器拔除），自动回退到主显示器默认位置

### 2.2 Agent 状态管理

**描述**: 追踪所有 Agent 会话的生命周期和实时状态。

#### Agent 状态机

```
         ┌──────────────────────────────────────────┐
         │                                          │
         ▼                                          │
       IDLE ──► RUNNING ──► WAITING_APPROVAL ──► RUNNING
                  │  │              │
                  │  │              ├──► ERROR ──► RUNNING（恢复）
                  │  │              │                │
                  │  │              └──► DONE        └──► DONE
                  │  │
                  │  ├──► DONE
                  │  │
                  │  └──► ERROR ──► RUNNING（恢复）
                  │                    │
                  │                    └──► DONE
                  │
                  └──► STALE（超时清理）
```

| 状态               | 颜色       | 说明                 |
| ---------------- | -------- | ------------------ |
| IDLE             | 灰色       | 已注册，未运行            |
| RUNNING          | 绿色（脉冲）   | 正在执行               |
| WAITING_APPROVAL | 橙色（闪烁）   | 等待用户审批             |
| DONE             | 蓝色       | 完成                 |
| ERROR            | 红色       | 出错                 |
| STALE            | 灰色（虚线边框） | 超时未活动，疑似 Agent 已崩溃 |

#### 会话信息

每个 Agent 会话追踪以下信息：

- Agent 类型（claude / codex / gemini-cli / cursor / ...）
- 工作目录
- 当前任务描述
- 当前活动子状态（见下方）
- 用量快照（见 2.12 用量监控）
- 最近工具调用列表（默认 50 条，可配置）
- 运行时长
- 终端标识（用于跳转）
- 上次事件时间（用于超时检测）

#### Agent 活动子状态

在 RUNNING 主状态下，通过分析最近一次工具调用进一步推断 Agent 当前活动：

| 活动           | 推断来源                                      | HUD 显示        |
| ------------ | ----------------------------------------- | ------------- |
| Thinking     | 进入 RUNNING 但尚未调用工具                        | 思考中           |
| Reading      | 最近工具为 Read / Grep / Glob                  | 阅读中           |
| Editing      | 最近工具为 Edit / Write                        | 编辑中           |
| Testing      | 最近工具为 Bash 且命令含 test/jest/cargo test 等关键词 | 测试中           |
| WaitingInput | Agent 处于 WAITING_APPROVAL 时自动切换           | 等待中           |
| Unknown      | 无法推断                                      | （仅显示 RUNNING） |

- 活动子状态主要用于 RUNNING 主状态下的展示层提示（WaitingInput 为例外，对应 WAITING_APPROVAL 状态），不影响状态机转换逻辑
- 折叠态在状态点旁显示活动标签（如 `● Claude Code  编辑中  fix auth bug`）

> **限制**: 活动粒度取决于 Agent hook payload 中的 `tool_name` 字段。当前无法区分"AI 在思考"和"AI 在生成代码"——两者在 hook 视角下均为 Agent 未调用工具的间隙。Thinking 状态为推测性标记，实际含义是"上一次工具调用后尚未开始下一次"。

#### 会话超时清理

- 如果一个 Running 或 WaitingApproval 状态的会话超过 10 分钟没有收到任何事件，自动标记为 `Stale`
- **WAITING_APPROVAL 心跳维持**: 进入 WAITING_APPROVAL 时由后端立即刷新 `last_event_at`，并启动一个独立心跳任务，每 30s 刷新一次直到状态退出。这样正常审批流程（最长 5 分钟）不会触及 Stale 阈值，仅 hook 通道异常或后端任务被异常终止时才触发
- WAITING_APPROVAL 的有效 Stale 阈值为 `max(stale_timeout_secs, timeout_secs * 1.5)`（默认 600s 与 450s 取大）
- Stale 会话在 HUD 上显示灰色虚线边框，提示用户 "Agent 可能已停止"
- 用户可手动关闭/清理 Stale 会话
- Stale 会话超过 1 小时自动移除
- **配置约束**: `stale_timeout_secs` 必须大于 `timeout_secs + 60`（扫描周期），系统在启动加载和运行时热更新时均自动校验，不满足则拒绝操作

### 2.3 Hook 服务

**描述**: 本地 HTTP 服务，接收 AI Agent 通过 hooks 发送的事件。

#### 服务规格

- 监听地址：`127.0.0.1:59999`（可配置）
- 协议：HTTP/1.1
- 数据格式：JSON
- **认证**: 每个请求必须携带 `Authorization: Bearer <token>` header
- **请求体大小限制**: 最大 1MB，超出返回 413

#### 输入约束与保护

| 约束                | 限制值       | 说明                                                                         |
| ----------------- | --------- | -------------------------------------------------------------------------- |
| HTTP body 大小      | <= 1MB    | Axum body size limit，防止超大 payload 耗尽内存                                     |
| `tool_input` 字段大小 | <= 500KB  | 对字符串按行截断（首尾各 200 行）；对 Object/Array 序列化后超限则替换为摘要                            |
| 同时待审批请求数          | <= 32     | 超出后新请求直接返回默认动作（Deny），防止内存堆积                                                |
| 前端增量更新队列          | <= 1024 条 | bounded queue，满时按更新幂等性处理：非幂等更新（如工具调用记录）立即触发全量快照恢复；幂等更新（如状态变更）静默丢弃，下次同类更新覆盖 |

#### API 端点

```
POST /v1/hook/pre-tool-use      # 工具调用前（短轮询模式，见下）
POST /v1/hook/pre-tool-use/poll # 审批结果轮询（幂等）
POST /v1/hook/post-tool-use     # 工具调用后（非阻塞）
POST /v1/hook/stop              # Agent 会话结束
POST /v1/hook/notification      # Agent 通知消息
POST /v1/hook/register          # Agent 注册（可选）
GET  /v1/status                 # 服务健康检查（无需认证）
```

#### PreToolUse 审批流（短轮询 + 幂等重提交）

> **协议动机**: 长阻塞 HTTP（5 分钟）在系统休眠/唤醒、企业代理超时、VPN 切换、安全端点探测时极易断连，导致审批丢失且 curl 以非 0 退出——Agent 可能误判为拒绝或崩溃。改用短轮询后，单次请求最多 ~30s，断连后幂等重提交即可恢复。

```
Claude Code (curl)           WinVibe Hook Server              WinVibe HUD
    │                               │                               │
    │─ POST /pre-tool-use ─────────>│                               │
    │  (生成 request_id)            │ 验证 Token                    │
    │  X-Approval-Id: <uuid>        │ 注册审批请求 ─────────────────>│ 弹出审批 UI
    │                               │ 挂起 oneshot，超时 25s        │
    │<─ 202 { status: "pending",    │                               │
    │       approval_id, wait: 5 }  │                               │
    │  （或 200 { decision })       │                               │
    │                               │                               │
    │─ POST /pre-tool-use/poll ────>│                               │
    │  X-Approval-Id: <uuid>        │ 按 approval_id 查询           │
    │<─ 202 pending / 200 decision  │                               │
    │                               │<── decide(approval_id, ...) ──│
    │ ... 循环轮询 ...               │                               │
    │<─ 200 OK { decision }         │                               │
```

##### 协议细节

- **approval_id 生成方**: 由 **Agent curl 侧**在发起请求时生成（UUID v4），通过 `X-Approval-Id` header 传入。后端以 `approval_id` 作为幂等键，同一 ID 的重复提交返回相同结果；若尚未决策则返回 `202 pending`
- **单轮超时**: 服务端挂起最长 25s；超时返回 `202 pending` 让客户端立即重连，避开 TCP/代理 idle 断连（通常 30-60s）
- **客户端重连**: curl 侧循环 poll，无指数退避（立即重连）。总等待时间由客户端 `--max-time` 决定（默认 310s，配置即生效）
- **决策持久化**: 后端对每个 `approval_id` 的决策结果缓存 10 分钟（内存），覆盖重连窗口
- **审批队列容量**: ≤ 32（同 §2.3 输入约束）。进入队列后，客户端超时（poll 停止调用）不影响队列项——用户仍可操作，但 Agent 侧可能已放弃
- **休眠/唤醒场景**: 系统休眠期间 curl 进程挂起，唤醒后立即重新 poll，服务端按 `approval_id` 返回缓存的决策

##### approval_id 的安全定位（重要）

- `approval_id` **不是秘密**，仅用于幂等匹配
- 它的目的：避免"用户点了 Approve 但网络抖动导致 Agent 没收到"的丢失
- **不依赖其保密性防御任何伪造攻击**（见 §3.3 安全边界讨论）

*超时默认动作为 **Deny**（可配置为 Approve）。总超时由 `server.runtime.timeout_secs` 控制。

#### 支持的 Agent 及 Hook 配置

**Claude Code** (`~/.claude/settings.json`):

```json
{
  "hooks": {
    "PreToolUse": [{"matcher": ".*", "hooks": [{"type": "command", "command": "winvibe-hookcli pre-tool-use"}]}],
    "PostToolUse": [{"matcher": ".*", "hooks": [{"type": "command", "command": "winvibe-hookcli post-tool-use"}]}],
    "Stop": [{"hooks": [{"type": "command", "command": "winvibe-hookcli stop"}]}],
    "Notification": [{"hooks": [{"type": "command", "command": "winvibe-hookcli notification"}]}]
  }
}
```

注意：

- WinVibe 随主程序分发一个轻量 CLI 客户端 `winvibe-hookcli`（< 2MB 静态二进制），由它负责：
  1. 生成 `approval_id` (UUID v4) 并加入 `X-Approval-Id` header
  2. 读取本地配置（端口 + token），无需在 hooks 中硬编码
  3. 实现 §2.3 短轮询 + 幂等重提交协议（pre-tool-use 专用，其他 hook 类型为 fire-and-forget）
  4. 处理断连重试、休眠唤醒恢复、信号转发
- **不再使用 curl** 直接拼装命令，原因：(a) curl 不支持幂等重提交逻辑；(b) Windows 默认无 curl；(c) 隐藏 token 不出现在 settings.json 命令行中（仅由 hookcli 从用户私有配置读取）
- 当 `port` 或 `auth_token` 变更时，hookcli 自动从配置文件读取最新值，**不需要重写** Agent settings；仅当 hookcli 路径改变时才触发重写

**Gemini CLI** (`~/.gemini/settings.json`): 类似结构，待验证

**Codex**: 待调研

### 2.4 自动配置（零配置接入）

**描述**: WinVibe 首次启动时，自动检测并配置已安装的 Agent hooks。

流程：

1. 检测系统中已安装的 Agent（检查 PATH、常见安装路径）
2. 生成随机认证 Token（32 字节 hex），保存到 WinVibe 配置
3. 提示用户：「检测到 Claude Code，是否自动配置 hooks？」
4. 用户确认后，写入对应配置文件（含 Token）
5. 提示需要重启 Agent 生效

配置写入安全策略：

- **安全写入**: 先写入临时文件并 fsync 落盘，再通过平台感知的 replace 替换目标文件，防止进程崩溃导致配置文件损坏
- **带时间戳备份**: 备份文件命名为 `settings.json.bak.20260408T120000`，保留最近 5 份
- **JSON merge**: 不覆盖用户现有配置，仅添加/更新 hooks 配置
- 提供「撤销配置」功能

### 2.5 终端跳转

**描述**: 一键将焦点切换到 Agent 运行所在的终端窗口/Tab。

#### 支持的终端

**MVP (Phase 1-2)**:

| 终端          | Windows | 跳转方式                                          |
| ----------- | ------- | --------------------------------------------- |
| 通用 Win32 窗口 | Yes     | 窗口句柄匹配（枚举窗口 → 匹配进程 CWD → SetForegroundWindow） |

**Spike 验证后接入**:

| 终端               | Windows | 跳转方式                                   | 状态    |
| ---------------- | ------- | -------------------------------------- | ----- |
| Windows Terminal | Yes     | COM API（需验证 ITerminalConnection 接口公开性） | spike |
| VS Code 集成终端     | Yes     | Extension API / 窗口标题匹配                 | spike |

**Phase 3 扩展**:

| 终端                       | 平台      | 跳转方式                 |
| ------------------------ | ------- | -------------------- |
| GNOME Terminal           | Linux   | DBUS                 |
| Kitty                    | Linux   | kitty @ focus-window |
| Alacritty                | Linux   | xdotool / wmctrl     |
| PowerShell 7             | Windows | 窗口句柄                 |
| WSL (Windows)            | Windows | 通过 Windows Terminal  |
| Windows Terminal Preview | Windows | COM API              |

#### 终端标识方案

Hook 事件中包含 `cwd`（工作目录），WinVibe 通过枚举当前用户的终端进程，匹配 CWD 找到对应终端窗口。

路径匹配规则：

- Windows: 大小写不敏感比较，统一路径分隔符
- WSL 路径自动转换: 通过 `wsl.exe -l -q` 动态枚举已注册发行版列表逐一尝试匹配（不硬编码发行版名称），`wsl.exe` 不可用时放弃 WSL 匹配
- 符号链接解析后比较
- 进程枚举结果缓存 5 秒，避免频繁扫描
- **Best effort**: `cwd` 为可选字段，缺失时返回明确错误提示，UI 可显示禁用的跳转按钮

### 2.6 全屏应用检测

**描述**: 自动检测前台是否有全屏应用运行，避免 HUD 遮挡。

行为：

| 场景              | HUD 行为 | 审批提醒方式           |
| --------------- | ------ | ---------------- |
| 正常桌面            | 正常显示   | HUD 闪烁 + 声音      |
| 全屏应用（游戏/PPT/会议） | 自动隐藏   | 系统 toast 通知 + 声音 |
| 全屏应用退出          | 自动恢复显示 | -                |

配置项：

- `fullscreen_auto_hide`: 启用/禁用全屏检测（默认启用）
- 白名单/黑名单模式（可选）

### 2.7 通知系统

**描述**: 多层级通知机制。

| 事件                | 通知方式               | 默认声音 |
| ----------------- | ------------------ | ---- |
| Agent 请求审批        | HUD 闪烁 + 声音 + 系统通知 | 提示音  |
| Agent 完成任务        | HUD 短暂高亮 + 声音      | 完成音  |
| Agent 报错          | HUD 红色 + 声音 + 系统通知 | 警告音  |
| Agent 发问          | HUD 闪烁 + 声音        | 问题音  |
| Agent 疑似崩溃（Stale） | HUD 灰色虚线 + 系统通知    | -    |

声音规格：

- 内置 8-bit 合成音效（4 种）
- 支持导入自定义 `.wav` / `.mp3` 文件
- 每类事件独立开关

#### 智能通知策略

根据用户当前焦点状态，动态调整通知行为，避免打断正在关注的会话：

| 场景       | HUD 行为     | 声音    | 系统通知     |
| -------- | ---------- | ----- | -------- |
| 前台会话有事件  | 静默更新状态，不闪烁 | 静音    | 不发送      |
| 后台会话完成   | HUD 短暂高亮   | 播放完成音 | 发送       |
| 后台会话报错   | HUD 红色     | 播放警告音 | 发送       |
| 后台会话请求审批 | HUD 强制闪烁   | 播放提示音 | 发送       |
| 全屏应用运行中  | 隐藏 HUD     | 播放    | 发送 toast |

**"前台会话"判定规则**:

判定分为两层，按精度递减自动降级：

1. **Tab 级精确匹配**（Windows Terminal COM API / VS Code Extension API）：获取前台窗口中当前活跃 Tab 的 CWD，与会话 CWD 精确比对。仅活跃 Tab 对应的会话被静默，同窗口内其他 Tab 的会话正常通知。
2. **降级策略**（通用后备）：当 Tab 级 API 不可用时，**不执行窗口级匹配**，直接视为后台会话并正常通知。降级方向是"宁可多通知，不漏通知"。
- 审批请求无论前台/后台均触发全量通知，不受降级影响
- 检测频率：每 2 秒一次（复用全屏检测的定时任务）

配置项：

- `notification.focus_aware`: 启用/禁用前台焦点感知（默认启用）
- `notification.focus_check_interval`: 焦点检测间隔秒数（默认 2）

### 2.8 配置管理

**描述**: 通过设置界面管理所有配置项。

#### 配置生效规则

配置项分为两类，在设置页面中明确标注：

| 类别        | 生效方式           | 包含配置                    |
| --------- | -------------- | ----------------------- |
| **启动期只读** | 修改后需重启 WinVibe | `port`、`auth_token`     |
| **即时生效**  | 修改后立即生效，无需重启   | 其他所有配置（HUD 外观、声音、超时时间等） |

> **原因**: `port` 和 `auth_token` 与 Hook Server 监听地址和认证中间件绑定，运行时修改需要重启 HTTP 服务并同步更新所有 Agent 的 hook 配置，风险较高。将其定义为启动期只读，避免各模块读到不一致的值。

```toml
# winvibe.toml

# ─── 启动期只读（修改后需重启 WinVibe）───
[server]
port = 59999                # Hook Server 监听端口
auth_token = "abc123..."    # 自动生成，不建议手动修改

# ─── 以下配置即时生效 ───
[server.runtime]
timeout_secs = 300          # PreToolUse 审批超时
timeout_action = "deny"     # 超时动作: approve | deny（默认 deny）

[hud]
position = "top-center"     # top-center | top-left | top-right
opacity = 0.92
always_on_top = true
auto_collapse_secs = 0      # 0 = 不自动折叠
fullscreen_auto_hide = true # 检测到全屏应用时自动隐藏 HUD

[sound]
enabled = true
approval_request = "default"
task_complete = "default"
error = "default"

[agents]
auto_configure = true       # 自动检测并配置 Agent hooks
tool_call_history = 50      # 每个 Agent 保留的工具调用历史数量

[session]
stale_timeout_secs = 600    # 超时标记为 Stale 的秒数（默认 600 = 10 分钟）
stale_cleanup_secs = 3600   # Stale 会话自动移除的秒数（默认 3600 = 1 小时）

[notification]
focus_aware = true           # 前台会话静默（默认启用）
focus_check_interval = 2     # 焦点检测间隔秒数

[usage]
enabled = true               # 启用用量监控
warning_threshold = 0.7      # 70% 时进度条变黄
critical_threshold = 0.9     # 90% 时进度条变红
default_session_budget = 0   # 会话预算上限（token 数），0 = 不限制

[mascot]
enabled = false              # 像素猫默认关闭
character = "pixel-cat"      # 角色选择（预留多角色）
```

### 2.9 优雅关闭

**描述**: WinVibe 关闭时安全处理所有 pending 状态。

流程：

1. 用户请求关闭 WinVibe
2. 检查是否有 pending 审批请求
3. 如果有 → 弹出确认框："当前有 N 个待审批请求，退出后将按默认策略（Deny）处理。确定退出？"
4. 用户确认后 → 对所有 pending 审批执行配置的默认动作（返回 HTTP 响应）
5. 停止接受新 hook 请求
6. flush 日志和审计记录
7. 进程退出

### 2.10 单实例运行

**描述**: 确保同一 OS 用户下只有一个 WinVibe 实例运行。

- **作用域**: 单 OS 用户单实例。多用户会话（Windows 快速用户切换 / RDP / Linux 多 seat）允许各自独立运行。
- **实现方案**:
  - Windows: named mutex `WinVibe_<user_sid>`
  - Linux: lock file 在 `$XDG_RUNTIME_DIR/winvibe.lock`（fallback `/tmp/winvibe-<uid>.lock`），`flock` 独占
- 启动时检测已有实例 → 通过 IPC 唤起已有实例窗口 → 新进程退出
- **端口冲突保护**: 不同用户实例使用相同默认端口 `59999` 时，监听失败方自动按 `port + uid_hash % 100` 偏移重试，并写回配置。由于 hookcli 每次执行都从用户配置读取端口，无需重写 Agent settings，端口偏移对已配置 Agent 透明生效

### 2.11 交互式提问响应

**描述**: 当 Agent 向用户提问时，在 HUD 中展示问题和选项按钮，用户点击后回传答案，无需切换到终端。

> **当前限制**: Claude Code 的 `AskUserQuestion` 不走 hook 系统——它直接在终端 stdin 等待用户输入。此功能需等待上游 Agent 开放新的 hook 类型（如 `UserQuestion`）后方可生效。当前仅预留接口定义和前端 UI 组件。其他 Agent（如 Gemini CLI）如果原生支持问题 hook，可优先接入。

#### 预留交互规格

```
┌──────────────────────────────────────────────────────────┐
│  ● Claude Code     fix auth bug            [跳转]   │
│  ─────────────────────────────────────────────────────   │
│  ❓ Agent 提问：                                         │
│  "Which library should we use for date formatting?"      │
│                                                          │
│  [date-fns]  [dayjs]  [moment]                          │
│                                                          │
│  [自定义回复...]                                          │
└──────────────────────────────────────────────────────────┘
```

- 问题文本支持 Markdown 渲染
- 选项以按钮形式排列，点击即回传
- 提供"自定义回复"文本输入框作为后备
- 超时行为：与审批超时一致，超时后按默认动作处理

#### 预留 Hook 协议

```
POST /v1/hook/question      # Agent 向用户提问（阻塞，等待回答）
```

请求体：

```json
{
  "session_id": "...",
  "question": "Which library should we use?",
  "options": ["date-fns", "dayjs", "moment"],
  "allow_custom": true
}
```

响应体：

```json
{
  "answer": "date-fns"
}
```

### 2.12 用量监控（MVP：调用计数 + 字符量，非 token）

**描述**: 实时追踪 Agent 会话的资源消耗，在 HUD 中以辅助信息形式展示。**MVP 阶段不展示伪 token 百分比**——独立架构评审指出"按字符数 / 4"对工具 payload（结构化 JSON、base64、CJK、缓存 token）误差远超 30-50%，会向用户传递误导性预警。

#### MVP 展示规格

折叠态：仅在 `default_session_budget` 显式设置时显示进度条，否则不展示用量字段。

展开态：

```
┌──────────────────────────────────────────────────┐
│  ● Claude Code   fix auth bug         ⤴ 跳转    │
│  ─────────────────────────────────────────────── │
│  调用次数：48 次工具调用                          │
│  累计字符：≈ 184 KB（输入）+ 92 KB（输出）         │
│  会话预算：未设置（在设置中配置）                  │
│  ─────────────────────────────────────────────── │
│  ...                                             │
└──────────────────────────────────────────────────┘
```

> **明确告知**: 用量页面顶部固定标签「📊 估算量级，非精确 token」，避免用户误读。

#### 颜色规则（仅在用户主动设置预算时生效）

预算单位由用户选择：`tool_calls` / `chars`。当无 `accountQuotaTotal` 且无 `default_session_budget` 时**不显示进度条**，仅显示原始计数。

| 消耗比例 | 颜色 |
|---|---|
| < 70% | `--wv-usage-normal`（绿） |
| 70-90% | `--wv-usage-warning`（黄） |
| > 90% | `--wv-usage-critical`（红） |

#### 数据模型

```typescript
type UsageAccuracy = "exact" | "derived" | "estimated" | "unknown";
type UsageSource = "official" | "post_tool_use" | "local_cache" | "manual_budget";
type BudgetUnit = "tokens" | "tool_calls" | "chars";

interface UsageSnapshot {
  sessionId: string;
  toolCallCount: number;          // MVP 主指标
  inputCharBytes: number;         // UTF-8 字节数
  outputCharBytes: number;        // UTF-8 字节数
  // 仅当 source = official | local_cache 时填充
  usedInputTokens?: number;
  usedOutputTokens?: number;
  accountQuotaUsed?: number;
  accountQuotaTotal?: number;
  resetAt?: string;
  source: UsageSource;
  accuracy: UsageAccuracy;
  budgetUnit?: BudgetUnit;
}
```

#### 数据来源优先级

| 优先级 | 来源 | accuracy | 说明 |
|---|---|---|---|
| 1 | official（Agent 官方 API） | exact | 最精确，需要额外授权，Phase 2 |
| 2 | local_cache（Agent 本地缓存文件） | derived | 零侵入读取，Phase 2 |
| 3 | post_tool_use（PostToolUse hook 累计） | estimated | **MVP 仅累计调用次数 + 字符字节数**，不推算 token |
| 4 | manual_budget（用户手动设定） | unknown | 仅提供预算上限 |

#### MVP 范围

- 仅累计 `toolCallCount`、`inputCharBytes`、`outputCharBytes`，不做任何 `bytes → tokens` 转换
- 仅支持会话级预算进度，不做账号额度查询
- 用户可在设置中配置 `default_session_budget` 与 `default_session_budget_unit`（默认单位 `tool_calls`）
- 字符量按紧凑 JSON 序列化的 UTF-8 字节数累计；CJK 按实际字节数（3 字节/字）
- **Phase 2 升级路径**: 当某 Agent hook payload 中出现标准化 token 字段（如 Claude Code `usage.input_tokens`）时，自动切换到 `source = post_tool_use, accuracy = exact` 并改用 token 单位

### 2.13 Mascot 彩蛋（像素猫）

**描述**: HUD 折叠态中可选展示的像素猫 mascot，与 Agent 状态联动，增加趣味性。默认关闭，在设置中开启。**演示模式自动隐藏**: 当全屏检测命中演示类应用（PowerPoint / Keynote / OBS Studio / Zoom 屏幕共享等，名单可配置）时强制隐藏，无视用户开关。

#### 状态映射

| Agent 状态/活动        | 猫咪状态     | 动画描述        |
| ------------------ | -------- | ----------- |
| IDLE               | Sleeping | 闭眼打盹，尾巴偶尔摆动 |
| RUNNING / Thinking | Looking  | 眼睛左右看，好奇表情  |
| RUNNING / Editing  | Typing   | 爪子敲键盘动作     |
| RUNNING / Reading  | Reading  | 眼睛跟着文字移动    |
| RUNNING / Testing  | Watching | 盯着屏幕，紧张表情   |
| WAITING_APPROVAL   | Poking   | 伸爪戳用户，催促表情  |
| DONE               | Happy    | 开心表情，尾巴竖起   |
| ERROR              | XEyes    | X 眼表情，倒地    |
| STALE              | Confused | 问号表情，歪头     |

#### 动画规格

- 格式：sprite sheet（PNG，单张图包含所有帧）
- 尺寸：每帧 ≤ 16×16 逻辑像素（适配 HUD 折叠态 36px 高度）
- 帧数：每状态 2-4 帧
- 帧率：4 FPS（低帧率省资源，像素风也更有味道）
- 渲染方式：CSS sprite animation（`steps()` timing function）

#### 性能预算

- sprite sheet 文件总大小 < 20KB
- 不增加额外 JS 依赖（纯 CSS 动画）
- 动画使用 `will-change: transform` 提示 GPU 合成，避免触发重排

---

## 3. 非功能需求

### 3.1 性能

| 指标         | 要求                          |
| ---------- | --------------------------- |
| 内存占用（空闲）   | < 50MB                      |
| 内存占用（运行中）  | < 80MB                      |
| Hook 响应延迟  | < 100ms（从 Agent 发送到 HUD 显示） |
| 启动时间       | < 2 秒                       |
| CPU 占用（空闲） | < 0.5%                      |

### 3.2 兼容性

| 平台      | 最低版本                     |
| ------- | ------------------------ |
| Windows | Windows 10 21H2          |
| Linux   | Ubuntu 22.04 / Fedora 38 |

#### 显示适配

- 支持 100%、125%、150%、200% DPI 缩放
- 支持多显示器环境（包括不同 DPI 的混合显示器）
- HUD 默认显示在主显示器，可配置跟随鼠标所在显示器
- 显示器热插拔时自动重新定位 HUD

### 3.3 安全性

#### 威胁模型与安全边界（坦白版）

WinVibe 的 Hook Server 运行在 `127.0.0.1`，安全边界**明确止于"同 OS 用户"**：

| 威胁 | 是否防护 | 机制 |
|---|---|---|
| 外部网络攻击 | ✅ 是 | 仅监听 loopback，不可远程访问 |
| 浏览器 CSRF / 恶意网页 | ✅ 是 | CORS 拒绝 Origin 非空请求 + Bearer Token 校验 |
| 其他 OS 用户的进程 | ✅ 是 | Token 存储在 0600 私有文件；socket/lock 在用户专属路径 |
| **同 OS 用户的恶意进程** | ❌ **否，不防御** | 见下方说明 |

##### 为什么不防御同用户进程

1. Token 必然要被同用户的 hookcli / Agent 读取，明文存在用户配置中
2. 即使引入 IPC HMAC 或临时 nonce，同用户进程可以：
   - 读取 0600 文件（同用户绕过文件权限）
   - attach/inject Tauri 主进程，dump 内存中的密钥
   - 直接调用 Tauri Command IPC 端点（Tauri 内置 IPC 不区分调用方进程身份）
3. 因此**任何"防同用户伪造"的机制都只是抬高门槛，不构成真实的安全边界**——文档若声称防御，会向用户传递虚假安全感

##### approval_id 的真实作用

`approval_id`（§2.3）**仅用于幂等匹配**，不是安全机制：

- 防止网络抖动 / 休眠唤醒后的审批结果丢失
- 防止同一审批被服务端重复处理
- **不防伪造**：同用户进程若已能调用 hook server，构造一条 PreToolUse 让用户 Approve 完全可行

##### 缓解策略（提高门槛而非根除）

虽然不能根除，但以下措施仍有价值：

- **审计日志记录调用方 PID + 进程路径**（可获取时），用户事后可追溯异常审批来源
- **审批 UI 显示请求来源进程信息**（PID / cwd / Agent 类型），让用户人眼识别可疑请求
- **Tauri IPC HMAC 握手**（Phase 2）：主进程启动时生成随机 session secret 写入 0600 文件，前端携带 HMAC 调用 Command。这不防内存 dump，但能阻挡"无脑调用 IPC 端点"的低门槛攻击
- 对 Approve 决策强制要求"用户活跃"信号（最近 N 秒有键盘/鼠标输入），防止后台进程在用户离开时触发审批 UI 并自动确认

> **设计约束**: 各 AI Agent 的 hook 机制仅支持 shell command，无法使用命名管道 / Unix domain socket 等强 IPC 方案。在此约束下，loopback HTTP + Bearer Token + 用户人眼审核是合理的安全上限。WinVibe **不是安全沙箱**，它是开发者工作流工具——用户运行的 Agent 本身已经能执行任意代码，WinVibe 不增加额外攻击面。

#### 安全措施

- Hook 服务仅监听 `127.0.0.1`（不对外暴露）
- **请求认证**: 所有 hook 请求必须携带 Bearer Token，**用于防御跨用户访问与浏览器 CSRF**，不防御同用户伪造
- **CORS 防护**: 拒绝 Origin 非空的请求
- **审批 UI 透明化**: 始终显示请求来源（Agent 类型 / cwd / 工具名 / PID 可获取时），由用户人眼识别异常请求
- 审批超时/异常时**默认拒绝**（可配置），遵循最小权限原则
- Token 文件 0600 权限（Windows 等价 ACL：仅当前用户可读）
- 进程枚举仅扫描当前用户拥有的进程，无需管理员权限
- **审计日志**: 所有审批决策（含超时/关闭自动处理）记录到独立审计日志，保留 30 天，字段包含 `approval_id` / `decided_by` / 调用方 PID（如可获取） / agent 类型 / tool 名

### 3.4 可靠性

- Hook Server 崩溃后自动恢复（看门狗机制，连续 3 次健康检查失败则重启）
- 审批请求不丢失（Sender 存储在 HashMap 而非通过可丢弃消息传递）
- Agent 非正常退出时会话自动标记为 Stale 并最终清理
- 配置文件原子写入，防止断电导致损坏
- 关闭前安全处理所有 pending 审批

---

## 4. 不做什么（范围外）

- **不做 Agent 本身** —— 只监控，不执行
- **不做 macOS 支持** —— 已有 Vibe Island
- **不做云同步** —— 纯本地运行
- **不做移动端** —— 桌面开发场景
- **不做 Agent 插件热加载（MVP）** —— 新 Agent 通过编译时 Adapter 支持
- **不做账号额度查询（MVP）** —— MVP 只做会话级消耗推算，不调用外部 API 查询账号余额

---

## 5. 开源策略

- **License**: MIT
- **发布平台**: GitHub
- **安装方式**: GitHub Releases 提供预编译二进制（`.exe` installer / AppImage / `.deb`）
- **社区**: GitHub Issues + Discussions
- **Agent Adapter 贡献**: 社区可通过实现 `AgentAdapter` trait 添加新 Agent 支持
