use axum::http::HeaderMap;
use std::net::IpAddr;

/// 校验 Origin/Host header 是否为 loopback 地址
pub fn validate_origin(headers: &HeaderMap) -> Result<(), OriginError> {
    // 检查 Origin header（值为 URL，如 "http://127.0.0.1:59999"）
    if let Some(origin) = headers.get("Origin") {
        let origin_str = origin.to_str().map_err(|_| OriginError::InvalidHeader)?;
        // Origin 格式: scheme://host[:port]
        let after_scheme = origin_str
            .split("//")
            .nth(1)
            .unwrap_or(origin_str);
        let host_part = extract_host(after_scheme);
        check_loopback(host_part)?;
    }

    // 检查 Host header（值为 host[:port]）
    if let Some(host) = headers.get("Host") {
        let host_str = host.to_str().map_err(|_| OriginError::InvalidHeader)?;
        let host_part = extract_host(host_str);
        check_loopback(host_part)?;
    }

    Ok(())
}

/// 从 host[:port] 或 [ipv6]:port 格式中提取 host 部分
fn extract_host(s: &str) -> &str {
    if let Some(rest) = s.strip_prefix('[') {
        // IPv6 括号格式: [::1]:port
        rest.split(']').next().unwrap_or(s)
    } else {
        // IPv4 或域名: host:port
        s.split(':').next().unwrap_or(s)
    }
}

/// 检查 host 字符串是否为 loopback 地址或 localhost
fn check_loopback(host: &str) -> Result<(), OriginError> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        // 能解析为 IP 地址时，检查是否为 loopback
        if !ip.is_loopback() {
            return Err(OriginError::NonLoopback);
        }
    } else if host != "localhost" {
        // 非 IP 地址时，只允许 "localhost"
        return Err(OriginError::NonLoopback);
    }
    Ok(())
}

/// Origin/Host 校验错误
#[derive(Debug)]
pub enum OriginError {
    /// header 值非法（无法解析为字符串）
    InvalidHeader,
    /// 非 loopback 地址
    NonLoopback,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn loopback_host_passes() {
        // 127.0.0.1 是 loopback 地址，应通过校验
        let mut headers = HeaderMap::new();
        headers.insert("Host", "127.0.0.1:59999".parse().unwrap());
        assert!(validate_origin(&headers).is_ok());
    }

    #[test]
    fn external_host_rejected() {
        // 192.168.x.x 不是 loopback 地址，应被拒绝
        let mut headers = HeaderMap::new();
        headers.insert("Host", "192.168.1.1:59999".parse().unwrap());
        assert!(validate_origin(&headers).is_err());
    }

    #[test]
    fn localhost_passes() {
        // localhost 主机名应通过校验
        let mut headers = HeaderMap::new();
        headers.insert("Host", "localhost:59999".parse().unwrap());
        assert!(validate_origin(&headers).is_ok());
    }

    #[test]
    fn loopback_origin_passes() {
        // Origin 中带 127.0.0.1 的应通过校验
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "http://127.0.0.1:59999".parse().unwrap());
        assert!(validate_origin(&headers).is_ok());
    }

    #[test]
    fn external_origin_rejected() {
        // Origin 中带外网 IP 的应被拒绝
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "http://192.168.1.100:3000".parse().unwrap());
        assert!(validate_origin(&headers).is_err());
    }

    #[test]
    fn localhost_origin_passes() {
        // Origin 中带 localhost 的应通过校验
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "http://localhost:3000".parse().unwrap());
        assert!(validate_origin(&headers).is_ok());
    }

    #[test]
    fn external_origin_with_loopback_host_rejected() {
        // Origin 中带外网域名、Host 中带 loopback，Origin 检查应失败
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "http://evil.com:3000".parse().unwrap());
        headers.insert("Host", "127.0.0.1:59999".parse().unwrap());
        assert!(validate_origin(&headers).is_err());
    }

    #[test]
    fn ipv6_loopback_host_passes() {
        // IPv6 loopback [::1] 应通过校验
        let mut headers = HeaderMap::new();
        headers.insert("Host", "[::1]:59999".parse().unwrap());
        assert!(validate_origin(&headers).is_ok());
    }

    #[test]
    fn ipv6_loopback_origin_passes() {
        // Origin 中带 IPv6 loopback [::1] 的应通过校验
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "http://[::1]:59999".parse().unwrap());
        assert!(validate_origin(&headers).is_ok());
    }
}
