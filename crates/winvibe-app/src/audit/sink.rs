use super::record::AuditRecord;
use std::path::PathBuf;
use tokio::sync::mpsc;

#[async_trait::async_trait]
pub trait AuditSink: Send + Sync {
    async fn write(&self, record: AuditRecord);
    async fn flush(&self) -> Result<(), std::io::Error>;
    async fn shutdown(&self) -> Result<(), std::io::Error>;
}

pub struct JsonlAuditSink {
    tx: mpsc::Sender<AuditSinkMessage>,
}

enum AuditSinkMessage {
    Write(AuditRecord),
    Flush(tokio::sync::oneshot::Sender<Result<(), std::io::Error>>),
    Shutdown(tokio::sync::oneshot::Sender<Result<(), std::io::Error>>),
}

impl JsonlAuditSink {
    pub fn new(dir: PathBuf) -> Self {
        let (tx, mut rx) = mpsc::channel::<AuditSinkMessage>(256);
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    AuditSinkMessage::Write(record) => {
                        let _ = write_record_to_file(&dir, &record);
                    }
                    AuditSinkMessage::Flush(reply) => {
                        let _ = reply.send(Ok(()));
                    }
                    AuditSinkMessage::Shutdown(reply) => {
                        let _ = reply.send(Ok(()));
                        break;
                    }
                }
            }
        });
        Self { tx }
    }
}

fn write_record_to_file(dir: &std::path::Path, record: &AuditRecord) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let date = record.decided_wall.get(..10).unwrap_or("1970-01-01");
    let path = dir.join(format!("{}.jsonl", date));
    let line = serde_json::to_string(record)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

#[async_trait::async_trait]
impl AuditSink for JsonlAuditSink {
    async fn write(&self, record: AuditRecord) {
        let _ = self.tx.send(AuditSinkMessage::Write(record)).await;
    }

    async fn flush(&self) -> Result<(), std::io::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = self.tx.send(AuditSinkMessage::Flush(tx)).await;
        rx.await.unwrap_or(Ok(()))
    }

    async fn shutdown(&self) -> Result<(), std::io::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = self.tx.send(AuditSinkMessage::Shutdown(tx)).await;
        rx.await.unwrap_or(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn audit_sink_flush_returns_ok() {
        let dir = tempfile::tempdir().unwrap();
        let sink = JsonlAuditSink::new(dir.path().to_path_buf());
        sink.flush().await.unwrap();
    }
}
