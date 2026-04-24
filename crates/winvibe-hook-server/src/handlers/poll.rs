use super::{
    submit::{map_runtime_error, SubmitResponse},
    AppState,
};
use crate::runtime::WaitOutcome;
use axum::http::StatusCode;
use axum::{extract::State, Extension, Json};
use serde::Deserialize;
use std::time::Duration;
use winvibe_core::protocol::ApprovalId;
use winvibe_core::trace::TraceCtx;

/// 最长等待时间
const MAX_WAIT: Duration = Duration::from_secs(25);

/// poll 接口请求体
#[derive(Deserialize)]
pub struct PollRequest {
    pub approval_id: String,
}

/// POST /v1/hook/poll 处理函数
pub async fn handle_poll(
    State(state): State<AppState>,
    Extension(trace): Extension<TraceCtx>,
    Json(body): Json<PollRequest>,
) -> Result<
    (StatusCode, Json<SubmitResponse>),
    (StatusCode, Json<winvibe_core::error::ErrorResponse>),
> {
    let trace_id_hex = trace.trace_id_hex();

    // 解析 approval_id
    let id: ApprovalId = body.approval_id.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(winvibe_core::error::ErrorResponse {
                code: winvibe_core::error::INVALID_REQUEST.into(),
                message: "invalid approval_id".into(),
                trace_id: Some(trace_id_hex.clone()),
                approval_id: None,
            }),
        )
    })?;

    match state.runtime.poll_decision(trace, id, MAX_WAIT).await {
        Ok(WaitOutcome::Decided { approval, .. }) => {
            let decision = approval
                .decision()
                .map(|d| serde_json::to_value(d).unwrap_or(serde_json::Value::Null))
                .unwrap_or(serde_json::Value::Null);
            Ok((
                StatusCode::OK,
                Json(SubmitResponse::Decided {
                    approval_id: approval.id.to_string(),
                    decision,
                }),
            ))
        }
        Ok(WaitOutcome::Pending { id, .. }) => Ok((
            StatusCode::ACCEPTED,
            Json(SubmitResponse::Pending {
                approval_id: id.to_string(),
            }),
        )),
        // poll_decision 只返回 Decided 或 Pending
        Ok(WaitOutcome::Existing { .. } | WaitOutcome::Created { .. }) => {
            unreachable!("poll_decision 只返回 Decided 或 Pending")
        }
        Err(e) => Err(map_runtime_error(e, Some(trace_id_hex))),
    }
}
