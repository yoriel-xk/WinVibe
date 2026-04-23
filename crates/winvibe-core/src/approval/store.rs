use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use crate::clock::{MonotonicClock, WallClock};
use crate::protocol::{ApprovalId, Decision, CancelReason, PreToolUsePayload};
use crate::session::compute_session_hash;
use crate::trace::{TraceId, SpanId};
use super::types::*;
use super::fingerprint::{compute_fingerprint, sha256_hex, canonical_json};
use super::snapshot::ApprovalListSnapshot;

pub struct ApprovalStore {
    active: Option<ApprovalId>,
    by_id: HashMap<ApprovalId, Approval>,
    fingerprint_index: HashMap<String, ApprovalId>,
    cached_order: VecDeque<ApprovalId>,
    limits: ApprovalStoreLimits,
    revision: u64,
    wall: Arc<dyn WallClock>,
    mono: Arc<dyn MonotonicClock>,
}

impl ApprovalStore {
    pub fn new(
        limits: ApprovalStoreLimits,
        wall: Arc<dyn WallClock>,
        mono: Arc<dyn MonotonicClock>,
    ) -> Self {
        Self {
            active: None,
            by_id: HashMap::new(),
            fingerprint_index: HashMap::new(),
            cached_order: VecDeque::new(),
            limits,
            revision: 0,
            wall,
            mono,
        }
    }

    pub fn active(&self) -> Option<&Approval> {
        self.active.as_ref().and_then(|id| self.by_id.get(id))
    }

