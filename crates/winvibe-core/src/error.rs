/// 稳定错误码常量表（§4.1）——MVP-1 冻结，仅允许新增，不允许修改语义
pub const INVALID_REQUEST: &str = "invalid_request";
pub const UNAUTHORIZED: &str = "unauthorized";
pub const ORIGIN_FORBIDDEN: &str = "origin_forbidden";
pub const BUSY_ANOTHER_ACTIVE: &str = "busy_another_active";
pub const APPROVAL_NOT_FOUND: &str = "approval_not_found";
pub const DUPLICATE_ID: &str = "duplicate_id";
pub const PAYLOAD_TOO_LARGE: &str = "payload_too_large";
pub const PAYLOAD_UNPROCESSABLE: &str = "payload_unprocessable";
pub const INTERNAL_ERROR: &str = "internal_error";
pub const SHUTTING_DOWN: &str = "shutting_down";

/// HTTP 错误响应体（扁平 JSON）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub trace_id: Option<String>,
    pub approval_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_codes_are_snake_case() {
        let codes = [
            INVALID_REQUEST,
            UNAUTHORIZED,
            ORIGIN_FORBIDDEN,
            BUSY_ANOTHER_ACTIVE,
            APPROVAL_NOT_FOUND,
            DUPLICATE_ID,
            PAYLOAD_TOO_LARGE,
            PAYLOAD_UNPROCESSABLE,
            INTERNAL_ERROR,
            SHUTTING_DOWN,
        ];
        for code in codes {
            assert!(
                code.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "error code '{code}' is not snake_case"
            );
        }
    }
}
