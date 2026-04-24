use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum DiagnosticKind {
    #[serde(rename = "hookcli.attempt")]
    HookCliAttempt,
    #[serde(rename = "server.received")]
    ServerReceived,
    #[serde(rename = "server.decided")]
    ServerDecided,
    #[serde(rename = "ipc.snapshot")]
    IpcSnapshot,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticRecord {
    pub ts_wall: String,
    pub ts_mono_ms: u64,
    pub kind: DiagnosticKind,
    pub trace_id: String,
    pub span_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_entry_span_id: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_record_serializes_kind() {
        let record = DiagnosticRecord {
            ts_wall: "2026-04-23T10:00:00Z".into(),
            ts_mono_ms: 1000,
            kind: DiagnosticKind::ServerReceived,
            trace_id: "a".repeat(32),
            span_id: "b".repeat(16),
            approval_id: Some("test-id".into()),
            approval_trace_id: None,
            approval_entry_span_id: None,
            message: "received pre_tool_use".into(),
            extra: None,
        };
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["kind"], "server.received");
    }

    #[test]
    fn diagnostic_record_skips_none_fields() {
        let record = DiagnosticRecord {
            ts_wall: "2026-04-23T10:00:00Z".into(),
            ts_mono_ms: 1000,
            kind: DiagnosticKind::Error,
            trace_id: "a".repeat(32),
            span_id: "b".repeat(16),
            approval_id: None,
            approval_trace_id: None,
            approval_entry_span_id: None,
            message: "oops".into(),
            extra: None,
        };
        let json = serde_json::to_value(&record).unwrap();
        assert!(json.get("approval_id").is_none());
        assert!(json.get("approval_trace_id").is_none());
    }
}
