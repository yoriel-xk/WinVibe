use std::sync::Arc;

use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use winvibe_core::approval::types::{Approval, ApprovalState};
use winvibe_core::protocol::{ApprovalId, CancelReason, Decision};
use winvibe_core::trace::TraceCtx;
use winvibe_hook_server::sink::ApprovalLifecycleSink;

use crate::audit::record::{AuditDecision, AuditRecord, truncate_feedback_preview};
use crate::audit::AuditSink;
use crate::diagnostic::record::{DiagnosticKind, DiagnosticRecord};
use crate::diagnostic::DiagnosticSink;

/// IPC 事件推送抽象，允许测试中注入 mock
pub trait IpcEmitter: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
}

/// 基于 tauri::AppHandle 的 IPC 推送实现
#[cfg(not(test))]
pub struct TauriIpcEmitter {
    handle: tauri::AppHandle,
}

#[cfg(not(test))]
impl TauriIpcEmitter {
    pub fn new(handle: tauri::AppHandle) -> Self {
        Self { handle }
    }
}

#[cfg(not(test))]
impl IpcEmitter for TauriIpcEmitter {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        use tauri::Emitter;
        let _ = self.handle.emit(event, payload);
    }
}

/// 桥接 IPC、审计日志和诊断的生命周期 sink
pub struct AppLifecycleSink {
    pub ipc: Option<Arc<dyn IpcEmitter>>,
    pub audit_sink: Arc<dyn AuditSink>,
    pub diag_sink: Arc<DiagnosticSink>,
}

impl AppLifecycleSink {
    pub fn new(
        ipc: Option<Arc<dyn IpcEmitter>>,
        audit_sink: Arc<dyn AuditSink>,
        diag_sink: Arc<DiagnosticSink>,
    ) -> Self {
        Self { ipc, audit_sink, diag_sink }
    }

    /// 便捷构造：从 tauri::AppHandle 创建
    #[cfg(not(test))]
    pub fn with_tauri(
        handle: tauri::AppHandle,
        audit_sink: Arc<dyn AuditSink>,
        diag_sink: Arc<DiagnosticSink>,
    ) -> Self {
        Self {
            ipc: Some(Arc::new(TauriIpcEmitter::new(handle))),
            audit_sink,
            diag_sink,
        }
    }
}

impl ApprovalLifecycleSink for AppLifecycleSink {
    fn approval_pushed(
        &self,
        trace: TraceCtx,
        _parent_span: tracing::Span,
        id: ApprovalId,
        revision: u64,
    ) {
        let trace_id = trace.trace_id_hex();
        let span_id = trace.entry_span_id_hex();
        let id_str = id.to_string();

        tracing::info!(
            trace_id = %trace_id,
            approval_id = %id_str,
            revision,
            "审批入队"
        );

        // IPC 事件推送
        if let Some(ipc) = &self.ipc {
            ipc.emit("approval_pushed", serde_json::json!({
                "approval_id": id_str,
                "revision": revision,
                "trace_id": trace_id,
            }));
        }

        // diagnostic 记录
        let ts_wall = time::OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into());
        let diag = DiagnosticRecord {
            ts_wall,
            ts_mono_ms: 0,
            kind: DiagnosticKind::ServerReceived,
            trace_id,
            span_id,
            approval_id: Some(id_str),
            approval_trace_id: None,
            approval_entry_span_id: None,
            message: "approval_pushed".into(),
            extra: None,
        };
        self.diag_sink.write(&diag);
    }

    fn approval_resolved(
        &self,
        trace: TraceCtx,
        _parent_span: tracing::Span,
        approval: Approval,
        revision: u64,
    ) {
        let trace_id = trace.trace_id_hex();
        let span_id = trace.entry_span_id_hex();
        let id_str = approval.id.to_string();

        tracing::info!(
            trace_id = %trace_id,
            approval_id = %id_str,
            revision,
            "审批已决策"
        );

        // IPC 事件推送
        if let Some(ipc) = &self.ipc {
            ipc.emit("approval_resolved", serde_json::json!({
                "approval_id": id_str,
                "revision": revision,
                "trace_id": trace_id,
            }));
        }

        // diagnostic 记录
        let ts_wall = time::OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into());
        let diag = DiagnosticRecord {
            ts_wall,
            ts_mono_ms: 0,
            kind: DiagnosticKind::ServerDecided,
            trace_id: trace_id.clone(),
            span_id,
            approval_id: Some(id_str.clone()),
            approval_trace_id: Some(approval.trace_id.to_hex()),
            approval_entry_span_id: Some(approval.approval_entry_span_id.to_hex()),
            message: "approval_resolved".into(),
            extra: None,
        };
        self.diag_sink.write(&diag);

        // audit 记录（仅 Decided 状态）
        if let ApprovalState::Decided { .. } = &approval.state {
            let audit_record = build_audit_record(&approval);
            let audit_sink = Arc::clone(&self.audit_sink);
            tokio::spawn(async move {
                audit_sink.write(audit_record).await;
            });
        }
    }
}

