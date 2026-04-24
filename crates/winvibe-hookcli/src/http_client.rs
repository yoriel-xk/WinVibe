use serde::Deserialize;
use winvibe_core::protocol::ApprovalId;

/// HTTP 客户端，封装与 hook-server 的通信
pub struct HookClient {
    base_url: String,
    auth_token: String,
    agent: ureq::Agent,
}

/// 服务端响应体（与 SubmitResponse 对称）
#[derive(Debug, Deserialize)]
#[serde(tag = "status")]
pub enum ServerResponse {
    #[serde(rename = "decided")]
    Decided {
        approval_id: String,
        decision: serde_json::Value,
    },
    #[serde(rename = "pending")]
    Pending { approval_id: String },
}

/// 服务端错误响应体
#[derive(Debug, Deserialize)]
struct ErrorBody {
    code: String,
    message: String,
}

/// 客户端错误
#[derive(Debug)]
pub enum ClientError {
    /// 网络层错误（连接失败、超时等）
    Network(String),
    /// HTTP 错误（4xx/5xx）
    Http {
        status: u16,
        code: String,
        message: String,
    },
    /// 响应体解析错误
    Parse(String),
}

/// 将 ureq 错误分流为 Http 或 Network
fn classify_ureq_error(err: ureq::Error) -> ClientError {
    match err {
        ureq::Error::Status(code, response) => match response.into_json::<ErrorBody>() {
            Ok(body) => ClientError::Http {
                status: code,
                code: body.code,
                message: body.message,
            },
            Err(_) => ClientError::Http {
                status: code,
                code: "UNKNOWN".into(),
                message: "无法解析错误响应体".into(),
            },
        },
        ureq::Error::Transport(t) => ClientError::Network(t.to_string()),
    }
}

impl HookClient {
    /// 创建新的 HTTP 客户端
    pub fn new(base_url: String, auth_token: String) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout_read(std::time::Duration::from_secs(30))
            .build();
        Self {
            base_url,
            auth_token,
            agent,
        }
    }

    /// POST /v1/hook/submit
    pub fn submit(
        &self,
        approval_id: &ApprovalId,
        payload: &serde_json::Value,
        traceparent: &str,
    ) -> Result<ServerResponse, ClientError> {
        let url = format!("{}/v1/hook/submit", self.base_url);
        let resp = self
            .agent
            .post(&url)
            .set("Authorization", &format!("Bearer {}", self.auth_token))
            .set("X-Approval-Id", &approval_id.to_string())
            .set("traceparent", traceparent)
            .send_json(payload)
            .map_err(classify_ureq_error)?;

        let body: ServerResponse = resp
            .into_json()
            .map_err(|e| ClientError::Parse(e.to_string()))?;
        Ok(body)
    }

    /// POST /v1/hook/poll
    pub fn poll(
        &self,
        approval_id: &ApprovalId,
        traceparent: &str,
    ) -> Result<ServerResponse, ClientError> {
        let url = format!("{}/v1/hook/poll", self.base_url);
        let resp = self
            .agent
            .post(&url)
            .set("Authorization", &format!("Bearer {}", self.auth_token))
            .set("traceparent", traceparent)
            .send_json(ureq::json!({ "approval_id": approval_id.to_string() }))
            .map_err(classify_ureq_error)?;

        let body: ServerResponse = resp
            .into_json()
            .map_err(|e| ClientError::Parse(e.to_string()))?;
        Ok(body)
    }
}
