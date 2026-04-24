use winvibe_core::trace::{TraceCtx, TraceSource};

/// 从 TRACEPARENT 环境变量继承 trace_id，若不存在则生成新的
pub fn acquire_or_create_trace() -> TraceCtx {
    match std::env::var("TRACEPARENT")
        .ok()
        .and_then(|v| TraceCtx::parse(&v))
    {
        Some(inherited) => inherited,
        None => TraceCtx::new(TraceSource::HookCliRequest),
    }
}

/// 保留同一 trace_id，生成新的 span_id（每次 HTTP 请求调用一次）
pub fn new_span_traceparent(trace: &TraceCtx) -> String {
    trace.with_new_span().to_traceparent()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // 序列化所有操作环境变量的测试，避免并行竞态
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn assert_valid_traceparent(tp: &str) {
        let parts: Vec<&str> = tp.split('-').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "00");
        assert_eq!(parts[1].len(), 32);
        assert_eq!(parts[2].len(), 16);
        assert_eq!(parts[3], "01");
    }

    #[test]
    fn creates_new_trace_when_env_absent() {
        let _guard = ENV_LOCK.lock().unwrap();
        // 清除环境变量确保生成新的
        std::env::remove_var("TRACEPARENT");
        let ctx = acquire_or_create_trace();
        assert_valid_traceparent(&ctx.to_traceparent());
    }

    #[test]
    fn inherits_trace_id_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        let env_tp = "00-abcdef1234567890abcdef1234567890-1234567890abcdef-01";
        std::env::set_var("TRACEPARENT", env_tp);
        let ctx = acquire_or_create_trace();
        let tp = ctx.to_traceparent();
        // trace_id 应继承
        assert!(tp.starts_with("00-abcdef1234567890abcdef1234567890-"));
        std::env::remove_var("TRACEPARENT");
    }

    #[test]
    fn new_span_preserves_trace_id_changes_span() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("TRACEPARENT");
        let ctx = acquire_or_create_trace();
        let tp1 = new_span_traceparent(&ctx);
        let tp2 = new_span_traceparent(&ctx);
        assert_valid_traceparent(&tp1);
        assert_valid_traceparent(&tp2);
        // trace_id 相同
        let tid1 = tp1.split('-').nth(1).unwrap();
        let tid2 = tp2.split('-').nth(1).unwrap();
        assert_eq!(tid1, tid2);
        // span_id 不同
        let sid1 = tp1.split('-').nth(2).unwrap();
        let sid2 = tp2.split('-').nth(2).unwrap();
        assert_ne!(sid1, sid2);
    }
}