/// 从 Approval 构建 AuditRecord
fn build_audit_record(approval: &Approval) -> AuditRecord {
    let created_wall = approval
        .created_wall
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into());

    let approval_trace_id = approval.trace_id.to_hex();

    let (decision, decided_wall, decision_trace_id) = match &approval.state {
        ApprovalState::Decided { decision, decided_wall, decision_trace_id, .. } => {
            let dw = decided_wall
                .format(&Rfc3339)
                .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into());
            (decision, dw, decision_trace_id.to_hex())
        }
        ApprovalState::Pending => {
            unreachable!("build_audit_record called on Pending approval")
        }
    };

    let audit_decision = build_audit_decision(decision);

    AuditRecord {
        approval_id: approval.id.to_string(),
        session_hash: approval.session_hash.clone(),
        tool_name: approval.tool_name.clone(),
        fingerprint: approval.fingerprint.clone(),
        fingerprint_version: approval.fingerprint_version as u32,
        decision: audit_decision,
        created_wall,
        decided_wall,
        approval_trace_id,
        decision_trace_id,
        tool_input_raw_sha256: approval.tool_input_raw_sha256.clone(),
        tool_input_canonical_sha256: approval.tool_input_canonical_sha256.clone(),
    }
}

/// 从 Decision 构建 AuditDecision
fn build_audit_decision(decision: &Decision) -> AuditDecision {
    match decision {
        Decision::Approved { feedback } => {
            let (feedback_present, feedback_sha256, feedback_preview) =
                build_feedback_fields(feedback.as_deref());
            AuditDecision {
                kind: "Approved".into(),
                cancel_reason: None,
                feedback_present,
                feedback_sha256,
                feedback_preview,
            }
        }
        Decision::Denied { feedback } => {
            let (feedback_present, feedback_sha256, feedback_preview) =
                build_feedback_fields(feedback.as_deref());
            AuditDecision {
                kind: "Denied".into(),
                cancel_reason: None,
                feedback_present,
                feedback_sha256,
                feedback_preview,
            }
        }
        Decision::TimedOut => AuditDecision {
            kind: "TimedOut".into(),
            cancel_reason: None,
            feedback_present: false,
            feedback_sha256: None,
            feedback_preview: None,
        },
        Decision::Cancelled { reason } => {
            let reason_str = match reason {
                CancelReason::StopHook => "StopHook",
                CancelReason::AppExit => "AppExit",
                CancelReason::UserAbort => "UserAbort",
            };
            AuditDecision {
                kind: "Cancelled".into(),
                cancel_reason: Some(reason_str.into()),
                feedback_present: false,
                feedback_sha256: None,
                feedback_preview: None,
            }
        }
    }
}

