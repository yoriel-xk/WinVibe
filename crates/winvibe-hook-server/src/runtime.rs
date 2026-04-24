use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{watch, Mutex};

use winvibe_core::approval::snapshot::ApprovalListSnapshot;
use winvibe_core::approval::store::ApprovalStore;
use winvibe_core::approval::types::Approval;
use winvibe_core::approval::types::{ApprovalStoreLimits, EnqueueOutcome};
use winvibe_core::clock::{MonotonicClock, WallClock};
use winvibe_core::protocol::{ApprovalId, CancelReason, Decision, PreToolUsePayload};
use winvibe_core::trace::TraceCtx;

use crate::error::RuntimeError;
use crate::sink::ApprovalLifecycleSink;

/// 等待决策的结果
pub enum WaitOutcome {
    /// 已决策
    Decided { approval: Approval, revision: u64 },
    /// 超时仍在等待
    Pending { id: ApprovalId, revision: u64 },
    /// 幂等重复提交，已有相同审批
    Existing { approval: Approval, revision: u64 },
    /// 新建审批（已决策，极短 TTL 场景）
    Created { approval: Approval, revision: u64 },
}

/// watch channel 的 tick 载荷
#[derive(Clone)]
pub struct RevisionTick {
    pub revision: u64,
}

/// 审批运行时，管理 ApprovalStore 并协调 HTTP handler 与 HUD sink
///
/// 锁序不变式: store 与 watchers 绝不同时持有。
/// 所有方法均先获取 store 锁完成操作后释放，再获取 watchers 锁。
pub struct ApprovalRuntime {
    store: Mutex<ApprovalStore>,
    watchers: Mutex<HashMap<ApprovalId, watch::Sender<RevisionTick>>>,
    sink: Arc<dyn ApprovalLifecycleSink>,
    accepting: AtomicBool,
    ttl_ms: u64,
}

impl ApprovalRuntime {
    /// 构造新的 ApprovalRuntime
    pub fn new(
        limits: ApprovalStoreLimits,
        wall: Arc<dyn WallClock>,
        mono: Arc<dyn MonotonicClock>,
        sink: Arc<dyn ApprovalLifecycleSink>,
        ttl_ms: u64,
    ) -> Self {
        Self {
            store: Mutex::new(ApprovalStore::new(limits, wall, mono)),
            watchers: Mutex::new(HashMap::new()),
            sink,
            accepting: AtomicBool::new(true),
            ttl_ms,
        }
    }

    /// 标记 runtime 开始关闭，不再接受新提交
    pub fn begin_shutdown(&self) {
        self.accepting.store(false, Ordering::SeqCst);
    }

    /// 检查是否仍在接受新提交
    pub fn is_accepting(&self) -> bool {
        self.accepting.load(Ordering::SeqCst)
    }

    /// 提交 pre-tool-use 审批请求，等待 wait_timeout 后返回结果
    pub async fn submit_pre_tool_use(
        &self,
        trace: TraceCtx,
        id: ApprovalId,
        payload: PreToolUsePayload,
        wait_timeout: Duration,
    ) -> Result<WaitOutcome, RuntimeError> {
        // 检查是否正在关闭
        if !self.is_accepting() {
            return Err(RuntimeError::ShuttingDown);
        }

        let (approval_id, revision, is_new) = {
            let mut store = self.store.lock().await;
            // 先过期旧的
            store.expire_due_pending();
            let outcome = store.enqueue(id, &payload, self.ttl_ms)?;
            match outcome {
                EnqueueOutcome::Created {
                    approval_id,
                    revision,
                } => (approval_id, revision, true),
                EnqueueOutcome::Existing {
                    approval_id,
                    revision,
                } => (approval_id, revision, false),
            }
        };

        if is_new {
            // 新建：创建 watcher，通知 sink，然后等待决策
            let (tx, rx) = watch::channel(RevisionTick { revision });
            {
                let mut watchers = self.watchers.lock().await;
                watchers.insert(approval_id, tx);
            }

            // 通知 sink：审批已入队
            let span = tracing::info_span!("approval_pushed", %approval_id, revision);
            self.sink
                .approval_pushed(trace, span, approval_id, revision);

            // 等待决策结果
            self.wait_for_decision(approval_id, revision, rx, wait_timeout)
                .await
        } else {
            // 幂等重复：获取现有 Approval 并返回 Existing
            let store = self.store.lock().await;
            let approval = store
                .get(&approval_id)
                .ok_or(RuntimeError::NotFound(approval_id))?
                .clone();
            Ok(WaitOutcome::Existing { approval, revision })
        }
    }

