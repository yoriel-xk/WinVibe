use super::record::DiagnosticRecord;
use std::path::PathBuf;

pub struct DiagnosticSink {
    dir: PathBuf,
    enabled: bool,
}

impl DiagnosticSink {
    pub fn new(dir: PathBuf, enabled: bool) -> Self {
        Self { dir, enabled }
    }

    pub fn write(&self, record: &DiagnosticRecord) {
        if !self.enabled {
            return;
        }
        let approval_id = record.approval_id.as_deref().unwrap_or("unknown");
        let path = self.dir.join(format!("{approval_id}.jsonl"));
        if let Err(e) = self.append_record(&path, record) {
            tracing::warn!(error = %e, "写入 diagnostic 文件失败");
        }
    }

    fn append_record(
        &self,
        path: &std::path::Path,
        record: &DiagnosticRecord,
    ) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let line = serde_json::to_string(record)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "{line}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::record::{DiagnosticKind, DiagnosticRecord};
    use super::*;

    #[test]
    fn sink_writes_file_per_approval_id() {
        let dir = tempfile::tempdir().unwrap();
        let sink = DiagnosticSink::new(dir.path().to_path_buf(), true);
        let record = DiagnosticRecord {
            ts_wall: "2026-04-23T10:00:00Z".into(),
            ts_mono_ms: 1000,
            kind: DiagnosticKind::ServerReceived,
            trace_id: "a".repeat(32),
            span_id: "b".repeat(16),
            approval_id: Some("test-approval-id".into()),
            approval_trace_id: None,
            approval_entry_span_id: None,
            message: "test".into(),
            extra: None,
        };
        sink.write(&record);
        let path = dir.path().join("test-approval-id.jsonl");
        assert!(path.exists());
    }

    #[test]
    fn sink_disabled_does_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let sink = DiagnosticSink::new(dir.path().to_path_buf(), false);
        let record = DiagnosticRecord {
            ts_wall: "2026-04-23T10:00:00Z".into(),
            ts_mono_ms: 1000,
            kind: DiagnosticKind::Error,
            trace_id: "a".repeat(32),
            span_id: "b".repeat(16),
            approval_id: Some("test-id".into()),
            approval_trace_id: None,
            approval_entry_span_id: None,
            message: "should not appear".into(),
            extra: None,
        };
        sink.write(&record);
        assert!(std::fs::read_dir(dir.path()).unwrap().count() == 0);
    }
}