/// 计算 feedback 的 SHA256 和预览
fn build_feedback_fields(
    feedback: Option<&str>,
) -> (bool, Option<String>, Option<String>) {
    match feedback {
        None => (false, None, None),
        Some(text) => {
            let mut hasher = Sha256::new();
            hasher.update(text.as_bytes());
            let hash = hex::encode(hasher.finalize());
            let preview = truncate_feedback_preview(text);
            (true, Some(hash), Some(preview))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use winvibe_core::approval::types::ApprovalState;
    use winvibe_core::protocol::{ApprovalId, Decision};
    use winvibe_core::trace::{SpanId, TraceCtx, TraceId, TraceSource};
    use crate::audit::record::AuditRecord;

    /// 测试用 SpyAuditSink，记录所有写入的 records
    struct SpyAuditSink {
        records: Mutex<Vec<AuditRecord>>,
    }

    impl SpyAuditSink {
        fn new() -> Self {
            Self { records: Mutex::new(Vec::new()) }
        }

        fn records_len(&self) -> usize {
            self.records.lock().unwrap().len()
        }

        fn first_record_kind(&self) -> Option<String> {
            self.records
                .lock()
                .unwrap()
                .first()
                .map(|r| r.decision.kind.clone())
        }
    }

    #[async_trait::async_trait]
    impl AuditSink for SpyAuditSink {
        async fn write(&self, record: AuditRecord) {
            self.records.lock().unwrap().push(record);
        }

        async fn flush(&self) -> Result<(), std::io::Error> {
            Ok(())
        }

        async fn shutdown(&self) -> Result<(), std::io::Error> {
            Ok(())
        }
    }

    fn make_decided_approval() -> Approval {
        Approval {
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
            state: ApprovalState::Decided {
                decision: Decision::Approved { feedback: None },
                decided_wall: time::OffsetDateTime::now_utc(),
                decided_mono_ms: 2000,
                decision_trace_id: TraceId::generate(),
            },
            trace_id: TraceId::generate(),
            approval_entry_span_id: SpanId::generate(),
        }
    }

    #[tokio::test]
    async fn approval_resolved_writes_audit_record() {
        let spy = Arc::new(SpyAuditSink::new());
        let diag_dir = tempfile::tempdir().unwrap();
        let diag_sink = Arc::new(DiagnosticSink::new(diag_dir.path().to_path_buf(), false));

        let sink = AppLifecycleSink::new(None, Arc::clone(&spy) as Arc<dyn AuditSink>, diag_sink);

        let trace = TraceCtx::new(TraceSource::HudIpc);
        let approval = make_decided_approval();
        let span = tracing::info_span!("test");

        sink.approval_resolved(trace, span, approval, 1);

        // tokio::spawn 是异步的，等待一小段时间让任务完成
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert_eq!(spy.records_len(), 1);
        assert_eq!(spy.first_record_kind().unwrap(), "Approved");
    }

    #[tokio::test]
    async fn approval_resolved_pending_does_not_write_audit() {
        let spy = Arc::new(SpyAuditSink::new());
        let diag_dir = tempfile::tempdir().unwrap();
        let diag_sink = Arc::new(DiagnosticSink::new(diag_dir.path().to_path_buf(), false));

        let sink = AppLifecycleSink::new(None, Arc::clone(&spy) as Arc<dyn AuditSink>, diag_sink);

        let trace = TraceCtx::new(TraceSource::HudIpc);
        let approval = Approval {
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
            trace_id: TraceId::generate(),
            approval_entry_span_id: SpanId::generate(),
        };
        let span = tracing::info_span!("test");

        sink.approval_resolved(trace, span, approval, 1);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert_eq!(spy.records_len(), 0);
    }

    #[test]
    fn build_audit_decision_approved_with_feedback() {
        let decision = Decision::Approved { feedback: Some("looks good".into()) };
        let audit = build_audit_decision(&decision);
        assert_eq!(audit.kind, "Approved");
        assert!(audit.feedback_present);
        assert!(audit.feedback_sha256.is_some());
        assert!(audit.feedback_preview.is_some());
    }

    #[test]
    fn build_audit_decision_cancelled_has_reason() {
        let decision = Decision::Cancelled { reason: CancelReason::AppExit };
        let audit = build_audit_decision(&decision);
        assert_eq!(audit.kind, "Cancelled");
        assert_eq!(audit.cancel_reason.as_deref(), Some("AppExit"));
    }

    #[test]
    fn build_feedback_fields_none() {
        let (present, sha, preview) = build_feedback_fields(None);
        assert!(!present);
        assert!(sha.is_none());
        assert!(preview.is_none());
    }

    #[test]
    fn build_feedback_fields_some() {
        let (present, sha, preview) = build_feedback_fields(Some("hello"));
        assert!(present);
        assert_eq!(sha.unwrap().len(), 64); // SHA256 hex = 64 chars
        assert_eq!(preview.unwrap(), "hello");
    }
}