    /// 等待指定审批的决策，超时后返回 Pending
    async fn wait_for_decision(
        &self,
        id: ApprovalId,
        initial_revision: u64,
        mut rx: watch::Receiver<RevisionTick>,
        timeout: Duration,
    ) -> Result<WaitOutcome, RuntimeError> {
        // 等待 watcher 有变化，或超时
        let timed_out = tokio::time::timeout(timeout, async {
            // 跳过当前值，等待下一个 tick
            rx.changed().await.ok();
        })
        .await
        .is_err();

        // 无论是否超时，检查当前状态
        let store = self.store.lock().await;
        if let Some(approval) = store.get(&id) {
            if approval.is_decided() {
                let revision = store.revision();
                return Ok(WaitOutcome::Decided {
                    approval: approval.clone(),
                    revision,
                });
            }
        }

        // 仍在等待（超时或其他原因）
        let _ = timed_out; // 明确忽略
        Ok(WaitOutcome::Pending {
            id,
            revision: initial_revision,
        })
    }

    /// 轮询审批决策（非阻塞检查，若已决策直接返回，否则等待 wait_timeout）
    pub async fn poll_decision(
        &self,
        _trace: TraceCtx,
        id: ApprovalId,
        wait_timeout: Duration,
    ) -> Result<WaitOutcome, RuntimeError> {
        // 先检查当前状态
        let (already_decided, revision) = {
            let store = self.store.lock().await;
            match store.get(&id) {
                Some(approval) if approval.is_decided() => {
                    (Some(approval.clone()), store.revision())
                }
                Some(_) => (None, store.revision()),
                None => return Err(RuntimeError::NotFound(id)),
            }
        };

        if let Some(approval) = already_decided {
            return Ok(WaitOutcome::Decided { approval, revision });
        }

        // 订阅已有的 watcher
        let rx = {
            let watchers = self.watchers.lock().await;
            watchers.get(&id).map(|tx| tx.subscribe())
        };

        match rx {
            Some(rx) => self.wait_for_decision(id, revision, rx, wait_timeout).await,
            None => {
                // 没有 watcher，直接返回 Pending
                Ok(WaitOutcome::Pending { id, revision })
            }
        }
    }

    /// 对指定审批作出决策
    pub async fn decide(
        &self,
        trace: TraceCtx,
        id: ApprovalId,
        decision: Decision,
    ) -> Result<u64, RuntimeError> {
        // 先 lock store 作决策，完成后 drop lock
        let (revision, approval) = {
            let mut store = self.store.lock().await;
            store.expire_due_pending();
            let revision = store.decide(id, decision)?;
            let approval = store.get(&id).ok_or(RuntimeError::NotFound(id))?.clone();
            (revision, approval)
        };

        // 通知 watcher 并移除（store lock 已释放）
        {
            let mut watchers = self.watchers.lock().await;
            if let Some(tx) = watchers.remove(&id) {
                let _ = tx.send(RevisionTick { revision });
            }
        }

        // 通知 sink
        let span = tracing::info_span!("approval_resolved", %id, revision);
        self.sink.approval_resolved(trace, span, approval, revision);

        Ok(revision)
    }

