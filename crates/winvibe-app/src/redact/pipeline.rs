use serde::Serialize;
use winvibe_core::approval::types::{Approval, ApprovalState};
use winvibe_core::protocol::Decision;

/// IPC 安全的审批数据，仅含 HUD 前端所需字段
///
/// 排除的内部字段：
/// - tool_input_raw_sha256, tool_input_canonical_sha256（内部哈希）
/// - tool_input_original_bytes（内部度量）
/// - created_mono_ms, expires_at_mono_ms（内部单调时钟）
/// - trace_id, approval_entry_span_id（内部追踪）
/// - decided_mono_ms, decision_trace_id（内部追踪）
#[derive(Debug, Clone, Serialize)]
pub struct RedactedApproval {
    pub id: String,
    pub session_hash: String,
    pub tool_name: String,
    pub fingerprint: String,
    pub fingerprint_version: u8,
    /// "pending" 或 "decided"
    pub state: String,
    /// "Approved" / "Denied" / "TimedOut" / "Cancelled"，Pending 时为 None
    pub decision_kind: Option<String>,
    pub feedback: Option<String>,
    /// ISO 8601 / RFC 3339 格式
    pub created_wall: String,
}

/// 将内部 Approval 转换为 IPC 安全的 RedactedApproval
pub fn redact_approval_for_ipc(approval: &Approval) -> RedactedApproval {
    let (state, decision_kind, feedback) = match &approval.state {
        ApprovalState::Pending => ("pending".to_string(), None, None),
        ApprovalState::Decided { decision, .. } => {
            let (kind, fb) = match decision {
                Decision::Approved { feedback } => ("Approved", feedback.clone()),
                Decision::Denied { feedback } => ("Denied", feedback.clone()),
                Decision::TimedOut => ("TimedOut", None),
                Decision::Cancelled { .. } => ("Cancelled", None),
            };
            ("decided".to_string(), Some(kind.to_string()), fb)
        }
    };

    let created_wall = approval
        .created_wall
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();

    RedactedApproval {
        id: approval.id.to_string(),
        session_hash: approval.session_hash.clone(),
        tool_name: approval.tool_name.clone(),
        fingerprint: approval.fingerprint.clone(),
        fingerprint_version: approval.fingerprint_version,
        state,
        decision_kind,
        feedback,
        created_wall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winvibe_core::approval::types::{Approval, ApprovalState};
    use winvibe_core::protocol::{ApprovalId, Decision};
    use winvibe_core::trace::{TraceId, SpanId};

    fn make_test_approval() -> Approval {
        Approval {
            id: ApprovalId::new(),
            session_hash: "abcd1234abcd1234".to_string(),
            tool_name: "Bash".to_string(),
            fingerprint: "c".repeat(64),
            fingerprint_version: 1,
            tool_input_raw_sha256: "a".repeat(64),
            tool_input_canonical_sha256: "b".repeat(64),
            tool_input_original_bytes: 42,
            created_wall: time::OffsetDateTime::now_utc(),
            created_mono_ms: 1000,
            expires_at_mono_ms: 301000,
            state: ApprovalState::Pending,
            trace_id: TraceId::generate(),
            approval_entry_span_id: SpanId::generate(),
        }
    }

    #[test]
    fn redact_preserves_session_hash() {
        let approval = make_test_approval();
        let redacted = redact_approval_for_ipc(&approval);
        assert_eq!(redacted.session_hash, "abcd1234abcd1234");
    }

    #[test]
    fn redact_preserves_fingerprint() {
        let approval = make_test_approval();
        let redacted = redact_approval_for_ipc(&approval);
        assert_eq!(redacted.fingerprint, "c".repeat(64));
    }

    #[test]
    fn redact_preserves_tool_name() {
        let approval = make_test_approval();
        let redacted = redact_approval_for_ipc(&approval);
        assert_eq!(redacted.tool_name, "Bash");
    }

    #[test]
    fn redact_omits_internal_fields() {
        let approval = make_test_approval();
        let redacted = redact_approval_for_ipc(&approval);
        let json = serde_json::to_value(&redacted).unwrap();
        // 内部追踪字段不应出现
        assert!(json.get("trace_id").is_none());
        assert!(json.get("approval_entry_span_id").is_none());
        assert!(json.get("tool_input_raw_sha256").is_none());
        assert!(json.get("tool_input_canonical_sha256").is_none());
        assert!(json.get("created_mono_ms").is_none());
        assert!(json.get("expires_at_mono_ms").is_none());
    }

    #[test]
    fn redact_pending_state() {
        let approval = make_test_approval();
        let redacted = redact_approval_for_ipc(&approval);
        assert_eq!(redacted.state, "pending");
        assert!(redacted.decision_kind.is_none());
    }

    #[test]
    fn redact_decided_approved() {
        let mut approval = make_test_approval();
        approval.state = ApprovalState::Decided {
            decision: Decision::Approved { feedback: Some("lgtm".to_string()) },
            decided_wall: time::OffsetDateTime::now_utc(),
            decided_mono_ms: 2000,
            decision_trace_id: TraceId::generate(),
        };
        let redacted = redact_approval_for_ipc(&approval);
        assert_eq!(redacted.state, "decided");
        assert_eq!(redacted.decision_kind.as_deref(), Some("Approved"));
        assert_eq!(redacted.feedback.as_deref(), Some("lgtm"));
    }

    #[test]
    fn redact_decided_denied() {
        let mut approval = make_test_approval();
        approval.state = ApprovalState::Decided {
            decision: Decision::Denied { feedback: None },
            decided_wall: time::OffsetDateTime::now_utc(),
            decided_mono_ms: 2000,
            decision_trace_id: TraceId::generate(),
        };
        let redacted = redact_approval_for_ipc(&approval);
        assert_eq!(redacted.decision_kind.as_deref(), Some("Denied"));
        assert!(redacted.feedback.is_none());
    }

    #[test]
    fn redact_created_wall_is_rfc3339() {
        let approval = make_test_approval();
        let redacted = redact_approval_for_ipc(&approval);
        // RFC 3339 格式包含 'T' 分隔符
        assert!(redacted.created_wall.contains('T'), "created_wall 应为 RFC 3339 格式");
    }
}
