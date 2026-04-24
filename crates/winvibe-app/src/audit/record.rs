use serde::Serialize;

/// 审计记录，每个已决策的审批写入一条
#[derive(Debug)]
pub struct AuditRecord {
    pub approval_id: String,
    pub session_hash: String,
    pub tool_name: String,
    pub fingerprint: String,
    pub fingerprint_version: u32,
    pub decision: AuditDecision,
    pub created_wall: String,
    pub decided_wall: String,
    pub approval_trace_id: String,
    pub decision_trace_id: String,
    pub tool_input_raw_sha256: String,
    pub tool_input_canonical_sha256: String,
}

// 手动实现 Serialize，在最前面注入 schema 字段
impl Serialize for AuditRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("schema", "winvibe.audit.v1")?;
        map.serialize_entry("approval_id", &self.approval_id)?;
        map.serialize_entry("session_hash", &self.session_hash)?;
        map.serialize_entry("tool_name", &self.tool_name)?;
        map.serialize_entry("fingerprint", &self.fingerprint)?;
        map.serialize_entry("fingerprint_version", &self.fingerprint_version)?;
        map.serialize_entry("decision", &self.decision)?;
        map.serialize_entry("created_wall", &self.created_wall)?;
        map.serialize_entry("decided_wall", &self.decided_wall)?;
        map.serialize_entry("approval_trace_id", &self.approval_trace_id)?;
        map.serialize_entry("decision_trace_id", &self.decision_trace_id)?;
        map.serialize_entry("tool_input_raw_sha256", &self.tool_input_raw_sha256)?;
        map.serialize_entry(
            "tool_input_canonical_sha256",
            &self.tool_input_canonical_sha256,
        )?;
        map.end()
    }
}

#[derive(Debug, Serialize)]
pub struct AuditDecision {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_reason: Option<String>,
    pub feedback_present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback_preview: Option<String>,
}

/// 截断 feedback 预览到 80 字符，去除换行
pub fn truncate_feedback_preview(text: &str) -> String {
    text.chars()
        .filter(|c| *c != '\n' && *c != '\r')
        .take(80)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_record_has_correct_schema_field() {
        let record = AuditRecord {
            approval_id: "test-id".into(),
            session_hash: "abcd1234abcd1234".into(),
            tool_name: "Bash".into(),
            fingerprint: "a".repeat(64),
            fingerprint_version: 1,
            decision: AuditDecision {
                kind: "Approved".into(),
                cancel_reason: None,
                feedback_present: false,
                feedback_sha256: None,
                feedback_preview: None,
            },
            created_wall: "2026-04-23T10:00:00Z".into(),
            decided_wall: "2026-04-23T10:01:00Z".into(),
            approval_trace_id: "a".repeat(32),
            decision_trace_id: "b".repeat(32),
            tool_input_raw_sha256: "c".repeat(64),
            tool_input_canonical_sha256: "d".repeat(64),
        };
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["schema"], "winvibe.audit.v1");
    }

    #[test]
    fn audit_decision_cancelled_has_cancel_reason() {
        let decision = AuditDecision {
            kind: "Cancelled".into(),
            cancel_reason: Some("AppExit".into()),
            feedback_present: false,
            feedback_sha256: None,
            feedback_preview: None,
        };
        let json = serde_json::to_value(&decision).unwrap();
        assert_eq!(json["cancel_reason"], "AppExit");
    }

    #[test]
    fn feedback_preview_truncated_to_80_chars() {
        let long_feedback = "あ".repeat(100);
        let preview = truncate_feedback_preview(&long_feedback);
        assert!(preview.chars().count() <= 80);
    }
}
