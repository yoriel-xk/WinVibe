//! 错误码矩阵契约测试（§5.2）
//! 通过真实 HTTP 请求验证服务器返回的错误码、状态码和响应格式

mod helpers;

use helpers::TestServer;
use serde_json::Value;

/// 构造 reqwest 客户端（禁用连接池以避免端口复用问题）
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("构造 HTTP 客户端失败")
}

/// 从响应 JSON 中提取 code 字段
fn extract_code(body: &Value) -> &str {
    body.get("code").and_then(|v| v.as_str()).unwrap_or("")
}

// ── 401 Unauthorized ──

#[tokio::test]
async fn test_401_unauthorized_no_token() {
    // 不带 Authorization header → 401, code="unauthorized", trace_id 存在
    let srv = TestServer::start().await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/hook/submit", srv.base_url()))
        .json(&serde_json::json!({
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .send()
        .await
        .expect("请求失败");

    assert_eq!(resp.status().as_u16(), 401);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(extract_code(&body), "unauthorized");
    // trace_id 应存在且非空
    let trace_id = body.get("trace_id").and_then(|v| v.as_str()).unwrap_or("");
    assert!(!trace_id.is_empty(), "trace_id 应存在于 401 响应中");

    srv.shutdown().await;
}

#[tokio::test]
async fn test_401_unauthorized_wrong_token() {
    // 带错误 Bearer token → 401, code="unauthorized"
    let srv = TestServer::start().await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/hook/submit", srv.base_url()))
        .header("Authorization", "Bearer wrong-token-xxx")
        .json(&serde_json::json!({
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .send()
        .await
        .expect("请求失败");

    assert_eq!(resp.status().as_u16(), 401);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(extract_code(&body), "unauthorized");

    srv.shutdown().await;
}

// ── 400 Invalid Request ──

#[tokio::test]
async fn test_400_missing_x_approval_id_header() {
    // 不带 X-Approval-Id header → 400, code="invalid_request"
    let srv = TestServer::start().await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/hook/submit", srv.base_url()))
        .header("Authorization", srv.bearer())
        .json(&serde_json::json!({
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .send()
        .await
        .expect("请求失败");

    assert_eq!(resp.status().as_u16(), 400);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(extract_code(&body), "invalid_request");

    srv.shutdown().await;
}

#[tokio::test]
async fn test_400_invalid_x_approval_id_header() {
    // X-Approval-Id 为非 UUID 字符串 → 400, code="invalid_request"
    let srv = TestServer::start().await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/hook/submit", srv.base_url()))
        .header("Authorization", srv.bearer())
        .header("X-Approval-Id", "not-a-uuid")
        .json(&serde_json::json!({
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .send()
        .await
        .expect("请求失败");

    assert_eq!(resp.status().as_u16(), 400);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(extract_code(&body), "invalid_request");

    srv.shutdown().await;
}

#[tokio::test]
async fn test_422_invalid_request_missing_tool_name() {
    // 请求体缺少 tool_name 字段 → axum Json 反序列化失败返回 422
    // 注意：这是 axum 默认行为，响应体格式可能不是自定义 ErrorResponse
    let srv = TestServer::start().await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/hook/submit", srv.base_url()))
        .header("Authorization", srv.bearer())
        .header("X-Approval-Id", uuid::Uuid::new_v4().to_string())
        .json(&serde_json::json!({
            "session_id": "s1",
            "tool_input": {"command": "ls"}
        }))
        .send()
        .await
        .expect("请求失败");

    // axum 的 Json<T> 提取器对反序列化失败默认返回 422
    let status = resp.status().as_u16();
    assert!(
        status == 400 || status == 422,
        "缺少 tool_name 应返回 4xx，实际: {status}"
    );

    srv.shutdown().await;
}

// ── 409 Conflict ──

#[tokio::test]
async fn test_409_busy_another_active() {
    // 提交两个不同 approval → 第二个应返回 409, code="busy_another_active"
    let srv = TestServer::start().await;
    let client = http_client();
    let base = srv.base_url();
    let bearer = srv.bearer();

    let id1 = uuid::Uuid::new_v4().to_string();
    let id2 = uuid::Uuid::new_v4().to_string();

    // 第一个 submit 会阻塞等待决策（最长 25s），放到后台任务
    let client_bg = client.clone();
    let base_bg = base.clone();
    let bearer_bg = bearer.clone();
    let id1_clone = id1.clone();
    let first_task = tokio::spawn(async move {
        let _ = client_bg
            .post(format!("{base_bg}/v1/hook/submit"))
            .header("Authorization", &bearer_bg)
            .header("X-Approval-Id", &id1_clone)
            .json(&serde_json::json!({
                "session_id": "s1",
                "tool_name": "Bash",
                "tool_input": {"command": "ls"}
            }))
            .send()
            .await;
    });

    // 等待第一个请求注册到 runtime
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // 第二个 submit 应立即返回 409
    let resp = client
        .post(format!("{base}/v1/hook/submit"))
        .header("Authorization", &bearer)
        .header("X-Approval-Id", &id2)
        .json(&serde_json::json!({
            "session_id": "s2",
            "tool_name": "Read",
            "tool_input": {"path": "/tmp"}
        }))
        .send()
        .await
        .expect("请求失败");

    assert_eq!(resp.status().as_u16(), 409);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(extract_code(&body), "busy_another_active");

    // 清理：中止后台任务
    first_task.abort();
    srv.shutdown().await;
}

// ── 404 Not Found ──

#[tokio::test]
async fn test_404_approval_not_found() {
    // poll 不存在的 approval_id → 404, code="approval_not_found"
    let srv = TestServer::start().await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/hook/poll", srv.base_url()))
        .header("Authorization", srv.bearer())
        .json(&serde_json::json!({
            "approval_id": uuid::Uuid::new_v4().to_string()
        }))
        .send()
        .await
        .expect("请求失败");

    assert_eq!(resp.status().as_u16(), 404);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(extract_code(&body), "approval_not_found");

    srv.shutdown().await;
}

// ── 503 Shutting Down ──

#[tokio::test]
async fn test_503_shutting_down() {
    // 调用 begin_shutdown() 后提交 → 503, code="shutting_down"
    let srv = TestServer::start().await;
    let client = http_client();

    // 标记 runtime 开始关闭
    srv.runtime.begin_shutdown();

    let resp = client
        .post(format!("{}/v1/hook/submit", srv.base_url()))
        .header("Authorization", srv.bearer())
        .header("X-Approval-Id", uuid::Uuid::new_v4().to_string())
        .json(&serde_json::json!({
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .send()
        .await
        .expect("请求失败");

    assert_eq!(resp.status().as_u16(), 503);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(extract_code(&body), "shutting_down");

    srv.shutdown().await;
}
