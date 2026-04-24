use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// 审批请求的唯一标识符，基于 UUID v4
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApprovalId(pub uuid::Uuid);

impl ApprovalId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for ApprovalId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ApprovalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ApprovalId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(uuid::Uuid::parse_str(s)?))
    }
}

/// 审批决策，所有变体均为终态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Decision {
    Approved { feedback: Option<String> },
    Denied { feedback: Option<String> },
    TimedOut,
    Cancelled { reason: CancelReason },
}

impl Decision {
    /// 所有 Decision 变体都是终态
    pub fn is_terminal(&self) -> bool {
        true
    }
}

/// 审批被取消的原因
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CancelReason {
    StopHook,
    AppExit,
    UserAbort,
}

/// 来自 hookcli 的 PreToolUse 钩子请求体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreToolUsePayload {
    pub session_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
}

impl PreToolUsePayload {
    /// 验证 tool_input 必须是 JSON 对象
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.tool_input.is_object() {
            return Err("tool_input must be a JSON object");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_approved_round_trip() {
        let d = Decision::Approved {
            feedback: Some("ok".into()),
        };
        let json = serde_json::to_string(&d).unwrap();
        let d2: Decision = serde_json::from_str(&json).unwrap();
        assert!(matches!(d2, Decision::Approved { feedback: Some(f) } if f == "ok"));
    }

    #[test]
    fn decision_cancelled_variants() {
        for reason in [
            CancelReason::StopHook,
            CancelReason::AppExit,
            CancelReason::UserAbort,
        ] {
            let d = Decision::Cancelled {
                reason: reason.clone(),
            };
            let json = serde_json::to_string(&d).unwrap();
            let d2: Decision = serde_json::from_str(&json).unwrap();
            assert!(matches!(d2, Decision::Cancelled { .. }));
        }
    }

    #[test]
    fn pre_tool_use_payload_rejects_non_object() {
        let json = r#"{"session_id":"s1","tool_name":"Bash","tool_input":"not an object"}"#;
        let payload: PreToolUsePayload = serde_json::from_str(json).unwrap();
        assert!(!payload.tool_input.is_object());
    }

    #[test]
    fn approval_id_display_and_parse() {
        let id = ApprovalId(uuid::Uuid::new_v4());
        let s = id.to_string();
        let parsed: ApprovalId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }
}
