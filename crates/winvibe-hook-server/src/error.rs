use winvibe_core::approval::types::{CancelError, DecideError, EnqueueError};
use winvibe_core::protocol::ApprovalId;

/// ApprovalRuntime 运行时错误枚举
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("shutting down")]
    ShuttingDown,
    #[error(transparent)]
    Enqueue(#[from] EnqueueError),
    #[error(transparent)]
    Decide(#[from] DecideError),
    #[error(transparent)]
    Cancel(#[from] CancelError),
    #[error("approval not found: {0}")]
    NotFound(ApprovalId),
}
