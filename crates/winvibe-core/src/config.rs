// config.rs — WinVibe 配置两段式校验
// 第一段：RawWinvibeConfig（从 TOML 反序列化）
// 第二段：WinvibeConfig（校验后的强类型配置）

use std::net::IpAddr;

const MIN_APPROVAL_TTL_MS: u64 = 5_000;
const MIN_AUTH_TOKEN_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("bind address is not a valid IP literal: {raw}")]
    InvalidBindAddress { raw: String },
    #[error("bind address must be loopback, got {raw}")]
    BindNotLoopback { raw: String },
    #[error("port must be 1..=65535, got {raw}")]
    PortOutOfRange { raw: String },
    #[error("port 0 is not allowed in production config")]
    PortZeroDisallowed,
    #[error("approval_ttl_ms ({got}) below minimum ({min})")]
    StaleTimeoutTooSmall { got: u64, min: u64 },
    #[error("auth_token format invalid (expect 32+ hex chars)")]
    AuthTokenFormatInvalid,
    #[error("auth_token missing")]
    MissingAuthToken,
    #[error("timeout_action must be \"deny\" or \"approve\", got {raw}")]
    InvalidTimeoutAction { raw: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthToken(String);

impl AuthToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct WinvibeConfig {
    pub bind: IpAddr,
    pub port: u16,
    pub auth_token: AuthToken,
    pub approval_ttl_ms: u64,
    pub max_cached: usize,
    pub timeout_action: TimeoutAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeoutAction {
    #[default]
    Deny,
    Approve,
}

#[derive(Debug, serde::Deserialize)]
pub struct RawWinvibeConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_port")]
    pub port: String,
    pub auth_token: Option<String>,
    #[serde(default = "default_ttl")]
    pub approval_ttl_ms: u64,
    #[serde(default = "default_cached")]
    pub max_cached: usize,
    #[serde(default)]
    pub timeout_action: Option<String>,
}

fn default_bind() -> String { "127.0.0.1".into() }
fn default_port() -> String { "59999".into() }
fn default_ttl() -> u64 { 300_000 }
fn default_cached() -> usize { 64 }

impl RawWinvibeConfig {
    pub fn validate(self) -> Result<WinvibeConfig, ConfigValidationError> {
        // 校验 bind 地址
        let bind: IpAddr = self.bind.parse()
            .map_err(|_| ConfigValidationError::InvalidBindAddress { raw: self.bind.clone() })?;
        if !bind.is_loopback() {
            return Err(ConfigValidationError::BindNotLoopback { raw: self.bind });
        }

        // 校验端口
        let port: u16 = self.port.parse()
            .map_err(|_| ConfigValidationError::PortOutOfRange { raw: self.port.clone() })?;
        if port == 0 {
            return Err(ConfigValidationError::PortZeroDisallowed);
        }

        // 校验 auth_token
        let token_str = match &self.auth_token {
            None => return Err(ConfigValidationError::MissingAuthToken),
            Some(t) => t,
        };
        if token_str.len() < MIN_AUTH_TOKEN_LEN
            || !token_str.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(ConfigValidationError::AuthTokenFormatInvalid);
        }
        let auth_token = AuthToken(token_str.clone());

        // 校验 TTL
        if self.approval_ttl_ms < MIN_APPROVAL_TTL_MS {
            return Err(ConfigValidationError::StaleTimeoutTooSmall {
                got: self.approval_ttl_ms,
                min: MIN_APPROVAL_TTL_MS,
            });
        }

        // 校验 timeout_action
        let timeout_action = match self.timeout_action.as_deref() {
            None | Some("deny") => TimeoutAction::Deny,
            Some("approve") => TimeoutAction::Approve,
            Some(other) => return Err(ConfigValidationError::InvalidTimeoutAction {
                raw: other.to_string(),
            }),
        };

        Ok(WinvibeConfig {
            bind,
            port,
            auth_token,
            approval_ttl_ms: self.approval_ttl_ms,
            max_cached: self.max_cached,
            timeout_action,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_raw_config_validates() {
        let raw = RawWinvibeConfig {
            bind: "127.0.0.1".into(),
            port: "59999".into(),
            auth_token: Some("a".repeat(32)),
            approval_ttl_ms: 300_000,
            max_cached: 64,
            timeout_action: None,
        };
        let config = raw.validate().unwrap();
        assert_eq!(config.port, 59999);
        assert_eq!(config.timeout_action, TimeoutAction::Deny);
        assert_eq!(config.approval_ttl_ms, 300_000);
    }

    #[test]
    fn non_loopback_bind_rejected() {
        let raw = RawWinvibeConfig {
            bind: "192.168.1.1".into(),
            port: "59999".into(),
            auth_token: Some("a".repeat(32)),
            approval_ttl_ms: 300_000,
            max_cached: 64,
            timeout_action: None,
        };
        let err = raw.validate().unwrap_err();
        assert!(matches!(err, ConfigValidationError::BindNotLoopback { .. }));
    }

    #[test]
    fn invalid_bind_rejected() {
        let raw = RawWinvibeConfig {
            bind: "localhost".into(),
            port: "59999".into(),
            auth_token: Some("a".repeat(32)),
            approval_ttl_ms: 300_000,
            max_cached: 64,
            timeout_action: None,
        };
        let err = raw.validate().unwrap_err();
        assert!(matches!(err, ConfigValidationError::InvalidBindAddress { .. }));
    }

    #[test]
    fn timeout_action_approve() {
        let raw = RawWinvibeConfig {
            bind: "127.0.0.1".into(),
            port: "59999".into(),
            auth_token: Some("a".repeat(32)),
            approval_ttl_ms: 300_000,
            max_cached: 64,
            timeout_action: Some("approve".into()),
        };
        let config = raw.validate().unwrap();
        assert_eq!(config.timeout_action, TimeoutAction::Approve);
    }

    #[test]
    fn timeout_action_invalid_rejected() {
        let raw = RawWinvibeConfig {
            bind: "127.0.0.1".into(),
            port: "59999".into(),
            auth_token: Some("a".repeat(32)),
            approval_ttl_ms: 300_000,
            max_cached: 64,
            timeout_action: Some("skip".into()),
        };
        let err = raw.validate().unwrap_err();
        assert!(matches!(err, ConfigValidationError::InvalidTimeoutAction { .. }));
    }

    #[test]
    fn missing_auth_token() {
        let raw = RawWinvibeConfig {
            bind: "127.0.0.1".into(),
            port: "59999".into(),
            auth_token: None,
            approval_ttl_ms: 300_000,
            max_cached: 64,
            timeout_action: None,
        };
        let err = raw.validate().unwrap_err();
        assert!(matches!(err, ConfigValidationError::MissingAuthToken));
    }

    #[test]
    fn ttl_too_small_rejected() {
        let raw = RawWinvibeConfig {
            bind: "127.0.0.1".into(),
            port: "59999".into(),
            auth_token: Some("a".repeat(32)),
            approval_ttl_ms: 500,
            max_cached: 64,
            timeout_action: None,
        };
        let err = raw.validate().unwrap_err();
        assert!(matches!(err, ConfigValidationError::StaleTimeoutTooSmall { .. }));
    }

    #[test]
    fn ipv6_loopback_accepted() {
        let raw = RawWinvibeConfig {
            bind: "::1".into(),
            port: "59999".into(),
            auth_token: Some("a".repeat(32)),
            approval_ttl_ms: 300_000,
            max_cached: 64,
            timeout_action: None,
        };
        let config = raw.validate().unwrap();
        assert!(config.bind.is_loopback());
    }
}
