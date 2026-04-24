//! 幂等性契约测试（§1.6 + §5.2）
//! 验证 submit 接口的幂等行为：相同 id+指纹、相同 id 不同指纹、不同 id 相同指纹

mod helpers;

use helpers::{TestServer, http_client, extract_code, wait_for_active};
use serde_json::Value;

// ── 幂等：相同 id + 相同指纹 → 返回已有审批 ──

#[tokio::test]
async fn same_id_same_fingerprint_returns_existing() {
    // 同一 approval_id + 同一 payload 提交两次 → 第二次应返回 200 或 202（幂等）
    // 幂等检查发生在 wait 之前，第二次请求不会阻塞
    let srv = TestServer::start().await;
    let client = http_client();
    let base = srv.base_url();
    let bearer = srv.bearer();
    let id = uuid::Uuid::new_v4().to_string();

    let body = serde_json::json!({
        "session_id": "sess-idem",
        "tool_name": "Bash",
        "tool_input": {"command": "echo hello"}
    });

    // 第一次提交放到后台（会阻塞等待决策）
    let client_bg = client.clone();
    let base_bg = base.clone();
    let bearer_bg = bearer.clone();
    let id_bg = id.clone();
    let body_bg = body.clone();
    let first_task = tokio::spawn(async move {
        client_bg
            .post(format!("{base_bg}/v1/hook/submit"))
            .header("Authorization", &bearer_bg)
            .header("X-Approval-Id", &id_bg)
            .json(&body_bg)
            .send()
            .await
    });

    // 等待第一个请求注册到 runtime
    wait_for_active(&srv).await;

    // 第二次提交：相同 id + 相同 payload → 幂等返回 Existing（202 Pending）
    let resp = client
        .post(format!("{base}/v1/hook/submit"))
        .header("Authorization", &bearer)
        .header("X-Approval-Id", &id)
        .json(&body)
        .send()
        .await
        .expect("第二次请求失败");

    let status = resp.status().as_u16();
    assert!(
        status == 200 || status == 202,
        "幂等重复提交应返回 200 或 202，实际: {status}"
    );

    first_task.abort();
    srv.shutdown().await;
}

// ── 相同 id + 不同指纹 → 409 duplicate_id ──

#[tokio::test]
async fn same_id_different_fingerprint_returns_409_duplicate_id() {
    // 同一 approval_id 但不同 payload → 409, code="duplicate_id"
    // duplicate_id 检查发生在 wait 之前，不会阻塞
    let srv = TestServer::start().await;
    let client = http_client();
    let base = srv.base_url();
    let bearer = srv.bearer();
    let id = uuid::Uuid::new_v4().to_string();

    let body1 = serde_json::json!({
        "session_id": "sess-dup",
        "tool_name": "Bash",
        "tool_input": {"command": "ls"}
    });
    let body2 = serde_json::json!({
        "session_id": "sess-dup",
        "tool_name": "Write",
        "tool_input": {"path": "/tmp/x"}
    });

    // 第一次提交放到后台
    let client_bg = client.clone();
    let base_bg = base.clone();
    let bearer_bg = bearer.clone();
    let id_bg = id.clone();
    let first_task = tokio::spawn(async move {
        client_bg
            .post(format!("{base_bg}/v1/hook/submit"))
            .header("Authorization", &bearer_bg)
            .header("X-Approval-Id", &id_bg)
            .json(&body1)
            .send()
            .await
    });

    // 等待第一个请求注册
    wait_for_active(&srv).await;

    // 第二次提交：相同 id 但不同 payload → 409 duplicate_id
    let resp = client
        .post(format!("{base}/v1/hook/submit"))
        .header("Authorization", &bearer)
        .header("X-Approval-Id", &id)
        .json(&body2)
        .send()
        .await
        .expect("第二次请求失败");

    assert_eq!(resp.status().as_u16(), 409);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(extract_code(&body), "duplicate_id");

    first_task.abort();
    srv.shutdown().await;
}

// ── 不同 id + 相同指纹（已有活跃）→ 409 busy_another_active ──

#[tokio::test]
async fn different_id_same_fingerprint_with_active_returns_409_busy() {
    // 不同 approval_id 但相同 payload，第一个仍活跃 → 409, code="busy_another_active"
    // busy 检查发生在 wait 之前，第二次请求不会阻塞
    let srv = TestServer::start().await;
    let client = http_client();
    let base = srv.base_url();
    let bearer = srv.bearer();

    let id1 = uuid::Uuid::new_v4().to_string();
    let id2 = uuid::Uuid::new_v4().to_string();

    let body = serde_json::json!({
        "session_id": "sess-busy",
        "tool_name": "Bash",
        "tool_input": {"command": "pwd"}
    });

    // 第一次提交放到后台（会阻塞等待决策）
    let client_bg = client.clone();
    let base_bg = base.clone();
    let bearer_bg = bearer.clone();
    let body_bg = body.clone();
    let first_task = tokio::spawn(async move {
        client_bg
            .post(format!("{base_bg}/v1/hook/submit"))
            .header("Authorization", &bearer_bg)
            .header("X-Approval-Id", &id1)
            .json(&body_bg)
            .send()
            .await
    });

    // 等待第一个请求注册
    wait_for_active(&srv).await;

    // 第二次提交：不同 id + 相同 payload → 409 busy_another_active
    let resp = client
        .post(format!("{base}/v1/hook/submit"))
        .header("Authorization", &bearer)
        .header("X-Approval-Id", &id2)
        .json(&body)
        .send()
        .await
        .expect("第二次请求失败");

    assert_eq!(resp.status().as_u16(), 409);
    let body_resp: Value = resp.json().await.unwrap();
    assert_eq!(extract_code(&body_resp), "busy_another_active");

    first_task.abort();
    srv.shutdown().await;
}