    /// 取消所有 pending 审批（关闭时调用），返回被取消的 (ApprovalId, revision) 列表
    pub async fn cancel_all_pending(
        &self,
        trace: TraceCtx,
        reason: CancelReason,
    ) -> Vec<(ApprovalId, u64)> {
        // 先在 store 锁内完成取消，取出审批快照，然后释放锁
        let cancelled_info: Vec<(ApprovalId, u64, Approval)> = {
            let mut store = self.store.lock().await;
            let mut result = Vec::new();
            if let Some(active_id) = store.active().map(|a| a.id) {
                if let Ok(rev) = store.cancel(active_id, reason.clone()) {
                    if let Some(approval) = store.get(&active_id) {
                        result.push((active_id, rev, approval.clone()));
                    }
                }
            }
            result
        };
        // store 锁已释放，再获取 watchers 锁，避免死锁
        let mut cancelled = Vec::new();
        for (active_id, rev, approval) in cancelled_info {
            {
                let mut watchers = self.watchers.lock().await;
                if let Some(tx) = watchers.remove(&active_id) {
                    let _ = tx.send(RevisionTick { revision: rev });
                }
            }
            let span = tracing::info_span!("approval_resolved", id = %active_id, revision = rev);
            self.sink
                .approval_resolved(trace.clone(), span, approval, rev);
            cancelled.push((active_id, rev));
        }
        cancelled
    }

    /// 获取当前状态快照
    pub async fn snapshot(&self) -> ApprovalListSnapshot {
        let mut store = self.store.lock().await;
        store.expire_due_pending();
        store.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sink::NoopSink;
    use std::sync::Arc;
    use std::time::Duration;
    use winvibe_core::clock::{FakeMonotonicClock, FakeWallClock};
    use winvibe_core::protocol::PreToolUsePayload;
    use winvibe_core::trace::{TraceCtx, TraceSource};

    fn make_runtime() -> ApprovalRuntime {
        ApprovalRuntime::new(
            Default::default(),
            Arc::new(FakeWallClock::default()),
            Arc::new(FakeMonotonicClock::new(10_000)),
            Arc::new(NoopSink),
            300_000,
        )
    }

    fn sample_payload() -> PreToolUsePayload {
        PreToolUsePayload {
            session_id: "sess-1".into(),
            tool_name: "Bash".into(),
            tool_input: serde_json::json!({"command": "ls"}),
        }
    }

    #[tokio::test]
    async fn submit_creates_pending() {
        let rt = make_runtime();
        let trace = TraceCtx::new(TraceSource::HookCliRequest);
        let result = rt
            .submit_pre_tool_use(
                trace,
                ApprovalId::new(),
                sample_payload(),
                Duration::from_millis(1),
            )
            .await;
        assert!(matches!(result, Ok(WaitOutcome::Pending { .. })));
        let snap = rt.snapshot().await;
        assert!(snap.active.is_some());
    }

    #[tokio::test]
    async fn decide_resolves_approval() {
        let rt = Arc::new(make_runtime());
        let trace = TraceCtx::new(TraceSource::HookCliRequest);
        let outcome = rt
            .submit_pre_tool_use(
                trace,
                ApprovalId::new(),
                sample_payload(),
                Duration::from_millis(1),
            )
            .await
            .unwrap();
        let id = match outcome {
            WaitOutcome::Pending { id, .. } => id,
            _ => panic!("expected Pending"),
        };

        let trace2 = TraceCtx::new(TraceSource::HudIpc);
        rt.decide(trace2, id, Decision::Approved { feedback: None })
            .await
            .unwrap();

        let snap = rt.snapshot().await;
        assert!(snap.active.is_none());
        assert_eq!(snap.cached.len(), 1);
    }

    #[tokio::test]
    async fn begin_shutdown_rejects_new() {
        let rt = make_runtime();
        rt.begin_shutdown();
        let trace = TraceCtx::new(TraceSource::HookCliRequest);
        let result = rt
            .submit_pre_tool_use(
                trace,
                ApprovalId::new(),
                sample_payload(),
                Duration::from_secs(1),
            )
            .await;
        assert!(matches!(result, Err(RuntimeError::ShuttingDown)));
    }
}
