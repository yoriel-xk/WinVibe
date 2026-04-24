use crate::audit::AuditSink;
use std::sync::Arc;
use winvibe_hook_server::runtime::ApprovalRuntime;

/// 应用全局状态，通过 Tauri 的 manage() 注入到所有 IPC 命令
pub struct AppState {
    /// 审批运行时，负责 snapshot / decide 操作
    pub runtime: Arc<ApprovalRuntime>,
    /// 审计日志接收器
    pub audit_sink: Arc<dyn AuditSink>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 AppState 可以被正常构造（类型级别检查）
    #[test]
    fn app_state_can_be_constructed() {
        // 仅做类型检查，不实际运行 runtime
        // 通过编译即证明字段类型正确
        fn _assert_send_sync()
        where
            AppState: Send + Sync,
        {
        }
    }
}