    pub fn get(&self, id: &ApprovalId) -> Option<&Approval> {
        self.by_id.get(id)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn next_revision(&mut self) -> u64 {
        self.revision += 1;
        self.revision
    }

    pub fn enqueue(
        &mut self,
        id: ApprovalId,
        payload: &PreToolUsePayload,
        ttl_ms: u64,
    ) -> Result<EnqueueOutcome, EnqueueError> {
        let fingerprint = compute_fingerprint(
            &payload.session_id,
            &payload.tool_name,
            &payload.tool_input,
        );

        // 幂等检查：相同 id
        if let Some(existing) = self.by_id.get(&id) {
            if existing.fingerprint == fingerprint {
                let rev = self.revision;
                return Ok(EnqueueOutcome::Existing { approval_id: id, revision: rev });
            } else {
                return Err(EnqueueError::DuplicateIdConflict { id });
            }
        }

        // 已有活跃审批
        if let Some(active_id) = self.active {
            return Err(EnqueueError::BusyAnotherActive { active: active_id });
        }

        let now_mono = self.mono.now_ms();
        let now_wall = self.wall.now();
        let canonical = canonical_json(&payload.tool_input);
        let raw_bytes = serde_json::to_vec(&payload.tool_input).unwrap_or_default();

        let approval = Approval {
            id,
            session_hash: compute_session_hash(&payload.session_id),
            tool_name: payload.tool_name.clone(),
            fingerprint: fingerprint.clone(),
            fingerprint_version: 1,
            tool_input_raw_sha256: sha256_hex(&raw_bytes),
            tool_input_canonical_sha256: sha256_hex(canonical.as_bytes()),
            tool_input_original_bytes: raw_bytes.len(),
            created_wall: now_wall,
            created_mono_ms: now_mono,
            expires_at_mono_ms: now_mono + ttl_ms,
            state: ApprovalState::Pending,
            trace_id: TraceId::generate(),
            approval_entry_span_id: SpanId::generate(),
        };

        self.fingerprint_index.insert(fingerprint, id);
        self.by_id.insert(id, approval);
        self.active = Some(id);
        let rev = self.next_revision();

        Ok(EnqueueOutcome::Created { approval_id: id, revision: rev })
    }

    pub fn decide(
        &mut self,
        id: ApprovalId,
        decision: Decision,
    ) -> Result<u64, DecideError> {
        let approval = self.by_id.get_mut(&id)
            .ok_or(DecideError::NotFound { id })?;

        if let ApprovalState::Decided { decision: ref current, .. } = approval.state {
            return Err(DecideError::AlreadyDecided { id, current: current.clone() });
        }

        approval.state = ApprovalState::Decided {
            decision,
            decided_wall: self.wall.now(),
            decided_mono_ms: self.mono.now_ms(),
            decision_trace_id: TraceId::generate(),
        };

        if self.active == Some(id) {
            self.active = None;
            self.move_to_cached(id);
        }

        Ok(self.next_revision())
    }

    pub fn cancel(
        &mut self,
        id: ApprovalId,
        reason: CancelReason,
    ) -> Result<u64, CancelError> {
        let approval = self.by_id.get_mut(&id)
            .ok_or(CancelError::NotFound { id })?;

        if let ApprovalState::Decided { decision: ref current, .. } = approval.state {
            return Err(CancelError::AlreadyDecided { id, current: current.clone() });
        }

        approval.state = ApprovalState::Decided {
            decision: Decision::Cancelled { reason },
            decided_wall: self.wall.now(),
            decided_mono_ms: self.mono.now_ms(),
            decision_trace_id: TraceId::generate(),
        };

        if self.active == Some(id) {
            self.active = None;
            self.move_to_cached(id);
        }

        Ok(self.next_revision())
    }

    pub fn expire_due_pending(&mut self) -> Vec<(ApprovalId, u64)> {
        let now = self.mono.now_ms();
        let mut expired = Vec::new();

        if let Some(active_id) = self.active {
            if let Some(approval) = self.by_id.get(&active_id) {
                if approval.is_pending() && now >= approval.expires_at_mono_ms {
                    let now_wall = self.wall.now();
                    let approval = self.by_id.get_mut(&active_id).unwrap();
                    approval.state = ApprovalState::Decided {
                        decision: Decision::TimedOut,
                        decided_wall: now_wall,
                        decided_mono_ms: now,
                        decision_trace_id: TraceId::generate(),
                    };
                    self.active = None;
                    self.move_to_cached(active_id);
                    let rev = self.next_revision();
                    expired.push((active_id, rev));
                }
            }
        }

        expired
    }

    pub fn snapshot(&self) -> ApprovalListSnapshot {
        let active = self.active.and_then(|id| self.by_id.get(&id).cloned());
        let cached: Vec<Approval> = self.cached_order.iter()
            .filter_map(|id| self.by_id.get(id).cloned())
            .collect();
        ApprovalListSnapshot {
            active,
            cached,
            revision: self.revision,
        }
    }

    fn move_to_cached(&mut self, id: ApprovalId) {
        self.cached_order.push_back(id);
        // FIFO 淘汰：超出 max_cached 时移除最旧的
        while self.cached_order.len() > self.limits.max_cached {
            if let Some(evicted) = self.cached_order.pop_front() {
                if let Some(a) = self.by_id.remove(&evicted) {
                    self.fingerprint_index.remove(&a.fingerprint);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{FakeMonotonicClock, FakeWallClock};
    use crate::protocol::ApprovalId;
    use std::sync::Arc;

    fn make_store() -> ApprovalStore {
        ApprovalStore::new(
            ApprovalStoreLimits::default(),
            Arc::new(FakeWallClock::default()),
            Arc::new(FakeMonotonicClock::new(10_000)),
        )
    }

    fn sample_payload() -> PreToolUsePayload {
        PreToolUsePayload {
            session_id: "sess-1".into(),
            tool_name: "Bash".into(),
            tool_input: serde_json::json!({"command": "ls"}),
        }
    }

    #[test]
    fn enqueue_creates_new_approval() {
        let mut store = make_store();
        let id = ApprovalId::new();
        let payload = sample_payload();
        let result = store.enqueue(id, &payload, 300_000);
        assert!(matches!(result, Ok(EnqueueOutcome::Created { .. })));
        assert!(store.active().is_some());
    }

    #[test]
    fn enqueue_idempotent_same_fingerprint() {
        let mut store = make_store();
        let id = ApprovalId::new();
        let payload = sample_payload();
        store.enqueue(id, &payload, 300_000).unwrap();
        let result = store.enqueue(id, &payload, 300_000);
        assert!(matches!(result, Ok(EnqueueOutcome::Existing { .. })));
    }

    #[test]
    fn enqueue_duplicate_id_different_fingerprint() {
        let mut store = make_store();
        let id = ApprovalId::new();
        let p1 = sample_payload();
        store.enqueue(id, &p1, 300_000).unwrap();
        let p2 = PreToolUsePayload {
            session_id: "sess-1".into(),
            tool_name: "Write".into(),
            tool_input: serde_json::json!({"path": "/tmp"}),
        };
        let result = store.enqueue(id, &p2, 300_000);
        assert!(matches!(result, Err(EnqueueError::DuplicateIdConflict { .. })));
    }

    #[test]
    fn enqueue_busy_another_active() {
        let mut store = make_store();
        let id1 = ApprovalId::new();
        let id2 = ApprovalId::new();
        let payload = sample_payload();
        store.enqueue(id1, &payload, 300_000).unwrap();
        let result = store.enqueue(id2, &payload, 300_000);
        assert!(matches!(result, Err(EnqueueError::BusyAnotherActive { .. })));
    }

    #[test]
    fn decide_approved() {
        let mut store = make_store();
        let id = ApprovalId::new();
        store.enqueue(id, &sample_payload(), 300_000).unwrap();
        let result = store.decide(id, Decision::Approved { feedback: None });
        assert!(result.is_ok());
        assert!(store.active().is_none());
        assert!(store.get(&id).unwrap().is_decided());
    }

    #[test]
    fn decide_already_decided() {
        let mut store = make_store();
        let id = ApprovalId::new();
        store.enqueue(id, &sample_payload(), 300_000).unwrap();
        store.decide(id, Decision::Approved { feedback: None }).unwrap();
        let result = store.decide(id, Decision::Denied { feedback: None });
        assert!(matches!(result, Err(DecideError::AlreadyDecided { .. })));
    }

    #[test]
    fn decide_not_found() {
        let mut store = make_store();
        let id = ApprovalId::new();
        let result = store.decide(id, Decision::Approved { feedback: None });
        assert!(matches!(result, Err(DecideError::NotFound { .. })));
    }

    #[test]
    fn cancel_pending() {
        let mut store = make_store();
        let id = ApprovalId::new();
        store.enqueue(id, &sample_payload(), 300_000).unwrap();
        let result = store.cancel(id, CancelReason::StopHook);
        assert!(result.is_ok());
        assert!(store.active().is_none());
        let a = store.get(&id).unwrap();
        assert!(matches!(a.decision(), Some(Decision::Cancelled { .. })));
    }

    #[test]
    fn expire_due_pending() {
        let mono = Arc::new(FakeMonotonicClock::new(10_000));
        let mut store = ApprovalStore::new(
            ApprovalStoreLimits::default(),
            Arc::new(FakeWallClock::default()),
            mono.clone(),
        );
        let id = ApprovalId::new();
        store.enqueue(id, &sample_payload(), 5_000).unwrap();
        mono.advance(std::time::Duration::from_secs(6));
        let expired = store.expire_due_pending();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].0, id);
        assert!(store.active().is_none());
    }

    #[test]
    fn snapshot_reflects_state() {
        let mut store = make_store();
        let id = ApprovalId::new();
        store.enqueue(id, &sample_payload(), 300_000).unwrap();
        let snap = store.snapshot();
        assert!(snap.active.is_some());
        assert!(snap.cached.is_empty());

        store.decide(id, Decision::Denied { feedback: None }).unwrap();
        let snap = store.snapshot();
        assert!(snap.active.is_none());
        assert_eq!(snap.cached.len(), 1);
    }

    #[test]
    fn cached_fifo_eviction() {
        let mut store = ApprovalStore::new(
            ApprovalStoreLimits { max_active: 1, max_cached: 2 },
            Arc::new(FakeWallClock::default()),
            Arc::new(FakeMonotonicClock::new(10_000)),
        );
        for _ in 0..3 {
            let id = ApprovalId::new();
            store.enqueue(id, &sample_payload(), 300_000).unwrap();
            store.decide(id, Decision::Approved { feedback: None }).unwrap();
        }
        let snap = store.snapshot();
        assert_eq!(snap.cached.len(), 2);
    }
}
