use serde::Serialize;
use tauri::State;
use winvibe_core::protocol::ApprovalId;
use winvibe_core::trace::{TraceCtx, TraceSource};

use crate::app_state::AppState;
use crate::hud_decision::HudDecision;
use crate::ipc_error::IpcError;
use crate::redact::{redact_approval_for_ipc, RedactedApproval};

/// IPC 安全的审批列表快照，发送给 HUD 前端
#[derive(Debug, Clone, Serialize)]
pub struct RedactedSnapshot {
    /// 当前活跃的审批（若无则为 None）
    pub active: Option<RedactedApproval>,
    /// 已缓存的历史审批列表
    pub cached: Vec<RedactedApproval>,
    /// 快照版本号，用于前端检测变更
    pub revision: u64,
}

/// 获取当前审批列表快照
///
/// 返回经过脱敏处理的快照，仅含 HUD 前端所需字段
#[tauri::command]
pub async fn snapshot(state: State<'_, AppState>) -> Result<RedactedSnapshot, IpcError> {
    // 从运行时获取原始快照
    let raw = state.runtime.snapshot().await;

    // 对 active 和 cached 分别进行脱敏处理
    let active = raw.active.as_ref().map(redact_approval_for_ipc);
    let cached = raw.cached.iter().map(redact_approval_for_ipc).collect();

    Ok(RedactedSnapshot {
        active,
        cached,
        revision: raw.revision,
    })
}

/// 对指定审批提交决策（批准或拒绝）
///
/// - `id`：审批 ID 字符串，需能解析为 ApprovalId
/// - `decision`：HUD 前端传入的决策数据
#[tauri::command]
pub async fn decide(
    id: String,
    decision: HudDecision,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    // 解析审批 ID
    let approval_id = id
        .parse::<ApprovalId>()
        .map_err(|e| IpcError::from_code("invalid_id", e.to_string()))?;

    // 构建追踪上下文，标记来源为 HUD IPC
    let trace = TraceCtx::new(TraceSource::HudIpc);

    // 将 HUD 决策转换为核心协议决策
    let core_decision = decision.to_decision();

    // 提交决策到运行时
    state
        .runtime
        .decide(trace, approval_id, core_decision)
        .await
        .map_err(|e| IpcError::from_code("decide_failed", e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 RedactedSnapshot 可以正确序列化为 JSON，包含所有必要字段
    #[test]
    fn redacted_snapshot_serializes_correctly() {
        let snapshot = RedactedSnapshot {
            active: None,
            cached: vec![],
            revision: 42,
        };

        let json = serde_json::to_value(&snapshot).unwrap();

        // 验证三个必要字段均存在
        assert!(json.get("active").is_some(), "应包含 active 字段");
        assert!(json.get("cached").is_some(), "应包含 cached 字段");
        assert!(json.get("revision").is_some(), "应包含 revision 字段");

        // 验证字段值正确
        assert!(json["active"].is_null());
        assert!(json["cached"].as_array().unwrap().is_empty());
        assert_eq!(json["revision"], 42);
    }

    /// 验证 RedactedSnapshot 含有 active 时序列化正确
    #[test]
    fn redacted_snapshot_with_active_serializes_correctly() {
        let active = RedactedApproval {
            id: "test-id".to_string(),
            session_hash: "abcd1234".to_string(),
            tool_name: "Bash".to_string(),
            fingerprint: "f".repeat(64),
            fingerprint_version: 1,
            state: "pending".to_string(),
            decision_kind: None,
            feedback: None,
            created_wall: "2024-01-01T00:00:00Z".to_string(),
        };

        let snapshot = RedactedSnapshot {
            active: Some(active),
            cached: vec![],
            revision: 1,
        };

        let json = serde_json::to_value(&snapshot).unwrap();
        assert!(json["active"].is_object());
        assert_eq!(json["active"]["tool_name"], "Bash");
        assert_eq!(json["revision"], 1);
    }
}
