use axum::http::HeaderMap;
use winvibe_core::trace::{TraceCtx, TraceSource};

/// 从请求 headers 解析 traceparent，缺失或非法则生成新的追踪上下文
pub fn extract_trace_ctx(headers: &HeaderMap, source: TraceSource) -> TraceCtx {
    if let Some(tp) = headers.get("traceparent") {
        if let Ok(tp_str) = tp.to_str() {
            // 尝试解析 W3C traceparent 格式
            if let Ok(ctx) = TraceCtx::from_traceparent(tp_str, source.clone()) {
                return ctx;
            }
        }
    }
    // 缺失或解析失败时生成新的追踪上下文
    TraceCtx::new(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_traceparent_parsed() {
        // 合法的 traceparent header 应被正确解析
        let mut headers = HeaderMap::new();
        headers.insert(
            "traceparent",
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
                .parse()
                .unwrap(),
        );
        let ctx = extract_trace_ctx(&headers, TraceSource::HookCliRequest);
        assert_eq!(ctx.trace_id.to_hex(), "0af7651916cd43dd8448eb211c80319c");
    }

    #[test]
    fn missing_traceparent_generates_new() {
        // 缺失 traceparent 时应生成新的 trace_id（32 字符十六进制）
        let headers = HeaderMap::new();
        let ctx = extract_trace_ctx(&headers, TraceSource::HookCliRequest);
        assert_eq!(ctx.trace_id.to_hex().len(), 32);
    }
}
