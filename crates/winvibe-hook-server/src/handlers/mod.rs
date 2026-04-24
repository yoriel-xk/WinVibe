pub mod poll;
pub mod submit;

use crate::middleware::auth::validate_bearer_token;
use crate::middleware::origin::validate_origin;
use crate::middleware::traceparent;
use crate::runtime::ApprovalRuntime;
use axum::{
    extract::Request, http::StatusCode, middleware as axum_mw, response::Response, routing::post,
    Json, Router,
};
use std::sync::Arc;
use winvibe_core::trace::TraceCtx;

/// 应用共享状态
#[derive(Clone)]
pub struct AppState {
    pub runtime: Arc<ApprovalRuntime>,
    pub auth_token: String,
}

/// 构建 axum Router，挂载三层 middleware
///
/// middleware 洋葱顺序（最后 .layer() = 最外层 = 最先执行）：
/// - traceparent 最外层：每个请求/响应都携带 trace_id
/// - auth 中间层：拒绝无效 token
/// - origin 最内层：拒绝非 loopback 来源
pub fn build_router(runtime: Arc<ApprovalRuntime>, auth_token: String) -> Router {
    let state = AppState {
        runtime,
        auth_token,
    };

    Router::new()
        .route("/v1/hook/submit", post(submit::handle_submit))
        .route("/v1/hook/poll", post(poll::handle_poll))
        // origin 最内层（最后执行）
        .layer(axum_mw::from_fn_with_state(state.clone(), origin_layer))
        // auth 中间层
        .layer(axum_mw::from_fn_with_state(state.clone(), auth_layer))
        // traceparent 最外层（最先执行）
        .layer(axum_mw::from_fn(traceparent_layer))
        .with_state(state)
}

/// auth middleware：校验 Bearer token
async fn auth_layer(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: Request,
    next: axum_mw::Next,
) -> Result<Response, (StatusCode, Json<winvibe_core::error::ErrorResponse>)> {
    let trace_id = req.extensions().get::<TraceCtx>().map(|t| t.trace_id_hex());
    validate_bearer_token(req.headers(), &state.auth_token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(winvibe_core::error::ErrorResponse {
                code: winvibe_core::error::UNAUTHORIZED.into(),
                message: "invalid or missing bearer token".into(),
                trace_id,
                approval_id: None,
            }),
        )
    })?;
    Ok(next.run(req).await)
}

/// origin middleware：校验请求来源为 loopback
async fn origin_layer(
    axum::extract::State(_state): axum::extract::State<AppState>,
    req: Request,
    next: axum_mw::Next,
) -> Result<Response, (StatusCode, Json<winvibe_core::error::ErrorResponse>)> {
    let trace_id = req.extensions().get::<TraceCtx>().map(|t| t.trace_id_hex());
    validate_origin(req.headers()).map_err(|_| {
        (
            StatusCode::FORBIDDEN,
            Json(winvibe_core::error::ErrorResponse {
                code: winvibe_core::error::ORIGIN_FORBIDDEN.into(),
                message: "origin/host not allowed".into(),
                trace_id,
                approval_id: None,
            }),
        )
    })?;
    Ok(next.run(req).await)
}

/// traceparent middleware：注入/传播 W3C traceparent
async fn traceparent_layer(mut req: Request, next: axum_mw::Next) -> Response {
    // 从请求 headers 解析或生成 TraceCtx
    let trace = traceparent::extract_trace_ctx(
        req.headers(),
        winvibe_core::trace::TraceSource::HookCliRequest,
    );
    let tp_header = trace.to_traceparent();
    req.extensions_mut().insert(trace);
    let mut resp = next.run(req).await;
    // 将 traceparent 注入响应 headers
    if let Ok(val) = tp_header.parse() {
        resp.headers_mut().insert("traceparent", val);
    }
    resp
}

