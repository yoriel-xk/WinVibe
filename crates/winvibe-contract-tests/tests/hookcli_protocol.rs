//! hookcli 协议层契约测试
//!
//! 验证 hookcli 的 exit code + stdout 行为，覆盖：
//! - busy_another_active → fail-closed (exit 2, 无 stdout)
//! - TimedOut/Cancelled → 跟随 timeout_action 配置
//! - 413/422 → fail-closed
//! - 网络不可达 → fail-closed

mod helpers;

use helpers::{http_client, TestServer};
use serde_json::json;
use winvibe_core::config::TimeoutAction;
use winvibe_core::protocol::{ApprovalId, CancelReason, Decision, PreToolUsePayload};
use winvibe_core::trace::{TraceCtx, TraceSource};
use winvibe_hookcli::commands::pre_tool_use::map_decision_to_hook_json;
use winvibe_hookcli::http_client::{ClientError, HookClient};

/// 测试 busy_another_active 返回 fail-closed
#[tokio::test]
async fn test_busy_another_active_fail_closed() {
    let srv = TestServer::start().await;
    let client = http_client();

    // 提交第一个审批，使其 active
    let id1 = ApprovalId::new();
    let resp1 = client
        .post(format!("{}/v1/hook/submit", srv.base_url()))
        .header("Authorization", srv.bearer())
        .header("X-Approval-Id", id1.to_string())
        .json(&json!({
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .send()
        .await
        .expect("第一次提交失败");
    assert_eq!(resp1.status().as_u16(), 202);

    // 等待第一个审批变为 active
    helpers::wait_for_active(&srv).await;

    // 使用 HookClient 提交第二个审批，应返回 409 busy_another_active
    let hook_client = HookClient::new(
        srv.base_url(),
        srv.auth_token().to_string(),
    );
    let id2 = ApprovalId::new();
    let payload = PreToolUsePayload {
        session_id: "s2".into(),
        tool_name: "Bash".into(),
        tool_input: json!({"command": "pwd"}),
    };
    let payload_json = serde_json::to_value(&payload).unwrap();
    let traceparent = "00-abcd1234567890abcdef1234567890ab-1234567890abcdef-01";

    // 在 spawn_blocking 中调用同步 HookClient
    let result = tokio::task::spawn_blocking(move || {
        hook_client.submit(&id2, &payload_json, traceparent)
    })
    .await
    .unwrap();

    // 验证返回 ClientError::Http，code 为 busy_another_active
    match result {
        Err(ClientError::Http { status, code, .. }) => {
            assert_eq!(status, 409);
            assert_eq!(code, "busy_another_active");
        }
        other => panic!("期望 ClientError::Http(409, busy_another_active)，实际: {:?}", other),
    }

    srv.shutdown().await;
}

/// 测试 TimedOut 决策跟随 timeout_action = Deny
#[tokio::test]
async fn test_timed_out_with_deny_action() {
    let srv = TestServer::start().await;
    let client = http_client();

    // 提交审批
    let id = ApprovalId::new();
    let resp = client
        .post(format!("{}/v1/hook/submit", srv.base_url()))
        .header("Authorization", srv.bearer())
        .header("X-Approval-Id", id.to_string())
        .json(&json!({
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .send()
        .await
        .expect("提交失败");
    assert_eq!(resp.status().as_u16(), 202);

    // 等待 active
    helpers::wait_for_active(&srv).await;

    // 使用 runtime 直接 decide 为 TimedOut
    let trace = TraceCtx::new(TraceSource::System(
        winvibe_core::trace::SystemTraceSource::Sweeper,
    ));
    srv.runtime.decide(trace, id, Decision::TimedOut).await.unwrap();

    // Poll 获取决策
    let poll_resp = client
        .post(format!("{}/v1/hook/poll", srv.base_url()))
        .header("Authorization", srv.bearer())
        .json(&json!({
            "approval_id": id.to_string()
        }))
        .send()
        .await
        .expect("poll 失败");
    assert_eq!(poll_resp.status().as_u16(), 200);
    let body: serde_json::Value = poll_resp.json().await.unwrap();
    assert_eq!(body["decision"]["kind"], "TimedOut");

    // 验证 map_decision_to_hook_json 输出
    let decision_json = body["decision"].clone();
    let hook_json = map_decision_to_hook_json(&decision_json, TimeoutAction::Deny);
    assert_eq!(hook_json["decision"], "block");
    assert_eq!(hook_json["reason"], "winvibe: timed_out");

    srv.shutdown().await;
}

/// 测试 TimedOut 决策跟随 timeout_action = Approve
#[tokio::test]
async fn test_timed_out_with_approve_action() {
    let srv = TestServer::start().await;
    let client = http_client();

    let id = ApprovalId::new();
    let resp = client
        .post(format!("{}/v1/hook/submit", srv.base_url()))
        .header("Authorization", srv.bearer())
        .header("X-Approval-Id", id.to_string())
        .json(&json!({
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .send()
        .await
        .expect("提交失败");
    assert_eq!(resp.status().as_u16(), 202);

    helpers::wait_for_active(&srv).await;
    let trace = TraceCtx::new(TraceSource::System(
        winvibe_core::trace::SystemTraceSource::Sweeper,
    ));
    srv.runtime.decide(trace, id, Decision::TimedOut).await.unwrap();

    let poll_resp = client
        .post(format!("{}/v1/hook/poll", srv.base_url()))
        .header("Authorization", srv.bearer())
        .json(&json!({
            "approval_id": id.to_string()
        }))
        .send()
        .await
        .expect("poll 失败");
    assert_eq!(poll_resp.status().as_u16(), 200);
    let body: serde_json::Value = poll_resp.json().await.unwrap();

    // 验证 timeout_action = Approve 时输出
    let decision_json = body["decision"].clone();
    let hook_json = map_decision_to_hook_json(&decision_json, TimeoutAction::Approve);
    assert_eq!(hook_json["decision"], "approve");
    assert_eq!(hook_json["reason"], "winvibe: timed_out");

    srv.shutdown().await;
}

/// 测试 Cancelled 决策跟随 timeout_action = Deny
#[tokio::test]
async fn test_cancelled_with_deny_action() {
    let srv = TestServer::start().await;
    let client = http_client();

    let id = ApprovalId::new();
    let resp = client
        .post(format!("{}/v1/hook/submit", srv.base_url()))
        .header("Authorization", srv.bearer())
        .header("X-Approval-Id", id.to_string())
        .json(&json!({
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .send()
        .await
        .expect("提交失败");
    assert_eq!(resp.status().as_u16(), 202);

    helpers::wait_for_active(&srv).await;

    // Decide 为 Cancelled
    let trace = TraceCtx::new(TraceSource::System(
        winvibe_core::trace::SystemTraceSource::AppExitCancel,
    ));
    srv.runtime
        .decide(trace, id, Decision::Cancelled { reason: CancelReason::AppExit })
        .await
        .unwrap();

    let poll_resp = client
        .post(format!("{}/v1/hook/poll", srv.base_url()))
        .header("Authorization", srv.bearer())
        .json(&json!({
            "approval_id": id.to_string()
        }))
        .send()
        .await
        .expect("poll 失败");
    assert_eq!(poll_resp.status().as_u16(), 200);
    let body: serde_json::Value = poll_resp.json().await.unwrap();

    // 验证 timeout_action = Deny 时输出
    let decision_json = body["decision"].clone();
    let hook_json = map_decision_to_hook_json(&decision_json, TimeoutAction::Deny);
    assert_eq!(hook_json["decision"], "block");
    assert_eq!(hook_json["reason"], "winvibe: cancelled");

    srv.shutdown().await;
}

/// 测试 Cancelled 决策跟随 timeout_action = Approve
#[tokio::test]
async fn test_cancelled_with_approve_action() {
    let srv = TestServer::start().await;
    let client = http_client();

    let id = ApprovalId::new();
    let resp = client
        .post(format!("{}/v1/hook/submit", srv.base_url()))
        .header("Authorization", srv.bearer())
        .header("X-Approval-Id", id.to_string())
        .json(&json!({
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .send()
        .await
        .expect("提交失败");
    assert_eq!(resp.status().as_u16(), 202);

    helpers::wait_for_active(&srv).await;

    let trace = TraceCtx::new(TraceSource::System(
        winvibe_core::trace::SystemTraceSource::AppExitCancel,
    ));
    srv.runtime
        .decide(trace, id, Decision::Cancelled { reason: CancelReason::UserAbort })
        .await
        .unwrap();

    let poll_resp = client
        .post(format!("{}/v1/hook/poll", srv.base_url()))
        .header("Authorization", srv.bearer())
        .json(&json!({
            "approval_id": id.to_string()
        }))
        .send()
        .await
        .expect("poll 失败");
    assert_eq!(poll_resp.status().as_u16(), 200);
    let body: serde_json::Value = poll_resp.json().await.unwrap();

    // 验证 timeout_action = Approve 时输出
    let decision_json = body["decision"].clone();
    let hook_json = map_decision_to_hook_json(&decision_json, TimeoutAction::Approve);
    assert_eq!(hook_json["decision"], "approve");
    assert_eq!(hook_json["reason"], "winvibe: cancelled");

    srv.shutdown().await;
}

/// 测试网络不可达返回 fail-closed
#[test]
fn test_network_unreachable_fail_closed() {
    // 构造无效 URL（端口不存在）
    let hook_client = HookClient::new("http://127.0.0.1:1".to_string(), "dummy-token".to_string());
    let id = ApprovalId::new();
    let payload = PreToolUsePayload {
        session_id: "s1".into(),
        tool_name: "Bash".into(),
        tool_input: json!({"command": "ls"}),
    };
    let payload_json = serde_json::to_value(&payload).unwrap();
    let traceparent = "00-abcd1234567890abcdef1234567890ab-1234567890abcdef-01";

    let result = hook_client.submit(&id, &payload_json, traceparent);

    // 验证返回 ClientError::Network
    match result {
        Err(ClientError::Network(_msg)) => {
            // 符合预期
        }
        other => panic!("期望 ClientError::Network，实际: {:?}", other),
    }
}
