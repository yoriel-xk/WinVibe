// winvibe-e2e: headless 端到端测试
// 验证链路: winvibe-hookcli 二进制 → winvibe-hook-server → AppLifecycleSink → audit JSONL 落盘

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use serde_json::Value;
    use tempfile::TempDir;
    use tokio::time::timeout;

    use winvibe_app::audit::{AuditSink, JsonlAuditSink};
    use winvibe_app::diagnostic::DiagnosticSink;
    use winvibe_app::lifecycle_sink::AppLifecycleSink;
    use winvibe_core::approval::types::ApprovalStoreLimits;
    use winvibe_core::clock::{RealMonotonicClock, RealWallClock};
    use winvibe_core::protocol::{ApprovalId, CancelReason, Decision};
    use winvibe_core::trace::{SystemTraceSource, TraceCtx, TraceSource};
    use winvibe_hook_server::runtime::ApprovalRuntime;
    use winvibe_hook_server::server::ServerHandle;
    use winvibe_hook_server::sink::ApprovalLifecycleSink;

    // ─── 工具函数 ─────────────────────────────────────────────────────────────

    fn hookcli_bin() -> PathBuf {
        if let Ok(p) = std::env::var("CARGO_BIN_EXE_winvibe-hookcli") {
            return PathBuf::from(p);
        }
        let manifest = std::env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR not set");
        PathBuf::from(&manifest)
            .parent().unwrap() // crates/
            .parent().unwrap() // workspace root
            .join("target")
            .join("debug")
            .join("winvibe-hookcli.exe")
    }

    fn write_hookcli_config(dir: &Path, port: u16, token: &str) -> PathBuf {
        let path = dir.join("winvibe.toml");
        let content = format!(
            "bind = \"127.0.0.1\"\nport = \"{port}\"\nauth_token = \"{token}\"\napproval_ttl_ms = 300000\nmax_cached = 64\n"
        );
        std::fs::write(&path, content).expect("写入 hookcli config 失败");
        path
    }

    fn make_payload_json(session_id: &str, tool_name: &str) -> String {
        serde_json::json!({
            "session_id": session_id,
            "tool_name": tool_name,
            "tool_input": { "command": "echo hello" }
        })
        .to_string()
    }

    async fn start_server(
        token: &str,
        lifecycle_sink: Arc<dyn ApprovalLifecycleSink>,
    ) -> (Arc<ApprovalRuntime>, Arc<ServerHandle>, u16) {
        let wall = Arc::new(RealWallClock);
        let mono = Arc::new(RealMonotonicClock::new());
        let limits = ApprovalStoreLimits { max_active: 1, max_cached: 64 };
        let runtime = Arc::new(ApprovalRuntime::new(
            limits, wall, mono, lifecycle_sink, 300_000,
        ));
        let handle = ServerHandle::start("127.0.0.1:0", Arc::clone(&runtime), token.to_string())
            .await
            .expect("HTTP server 启动失败");
        let port = handle.local_addr().port();
        (runtime, handle, port)
    }

    fn run_hookcli(config_path: &Path, payload_json: &str, traceparent: Option<&str>) -> (i32, String) {
        let bin = hookcli_bin();
        assert!(bin.exists(), "hookcli 二进制不存在: {bin:?}\n请先运行 cargo build -p winvibe-hookcli");

        let mut cmd = std::process::Command::new(&bin);
        cmd.arg("--config").arg(config_path);
        cmd.arg("pre-tool-use");
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if let Some(tp) = traceparent {
            cmd.env("TRACEPARENT", tp);
        }

        let mut child = cmd.spawn().expect("hookcli 启动失败");
        child.stdin.as_mut().unwrap()
            .write_all(payload_json.as_bytes())
            .expect("写 stdin 失败");
        drop(child.stdin.take());

        let output = child.wait_with_output().expect("等待 hookcli 输出失败");
        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        (exit_code, stdout)
    }

    async fn poll_audit_record(audit_dir: &Path, approval_id_str: &str) -> Value {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if tokio::time::Instant::now() > deadline {
                panic!("poll_audit_record 超时: id={approval_id_str}");
            }
            if let Some(v) = try_find_audit_record(audit_dir, approval_id_str) {
                return v;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn try_find_audit_record(audit_dir: &Path, approval_id_str: &str) -> Option<Value> {
        let entries = std::fs::read_dir(audit_dir).ok()?;
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let content = std::fs::read_to_string(entry.path()).ok()?;
            for line in content.lines() {
                if line.is_empty() { continue; }
                if let Ok(v) = serde_json::from_str::<Value>(line) {
                    if v["approval_id"].as_str() == Some(approval_id_str) {
                        return Some(v);
                    }
                }
            }
        }
        None
    }

    /// 轮询等待第一个 Pending 审批入队，返回其 ApprovalId
    async fn wait_for_pending_approval(runtime: &ApprovalRuntime, max_wait: Duration) -> ApprovalId {
        let deadline = tokio::time::Instant::now() + max_wait;
        loop {
            if tokio::time::Instant::now() > deadline {
                panic!("wait_for_pending_approval 超时");
            }
            let snap = runtime.snapshot().await;
            if let Some(approval) = snap.active {
                if approval.is_pending() {
                    return approval.id;
                }
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    }

    // ─── 测试 1: Approved happy path ──────────────────────────────────────────

    #[tokio::test]
    async fn e2e_approved_happy_path() {
        let tmp = TempDir::new().unwrap();
        let audit_dir = tmp.path().join("audit");
        let token = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        let audit_sink: Arc<dyn AuditSink> = Arc::new(JsonlAuditSink::new(audit_dir.clone()));
        let diag_sink = Arc::new(DiagnosticSink::new(tmp.path().join("diag"), false));
        let lifecycle_sink: Arc<dyn ApprovalLifecycleSink> =
            Arc::new(AppLifecycleSink::new(None, Arc::clone(&audit_sink), diag_sink));

        let (runtime, server_handle, port) = start_server(token, lifecycle_sink).await;
        let config_path = write_hookcli_config(tmp.path(), port, token);
        let payload = make_payload_json(&uuid::Uuid::new_v4().to_string(), "Bash");

        let cfg = config_path.clone();
        let pl = payload.clone();
        let hookcli_task = tokio::task::spawn_blocking(move || run_hookcli(&cfg, &pl, None));

        let approval_id = wait_for_pending_approval(&runtime, Duration::from_secs(5)).await;
        runtime.decide(
            TraceCtx::new(TraceSource::HudIpc),
            approval_id,
            Decision::Approved { feedback: None },
        ).await.expect("decide 失败");

        let (exit_code, stdout) = timeout(Duration::from_secs(10), hookcli_task)
            .await.expect("hookcli 超时").expect("join 失败");

        assert_eq!(exit_code, 0, "stdout={stdout}");
        let hook_json: Value = serde_json::from_str(stdout.trim()).expect("stdout 非 JSON");
        assert_eq!(hook_json["decision"], "approve");

        let record = poll_audit_record(&audit_dir, &approval_id.to_string()).await;
        assert_eq!(record["schema"], "winvibe.audit.v1");
        assert_eq!(record["decision"]["kind"], "Approved");

        let _ = server_handle.shutdown().await;
        let _ = audit_sink.shutdown().await;
    }

    // ─── 测试 2: Denied happy path ────────────────────────────────────────────

    #[tokio::test]
    async fn e2e_denied_happy_path() {
        let tmp = TempDir::new().unwrap();
        let audit_dir = tmp.path().join("audit");
        let token = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

        let audit_sink: Arc<dyn AuditSink> = Arc::new(JsonlAuditSink::new(audit_dir.clone()));
        let diag_sink = Arc::new(DiagnosticSink::new(tmp.path().join("diag"), false));
        let lifecycle_sink: Arc<dyn ApprovalLifecycleSink> =
            Arc::new(AppLifecycleSink::new(None, Arc::clone(&audit_sink), diag_sink));

        let (runtime, server_handle, port) = start_server(token, lifecycle_sink).await;
        let config_path = write_hookcli_config(tmp.path(), port, token);
        let payload = make_payload_json(&uuid::Uuid::new_v4().to_string(), "Write");

        let cfg = config_path.clone();
        let pl = payload.clone();
        let hookcli_task = tokio::task::spawn_blocking(move || run_hookcli(&cfg, &pl, None));

        let approval_id = wait_for_pending_approval(&runtime, Duration::from_secs(5)).await;
        runtime.decide(
            TraceCtx::new(TraceSource::HudIpc),
            approval_id,
            Decision::Denied { feedback: None },
        ).await.expect("decide 失败");

        let (exit_code, stdout) = timeout(Duration::from_secs(10), hookcli_task)
            .await.expect("hookcli 超时").expect("join 失败");

        assert_eq!(exit_code, 0, "stdout={stdout}");
        let hook_json: Value = serde_json::from_str(stdout.trim()).expect("stdout 非 JSON");
        assert_eq!(hook_json["decision"], "block");

        let record = poll_audit_record(&audit_dir, &approval_id.to_string()).await;
        assert_eq!(record["schema"], "winvibe.audit.v1");
        assert_eq!(record["decision"]["kind"], "Denied");

        let _ = server_handle.shutdown().await;
        let _ = audit_sink.shutdown().await;
    }

    // ─── 测试 3: Cancelled path ───────────────────────────────────────────────

    #[tokio::test]
    async fn e2e_cancelled_path() {
        let tmp = TempDir::new().unwrap();
        let audit_dir = tmp.path().join("audit");
        let token = "cccccccccccccccccccccccccccccccccc";

        let audit_sink: Arc<dyn AuditSink> = Arc::new(JsonlAuditSink::new(audit_dir.clone()));
        let diag_sink = Arc::new(DiagnosticSink::new(tmp.path().join("diag"), false));
        let lifecycle_sink: Arc<dyn ApprovalLifecycleSink> =
            Arc::new(AppLifecycleSink::new(None, Arc::clone(&audit_sink), diag_sink));

        let (runtime, server_handle, port) = start_server(token, lifecycle_sink).await;
        let config_path = write_hookcli_config(tmp.path(), port, token);
        let payload = make_payload_json(&uuid::Uuid::new_v4().to_string(), "Bash");

        let cfg = config_path.clone();
        let pl = payload.clone();
        let hookcli_task = tokio::task::spawn_blocking(move || run_hookcli(&cfg, &pl, None));

        let approval_id = wait_for_pending_approval(&runtime, Duration::from_secs(5)).await;
        runtime.cancel_all_pending(
            TraceCtx::new(TraceSource::System(SystemTraceSource::AppExitCancel)),
            CancelReason::StopHook,
        ).await;

        let (exit_code, stdout) = timeout(Duration::from_secs(10), hookcli_task)
            .await.expect("hookcli 超时").expect("join 失败");

        // timeout_action 默认 Deny → Cancelled 映射为 block, exit 0
        assert_eq!(exit_code, 0, "stdout={stdout}");
        let hook_json: Value = serde_json::from_str(stdout.trim()).expect("stdout 非 JSON");
        assert_eq!(hook_json["decision"], "block");

        let record = poll_audit_record(&audit_dir, &approval_id.to_string()).await;
        assert_eq!(record["schema"], "winvibe.audit.v1");
        assert_eq!(record["decision"]["kind"], "Cancelled");
        assert_eq!(record["decision"]["cancel_reason"], "StopHook");

        let _ = server_handle.shutdown().await;
        let _ = audit_sink.shutdown().await;
    }

    // ─── 测试 4: Trace 字段完整性 ──────────────────────────────────────────────
    // 验证每条 audit 记录都携带合法的 approval_trace_id 与 decision_trace_id（32 hex），
    // 并且两者为不同的 trace（approval trace 来自 hookcli，decision trace 来自 HUD decide）。

    #[tokio::test]
    async fn e2e_trace_fields_present() {
        let tmp = TempDir::new().unwrap();
        let audit_dir = tmp.path().join("audit");
        let token = "dddddddddddddddddddddddddddddddddd";

        let audit_sink: Arc<dyn AuditSink> = Arc::new(JsonlAuditSink::new(audit_dir.clone()));
        let diag_sink = Arc::new(DiagnosticSink::new(tmp.path().join("diag"), false));
        let lifecycle_sink: Arc<dyn ApprovalLifecycleSink> =
            Arc::new(AppLifecycleSink::new(None, Arc::clone(&audit_sink), diag_sink));

        let (runtime, server_handle, port) = start_server(token, lifecycle_sink).await;
        let config_path = write_hookcli_config(tmp.path(), port, token);
        let payload = make_payload_json(&uuid::Uuid::new_v4().to_string(), "Read");

        // 注入固定 traceparent，让 hookcli 使用指定 trace_id 提交
        let approval_trace_id_hex = "0af7651916cd43dd8448eb211c803abc";
        let span_id_hex = "b7ad6b7169203331";
        let traceparent = format!("00-{approval_trace_id_hex}-{span_id_hex}-01");

        let cfg = config_path.clone();
        let pl = payload.clone();
        let tp = traceparent.clone();
        let hookcli_task = tokio::task::spawn_blocking(move || run_hookcli(&cfg, &pl, Some(&tp)));

        let approval_id = wait_for_pending_approval(&runtime, Duration::from_secs(5)).await;

        // 用独立的 decision trace 来决策
        let decision_trace = TraceCtx::new(TraceSource::HudIpc);
        runtime.decide(
            decision_trace,
            approval_id,
            Decision::Approved { feedback: None },
        ).await.expect("decide 失败");

        let (exit_code, stdout) = timeout(Duration::from_secs(10), hookcli_task)
            .await.expect("hookcli 超时").expect("join 失败");

        assert_eq!(exit_code, 0, "stdout={stdout}");

        let record = poll_audit_record(&audit_dir, &approval_id.to_string()).await;

        // approval_trace_id 必须是合法的 32 hex
        let approval_trace = record["approval_trace_id"].as_str().unwrap_or("");
        assert_eq!(approval_trace.len(), 32, "approval_trace_id 长度异常: {approval_trace}");
        assert!(approval_trace.chars().all(|c| c.is_ascii_hexdigit()),
            "approval_trace_id 非 hex: {approval_trace}");

        // decision_trace_id 必须是合法的 32 hex，且与 approval_trace_id 不同
        let decision_trace_field = record["decision_trace_id"].as_str().unwrap_or("");
        assert_eq!(decision_trace_field.len(), 32, "decision_trace_id 长度异常: {decision_trace_field}");
        assert_ne!(approval_trace, decision_trace_field,
            "approval_trace_id 与 decision_trace_id 不应相同");

        let _ = server_handle.shutdown().await;
        let _ = audit_sink.shutdown().await;
    }
}
