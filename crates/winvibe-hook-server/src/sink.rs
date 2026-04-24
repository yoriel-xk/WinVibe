use winvibe_core::approval::types::Approval;
use winvibe_core::protocol::ApprovalId;
use winvibe_core::trace::TraceCtx;

/// 审批生命周期事件 sink，由 Tauri 层实现以桥接 IPC、审计日志和诊断
pub trait ApprovalLifecycleSink: Send + Sync {
    /// 审批入队时调用
    fn approval_pushed(
        &self,
        trace: TraceCtx,
        parent_span: tracing::Span,
        id: ApprovalId,
        revision: u64,
    );

    /// 审批决策完成时调用
    fn approval_resolved(
        &self,
        trace: TraceCtx,
        parent_span: tracing::Span,
        approval: Approval,
        revision: u64,
    );
}

/// 测试用 no-op sink，所有方法均为空实现
pub struct NoopSink;

impl ApprovalLifecycleSink for NoopSink {
    fn approval_pushed(&self, _: TraceCtx, _: tracing::Span, _: ApprovalId, _: u64) {}
    fn approval_resolved(&self, _: TraceCtx, _: tracing::Span, _: Approval, _: u64) {}
}
