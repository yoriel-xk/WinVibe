use axum::http::HeaderMap;
use subtle::ConstantTimeEq;

/// Bearer token 校验，匹配则通过，否则返回 AuthError
pub fn validate_bearer_token(headers: &HeaderMap, expected: &str) -> Result<(), AuthError> {
    // 获取 Authorization header，缺失则返回 Missing 错误
    let header = headers.get("Authorization").ok_or(AuthError::Missing)?;
    // 转换为字符串，失败则返回 Invalid 错误
    let value = header.to_str().map_err(|_| AuthError::Invalid)?;
    // 去掉 "Bearer " 前缀，格式不符则返回 Invalid 错误
    let token = value.strip_prefix("Bearer ").ok_or(AuthError::Invalid)?;

    // 使用常量时间比较防止时序攻击
    if token.as_bytes().ct_eq(expected.as_bytes()).unwrap_u8() == 0 {
        return Err(AuthError::Mismatch);
    }
    Ok(())
}

/// Bearer token 校验错误
#[derive(Debug)]
pub enum AuthError {
    /// Authorization header 缺失
    Missing,
    /// Authorization header 格式非法
    Invalid,
    /// token 不匹配
    Mismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;

    #[test]
    fn missing_token_returns_401() {
        // 不带 Authorization header 的请求应返回 Missing 错误
        let req = Request::builder()
            .uri("/v1/hook/submit")
            .body(Body::empty())
            .unwrap();
        let result = validate_bearer_token(req.headers(), "secret-token-abc123");
        assert!(matches!(result, Err(AuthError::Missing)));
    }

    #[test]
    fn wrong_token_returns_401() {
        // 带错误 token 的请求应返回 Mismatch 错误
        let req = Request::builder()
            .uri("/v1/hook/submit")
            .header("Authorization", "Bearer wrong-token")
            .body(Body::empty())
            .unwrap();
        let result = validate_bearer_token(req.headers(), "secret-token-abc123");
        assert!(matches!(result, Err(AuthError::Mismatch)));
    }

    #[test]
    fn correct_token_passes() {
        // 带正确 token 的请求应通过校验
        let req = Request::builder()
            .uri("/v1/hook/submit")
            .header("Authorization", "Bearer secret-token-abc123")
            .body(Body::empty())
            .unwrap();
        let result = validate_bearer_token(req.headers(), "secret-token-abc123");
        assert!(result.is_ok());
    }
}
