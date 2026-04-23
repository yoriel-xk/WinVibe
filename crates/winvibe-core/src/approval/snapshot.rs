use super::types::Approval;
use serde::{Serialize, Deserialize};

/// 审批列表的快照，用于 HUD 展示和状态同步
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalListSnapshot {
    pub active: Option<Approval>,
    pub cached: Vec<Approval>,
    pub revision: u64,
}
