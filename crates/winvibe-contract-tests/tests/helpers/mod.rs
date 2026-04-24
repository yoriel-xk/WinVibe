pub mod test_server;
pub use test_server::TestServer;

/// 构造 reqwest 客户端（禁用代理以避免环境干扰）
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("构造 HTTP 客户端失败")
}

/// 从响应 JSON 中提取 code 字段
pub fn extract_code(body: &serde_json::Value) -> &str {
    body.get("code").and_then(|v| v.as_str()).unwrap_or("")
}

/// 断言错误响应包含非空 trace_id
pub fn assert_trace_id(body: &serde_json::Value) {
    let trace_id = body.get("trace_id").and_then(|v| v.as_str()).unwrap_or("");
    assert!(!trace_id.is_empty(), "trace_id 应存在于错误响应中");
}

/// 轮询等待 runtime 中出现 active 审批（替代固定 sleep，避免竞态）
pub async fn wait_for_active(srv: &TestServer) {
    for _ in 0..50 {
        if srv.runtime.snapshot().await.active.is_some() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("等待 active 审批超时");
}