/// 测试辅助：构造带 NoopSink 的 ApprovalRuntime
#[cfg(test)]
fn test_runtime() -> Arc<ApprovalRuntime> {
    use crate::sink::NoopSink;
    use winvibe_core::approval::types::ApprovalStoreLimits;
    use winvibe_core::clock::{FakeMonotonicClock, FakeWallClock};
    Arc::new(ApprovalRuntime::new(
        ApprovalStoreLimits::default(),
        Arc::new(FakeWallClock::default()),
        Arc::new(FakeMonotonicClock::new(10_000)),
        Arc::new(NoopSink),
        300_000,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::http::{HeaderName, HeaderValue};
    use axum_test::TestServer;

    /// 构造测试用 TestServer
    async fn test_app() -> TestServer {
        let runtime = test_runtime();
        let app = build_router(runtime, "test-token-abc123".into());
        // axum-test v20: TestServer::new 直接返回 TestServer，不是 Result
        TestServer::new(app)
    }

    #[tokio::test]
    async fn submit_without_auth_returns_401() {
        // 不带 Authorization header 应返回 401
        let server = test_app().await;
        let resp = server
            .post("/v1/hook/submit")
            .json(&serde_json::json!({
                "session_id": "s1",
                "tool_name": "Bash",
                "tool_input": {"cmd": "ls"}
            }))
            .await;
        resp.assert_status(StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn submit_with_auth_returns_200_or_202() {
        // 带正确 token 和 X-Approval-Id 应返回 200 或 202
        let server = test_app().await;
        let approval_id = uuid::Uuid::new_v4().to_string();
        let resp = server
            .post("/v1/hook/submit")
            .add_header(
                HeaderName::from_static("authorization"),
                HeaderValue::from_static("Bearer test-token-abc123"),
            )
            .add_header(
                HeaderName::from_static("x-approval-id"),
                HeaderValue::from_str(&approval_id).unwrap(),
            )
            .json(&serde_json::json!({
                "session_id": "s1",
                "tool_name": "Bash",
                "tool_input": {"cmd": "ls"}
            }))
            .await;
        let status = resp.status_code();
        assert!(
            status == StatusCode::OK || status == StatusCode::ACCEPTED,
            "expected 200 or 202, got {status}"
        );
    }

    #[tokio::test]
    async fn external_host_returns_403() {
        // 非 loopback Host 应返回 403
        let server = test_app().await;
        let resp = server
            .post("/v1/hook/submit")
            .add_header(
                HeaderName::from_static("authorization"),
                HeaderValue::from_static("Bearer test-token-abc123"),
            )
            .add_header(
                HeaderName::from_static("host"),
                HeaderValue::from_static("192.168.1.1:59999"),
            )
            .json(&serde_json::json!({
                "session_id": "s1",
                "tool_name": "Bash",
                "tool_input": {"cmd": "ls"}
            }))
            .await;
        resp.assert_status(StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn external_origin_returns_403() {
        // 非 loopback Origin 应返回 403
        let server = test_app().await;
        let resp = server
            .post("/v1/hook/submit")
            .add_header(
                HeaderName::from_static("authorization"),
                HeaderValue::from_static("Bearer test-token-abc123"),
            )
            .add_header(
                HeaderName::from_static("origin"),
                HeaderValue::from_static("http://192.168.1.100:3000"),
            )
            .json(&serde_json::json!({
                "session_id": "s1",
                "tool_name": "Bash",
                "tool_input": {"cmd": "ls"}
            }))
            .await;
        resp.assert_status(StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn response_contains_traceparent_header() {
        // 即使请求因缺少 X-Approval-Id 返回 400，traceparent 层仍应注入响应头
        let server = test_app().await;
        let resp = server
            .post("/v1/hook/submit")
            .add_header(
                HeaderName::from_static("authorization"),
                HeaderValue::from_static("Bearer test-token-abc123"),
            )
            .json(&serde_json::json!({
                "session_id": "s1",
                "tool_name": "Bash",
                "tool_input": {"cmd": "ls"}
            }))
            .await;
        // traceparent 响应头应存在且非空
        let tp = resp.header("traceparent");
        assert!(
            !tp.is_empty(),
            "traceparent header should be present in response"
        );
    }
}
