use crate::protocol::{ApprovalId, Decision};
use crate::trace::{TraceId, SpanId};
use serde::{Serialize, Deserialize};

/// 审批请求的核心实体，追踪从 Pending 到 Decided 的生命周期
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub id: ApprovalId,
    pub session_hash: String,
    pub tool_name: String,
    pub fingerprint: String,
    pub fingerprint_version: u8,
    pub tool_input_raw_sha256: String,
    pub tool_input_canonical_sha256: String,
    pub tool_input_original_bytes: usize,
    pub created_wall: time::OffsetDateTime,
    pub created_mono_ms: u64,
    pub expires_at_mono_ms: u64,
    pub state: ApprovalState,
    pub trace_id: TraceId,
    pub approval_entry_span_id: SpanId,
}

impl Approval {
    /// 审批是否处于待决状态
    pub fn is_pending(&self) -> bool {
        matches!(self.state, ApprovalState::Pending)
    }

    /// 审批是否已决策
    pub fn is_decided(&self) -> bool {
        matches!(self.state, ApprovalState::Decided { .. })
    }

    /// 获取决策结果，未决策时返回 None
    pub fn decision(&self) -> Option<&Decision> {
        match &self.state {
            ApprovalState::Decided { decision, .. } => Some(decision),
            _ => None,
        }
    }
}

/// 审批状态枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalState {
    Pending,
    Decided {
        decision: Decision,
        decided_wall: time::OffsetDateTime,
        decided_mono_ms: u64,
        decision_trace_id: TraceId,
    },
}

/// 审批存储容量限制
#[derive(Debug, Clone)]
pub struct ApprovalStoreLimits {
    pub max_active: usize,
    pub max_cached: usize,
}

impl Default for ApprovalStoreLimits {
    fn default() -> Self {
        Self { max_active: 1, max_cached: 64 }
    }
}

/// 入队操作的结果
#[derive(Debug)]
pub enum EnqueueOutcome {
    Created { approval_id: ApprovalId, revision: u64 },
    Existing { approval_id: ApprovalId, revision: u64 },
}

/// 入队操作的错误类型
#[derive(Debug, thiserror::Error)]
pub enum EnqueueError {
    #[error("busy: another approval {active} is active")]
    BusyAnotherActive { active: ApprovalId },
    #[error("duplicate id conflict: {id}")]
    DuplicateIdConflict { id: ApprovalId },
    #[error("store full")]
    StoreFull,
}

/// 决策操作的错误类型
#[derive(Debug, thiserror::Error)]
pub enum DecideError {
    #[error("approval not found: {id}")]
    NotFound { id: ApprovalId },
    #[error("already decided: {id}")]
    AlreadyDecided { id: ApprovalId, current: Decision },
}

/// 取消操作的错误类型
#[derive(Debug, thiserror::Error)]
pub enum CancelError {
    #[error("approval not found: {id}")]
    NotFound { id: ApprovalId },
    #[error("already decided: {id}")]
    AlreadyDecided { id: ApprovalId, current: Decision },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Decision;

    #[test]
    fn approval_state_pending_by_default() {
        let a = Approval {
            id: ApprovalId::new(),
            session_hash: "abcdef0123456789".into(),
            tool_name: "Bash".into(),
            fingerprint: "f".repeat(64),
            fingerprint_version: 1,
            tool_input_raw_sha256: "r".repeat(64),
            tool_input_canonical_sha256: "c".repeat(64),
            tool_input_original_bytes: 128,
            created_wall: time::OffsetDateTime::now_utc(),
            created_mono_ms: 1000,
            expires_at_mono_ms: 301_000,
            state: ApprovalState::Pending,
            trace_id: crate::trace::TraceId::generate(),
            approval_entry_span_id: crate::trace::SpanId::generate(),
        };
        assert!(a.is_pending());
        assert!(!a.is_decided());
    }

    #[test]
    fn approval_state_decided() {
        let mut a = Approval {
            id: ApprovalId::new(),
            session_hash: "abcdef0123456789".into(),
            tool_name: "Write".into(),
            fingerprint: "f".repeat(64),
            fingerprint_version: 1,
            tool_input_raw_sha256: "r".repeat(64),
            tool_input_canonical_sha256: "c".repeat(64),
            tool_input_original_bytes: 64,
            created_wall: time::OffsetDateTime::now_utc(),
            created_mono_ms: 1000,
            expires_at_mono_ms: 301_000,
            state: ApprovalState::Pending,
            trace_id: crate::trace::TraceId::generate(),
            approval_entry_span_id: crate::trace::SpanId::generate(),
        };
        a.state = ApprovalState::Decided {
            decision: Decision::Approved { feedback: None },
            decided_wall: time::OffsetDateTime::now_utc(),
            decided_mono_ms: 2000,
            decision_trace_id: crate::trace::TraceId::generate(),
        };
        assert!(a.is_decided());
        assert!(!a.is_pending());
    }

    #[test]
    fn enqueue_outcome_variants() {
        let id = ApprovalId::new();
        let outcome = EnqueueOutcome::Created { approval_id: id, revision: 1 };
        assert!(matches!(outcome, EnqueueOutcome::Created { .. }));
    }
}
