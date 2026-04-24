use axum::http::{HeaderMap, StatusCode};
use axum::{extract::State, Json, Extension};
use serde::Serialize;
use std::time::Duration;
use winvibe_core::protocol::{ApprovalId, PreToolUsePayload};
use winvibe_core::trace::TraceCtx;
use crate::runtime::WaitOutcome;
use super::AppState;

/// 最长等待时间
const MAX_WAIT: Duration = Duration::from_secs(25);

/// submit 接口响应体，按 status 字段区分已决策/待决策
#[derive(Serialize)]
#[serde(tag = "status")]
pub enum SubmitResponse {
    #[serde(rename = "decided")]
    Decided {
        approval_id: String,
        decision: serde_json::Value,
    },
    #[serde(rename = "pending")]
    Pending {
        approval_id: String,
    },
}

/// POST /v1/hook/submit 处理函数
pub async fn handle_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(trace): Extension<TraceCtx>,
    Json(payload): Json<PreToolUsePayload>,
) -> Result<
    (StatusCode, Json<SubmitResponse>),
    (StatusCode, Json<winvibe_core::error::ErrorResponse>),
> {
    // 从 X-Approval-Id header 解析幂等键
    let approval_id: ApprovalId = headers
        .get("X-Approval-Id")
        .ok_or_else(|| make_error(
            StatusCode::BAD_REQUEST,
            winvibe_core::error::INVALID_REQUEST,
            "X-Approval-Id header is required",
            &trace,
        ))?
        .to_str()
        .map_err(|_| make_error(
            StatusCode::BAD_REQUEST,
            winvibe_core::error::INVALID_REQUEST,
            "X-Approval-Id is not valid UTF-8",
            &trace,
        ))?
        .parse::<ApprovalId>()
        .map_err(|_| make_error(
            StatusCode::BAD_REQUEST,
            winvibe_core::error::INVALID_REQUEST,
            "X-Approval-Id is not a valid UUID v4",
            &trace,
        ))?;

    // 校验 tool_input 必须是 JSON object
    if !payload.tool_input.is_object() {
        return Err(make_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            winvibe_core::error::PAYLOAD_UNPROCESSABLE,
            "tool_input must be a JSON object",
            &trace,
        ));
    }

    let trace_id_hex = trace.trace_id_hex();
    match state.runtime.submit_pre_tool_use(trace, approval_id, payload, MAX_WAIT).await {
        Ok(WaitOutcome::Decided { approval, .. }) => {
            let decision = approval
                .decision()
                .map(|d| serde_json::to_value(d).unwrap_or(serde_json::Value::Null))
                .unwrap_or(serde_json::Value::Null);
            Ok((StatusCode::OK, Json(SubmitResponse::Decided {
                approval_id: approval.id.to_string(),
                decision,
            })))
        }
        Ok(WaitOutcome::Existing { approval, .. }) => {
            // 幂等重复提交：若已决策返回 200，否则返回 202
            if let Some(d) = approval.decision() {
                let decision = serde_json::to_value(d).unwrap_or(serde_json::Value::Null);
                Ok((StatusCode::OK, Json(SubmitResponse::Decided {
                    approval_id: approval.id.to_string(),
                    decision,
                })))
            } else {
                Ok((StatusCode::ACCEPTED, Json(SubmitResponse::Pending {
                    approval_id: approval.id.to_string(),
                })))
            }
        }
        Ok(WaitOutcome::Pending { id, .. }) => {
            Ok((StatusCode::ACCEPTED, Json(SubmitResponse::Pending {
                approval_id: id.to_string(),
            })))
        }
        // submit_pre_tool_use 内部会等待，不会返回 Created
        Ok(WaitOutcome::Created { .. }) => unreachable!("submit_pre_tool_use 内部会等待"),
        Err(e) => Err(map_runtime_error(e, Some(trace_id_hex))),
    }
}

/// 构造错误响应
fn make_error(
    status: StatusCode,
    code: &str,
    message: &str,
    trace: &TraceCtx,
) -> (StatusCode, Json<winvibe_core::error::ErrorResponse>) {
    (status, Json(winvibe_core::error::ErrorResponse {
        code: code.into(),
        message: message.into(),
        trace_id: Some(trace.trace_id_hex()),
        approval_id: None,
    }))
}

/// 将 RuntimeError 映射为 HTTP 错误响应
pub(crate) fn map_runtime_error(
    e: crate::error::RuntimeError,
    trace_id: Option<String>,
) -> (StatusCode, Json<winvibe_core::error::ErrorResponse>) {
    use crate::error::RuntimeError;
    use winvibe_core::approval::types::{EnqueueError, DecideError, CancelError};

    let (status, code, message) = match e {
        RuntimeError::ShuttingDown =>
            (StatusCode::SERVICE_UNAVAILABLE, winvibe_core::error::SHUTTING_DOWN, "server shutting down"),
        RuntimeError::Enqueue(EnqueueError::BusyAnotherActive { .. }) =>
            (StatusCode::CONFLICT, winvibe_core::error::BUSY_ANOTHER_ACTIVE, "another approval is active"),
        RuntimeError::Enqueue(EnqueueError::DuplicateIdConflict { .. }) =>
            (StatusCode::CONFLICT, winvibe_core::error::DUPLICATE_ID, "duplicate id with different fingerprint"),
        RuntimeError::Enqueue(EnqueueError::StoreFull) =>
            (StatusCode::INTERNAL_SERVER_ERROR, winvibe_core::error::INTERNAL_ERROR, "store full"),
        RuntimeError::Decide(DecideError::NotFound { .. })
        | RuntimeError::Cancel(CancelError::NotFound { .. })
        | RuntimeError::NotFound(_) =>
            (StatusCode::NOT_FOUND, winvibe_core::error::APPROVAL_NOT_FOUND, "approval not found"),
        RuntimeError::Decide(DecideError::AlreadyDecided { .. })
        | RuntimeError::Cancel(CancelError::AlreadyDecided { .. }) =>
            (StatusCode::CONFLICT, winvibe_core::error::DUPLICATE_ID, "already decided"),
    };

    (status, Json(winvibe_core::error::ErrorResponse {
        code: code.into(),
        message: message.into(),
        trace_id,
        approval_id: None,
    }))
}
