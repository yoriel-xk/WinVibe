use serde::Deserialize;
use winvibe_core::protocol::Decision;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum HudDecisionKind {
    Approved,
    Denied,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HudDecision {
    pub kind: HudDecisionKind,
    pub feedback: Option<String>,
}

impl HudDecision {
    pub fn to_decision(&self) -> Decision {
        match self.kind {
            HudDecisionKind::Approved => Decision::Approved {
                feedback: self.feedback.clone(),
            },
            HudDecisionKind::Denied => Decision::Denied {
                feedback: self.feedback.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_decision_deserialize_approve() {
        let json = r#"{"kind":"Approved"}"#;
        let d: HudDecision = serde_json::from_str(json).unwrap();
        assert_eq!(d.kind, HudDecisionKind::Approved);
        assert!(d.feedback.is_none());
    }

    #[test]
    fn hud_decision_deserialize_deny_with_feedback() {
        let json = r#"{"kind":"Denied","feedback":"dangerous tool"}"#;
        let d: HudDecision = serde_json::from_str(json).unwrap();
        assert_eq!(d.kind, HudDecisionKind::Denied);
        assert_eq!(d.feedback.as_deref(), Some("dangerous tool"));
    }

    #[test]
    fn hud_decision_to_core_decision() {
        let approve = HudDecision { kind: HudDecisionKind::Approved, feedback: None };
        assert!(matches!(approve.to_decision(), winvibe_core::protocol::Decision::Approved { .. }));

        let deny = HudDecision {
            kind: HudDecisionKind::Denied,
            feedback: Some("no".into()),
        };
        assert!(matches!(deny.to_decision(), winvibe_core::protocol::Decision::Denied { .. }));
    }
}
