use crate::exit_code::ExitCode;
use crate::http_client::{HookClient, ServerResponse};
use crate::trace_ctx::{acquire_or_create_trace, new_span_traceparent};
use std::io::Read;
use winvibe_core::config::TimeoutAction;
use winvibe_core::protocol::{ApprovalId, PreToolUsePayload};

/// 最大 stdin 读取字节数（1 MiB + 1 字节用于溢出检测）
const MAX_STDIN_BYTES: usize = 1024 * 1024 + 1;

/// 将服务端 Decision JSON 映射为 Claude Code hook JSON
pub fn map_decision_to_hook_json(
    decision: &serde_json::Value,
    timeout_action: TimeoutAction,
) -> serde_json::Value {
    let kind = decision.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    match kind {
        "Approved" => serde_json::json!({
            "decision": "approve",
            "reason": "winvibe: approved"
        }),
        "Denied" => serde_json::json!({
            "decision": "block",
            "reason": "winvibe: denied"
        }),
        "TimedOut" => {
            let (decision_str, reason) = match timeout_action {
                TimeoutAction::Approve => ("approve", "winvibe: timed_out"),
                TimeoutAction::Deny => ("block", "winvibe: timed_out"),
            };
            serde_json::json!({
                "decision": decision_str,
                "reason": reason
            })
        }
        "Cancelled" => {
            let (decision_str, reason) = match timeout_action {
                TimeoutAction::Approve => ("approve", "winvibe: cancelled"),
                TimeoutAction::Deny => ("block", "winvibe: cancelled"),
            };
            serde_json::json!({
                "decision": decision_str,
                "reason": reason
            })
        }
        other => serde_json::json!({
            "decision": "block",
            "reason": format!("winvibe: unknown decision kind '{other}'")
        }),
    }
}

/// 从 stdin 读取 PreToolUsePayload（最大 1 MiB）
pub fn read_stdin_payload() -> Result<PreToolUsePayload, ExitCode> {
    let stdin = std::io::stdin();
    let mut buf = Vec::with_capacity(4096);
    stdin
        .lock()
        .take(MAX_STDIN_BYTES as u64)
        .read_to_end(&mut buf)
        .map_err(|e| {
            eprintln!("winvibe-hookcli: stdin read error: {e}");
            ExitCode::FailClosed
        })?;

    if buf.len() >= MAX_STDIN_BYTES {
        eprintln!("winvibe-hookcli: stdin payload exceeds 1 MiB limit");
        return Err(ExitCode::FailClosed);
    }

    serde_json::from_slice::<PreToolUsePayload>(&buf).map_err(|e| {
        eprintln!("winvibe-hookcli: failed to parse stdin JSON: {e}");
        ExitCode::FailClosed
    })
}

/// 执行 PreToolUse 主流程：submit → poll 循环 → stdout
pub fn run_pre_tool_use(
    client: &HookClient,
    payload: &PreToolUsePayload,
    timeout_action: TimeoutAction,
    max_time_secs: u64,
) -> ExitCode {
    let approval_id = ApprovalId::new();
    let trace = acquire_or_create_trace();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(max_time_secs);

    // 序列化 payload 为 JSON
    let payload_json = match serde_json::to_value(payload) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("winvibe-hookcli: failed to serialize payload: {e}");
            return ExitCode::FailClosed;
        }
    };

    // submit 请求
    let traceparent = new_span_traceparent(&trace);
    let resp = match client.submit(&approval_id, &payload_json, &traceparent) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("winvibe-hookcli: submit error: {e:?}");
            return ExitCode::FailClosed;
        }
    };

    // 处理 submit 响应
    match resp {
        ServerResponse::Decided { decision, .. } => {
            return emit_decision(&decision, timeout_action);
        }
        ServerResponse::Pending { .. } => {
            // 进入 poll 循环
        }
    }

    // poll 循环
    loop {
        if std::time::Instant::now() >= deadline {
            eprintln!("winvibe-hookcli: approval timed out after {max_time_secs} seconds");
            return ExitCode::FailClosed;
        }

        std::thread::sleep(std::time::Duration::from_millis(500));

        let traceparent = new_span_traceparent(&trace);
        let resp = match client.poll(&approval_id, &traceparent) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("winvibe-hookcli: poll error: {e:?}");
                return ExitCode::FailClosed;
            }
        };

        match resp {
            ServerResponse::Decided { decision, .. } => {
                return emit_decision(&decision, timeout_action);
            }
            ServerResponse::Pending { .. } => {
                // 继续等待
            }
        }
    }
}

/// 将 decision 写入 stdout 并返回 ProtocolSuccess
fn emit_decision(decision: &serde_json::Value, timeout_action: TimeoutAction) -> ExitCode {
    let hook_json = map_decision_to_hook_json(decision, timeout_action);
    match serde_json::to_string(&hook_json) {
        Ok(s) => {
            println!("{s}");
            ExitCode::ProtocolSuccess
        }
        Err(e) => {
            eprintln!("winvibe-hookcli: failed to serialize hook JSON: {e}");
            ExitCode::FailClosed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winvibe_core::config::TimeoutAction;

    #[test]
    fn map_decision_approved() {
        let decision = serde_json::json!({ "kind": "Approved", "feedback": null });
        let result = map_decision_to_hook_json(&decision, TimeoutAction::Deny);
        assert_eq!(result["decision"], "approve");
        assert_eq!(result["reason"], "winvibe: approved");
    }

    #[test]
    fn map_decision_denied() {
        let decision = serde_json::json!({ "kind": "Denied", "feedback": null });
        let result = map_decision_to_hook_json(&decision, TimeoutAction::Deny);
        assert_eq!(result["decision"], "block");
        assert_eq!(result["reason"], "winvibe: denied");
    }

    #[test]
    fn map_decision_timed_out_default_deny() {
        let decision = serde_json::json!({ "kind": "TimedOut" });
        let result = map_decision_to_hook_json(&decision, TimeoutAction::Deny);
        assert_eq!(result["decision"], "block");
        assert_eq!(result["reason"], "winvibe: timed_out");
    }

    #[test]
    fn map_decision_timed_out_configured_approve() {
        let decision = serde_json::json!({ "kind": "TimedOut" });
        let result = map_decision_to_hook_json(&decision, TimeoutAction::Approve);
        assert_eq!(result["decision"], "approve");
        assert_eq!(result["reason"], "winvibe: timed_out");
    }

    #[test]
    fn map_decision_cancelled_default_deny() {
        let decision = serde_json::json!({ "kind": "Cancelled", "reason": "StopHook" });
        let result = map_decision_to_hook_json(&decision, TimeoutAction::Deny);
        assert_eq!(result["decision"], "block");
        assert_eq!(result["reason"], "winvibe: cancelled");
    }
}
