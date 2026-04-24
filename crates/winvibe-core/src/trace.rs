use rand::Rng;
use serde::{Deserialize, Serialize};

/// 128-bit 追踪 ID，对应 W3C traceparent 中的 trace-id 字段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraceId([u8; 16]);

impl TraceId {
    /// 生成随机 TraceId
    pub fn generate() -> Self {
        let mut bytes = [0u8; 16];
        rand::thread_rng().fill(&mut bytes);
        Self(bytes)
    }

    /// 转为 32 字符小写十六进制字符串
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(32);
        for b in &self.0 {
            s.push_str(&format!("{:02x}", b));
        }
        s
    }

    /// 从 32 字符十六进制字符串解析
    pub fn from_hex(hex: &str) -> Result<Self, TraceParseError> {
        if hex.len() != 32 {
            return Err(TraceParseError::InvalidLength { expected: 32, got: hex.len() });
        }
        let mut bytes = [0u8; 16];
        for i in 0..16 {
            bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(|_| TraceParseError::InvalidHex)?;
        }
        Ok(Self(bytes))
    }
}

impl Serialize for TraceId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for TraceId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// 64-bit Span ID，对应 W3C traceparent 中的 parent-id 字段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpanId([u8; 8]);

impl SpanId {
    /// 生成随机 SpanId
    pub fn generate() -> Self {
        let mut bytes = [0u8; 8];
        rand::thread_rng().fill(&mut bytes);
        Self(bytes)
    }

    /// 转为 16 字符小写十六进制字符串
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(16);
        for b in &self.0 {
            s.push_str(&format!("{:02x}", b));
        }
        s
    }

    /// 从 16 字符十六进制字符串解析
    pub fn from_hex(hex: &str) -> Result<Self, TraceParseError> {
        if hex.len() != 16 {
            return Err(TraceParseError::InvalidLength { expected: 16, got: hex.len() });
        }
        let mut bytes = [0u8; 8];
        for i in 0..8 {
            bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(|_| TraceParseError::InvalidHex)?;
        }
        Ok(Self(bytes))
    }
}

impl Serialize for SpanId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for SpanId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// 追踪解析错误
#[derive(Debug, thiserror::Error)]
pub enum TraceParseError {
    #[error("invalid hex length: expected {expected}, got {got}")]
    InvalidLength { expected: usize, got: usize },
    #[error("invalid hex character")]
    InvalidHex,
    #[error("invalid traceparent format")]
    InvalidFormat,
}

/// 追踪来源
#[derive(Debug, Clone, PartialEq)]
pub enum TraceSource {
    HookCliRequest,
    HudIpc,
    System(SystemTraceSource),
}

/// 系统内部追踪来源
#[derive(Debug, Clone, PartialEq)]
pub enum SystemTraceSource {
    AppExitCancel,
    Sweeper,
}

/// W3C traceparent 传播上下文
#[derive(Debug, Clone)]
pub struct TraceCtx {
    pub trace_id: TraceId,
    pub entry_span_id: SpanId,
    pub source: TraceSource,
}

impl TraceCtx {
    /// 创建新的追踪上下文，生成随机 trace_id 和 span_id
    pub fn new(source: TraceSource) -> Self {
        Self {
            trace_id: TraceId::generate(),
            entry_span_id: SpanId::generate(),
            source,
        }
    }

    /// 从 W3C traceparent 字符串解析
    pub fn from_traceparent(raw: &str, source: TraceSource) -> Result<Self, TraceParseError> {
        let parts: Vec<&str> = raw.split('-').collect();
        if parts.len() != 4 {
            return Err(TraceParseError::InvalidFormat);
        }
        if parts[0] != "00" {
            return Err(TraceParseError::InvalidFormat);
        }
        let trace_id = TraceId::from_hex(parts[1])?;
        let span_id = SpanId::from_hex(parts[2])?;
        Ok(Self { trace_id, entry_span_id: span_id, source })
    }

    /// 便捷解析，失败返回 None，默认来源为 HookCliRequest
    pub fn parse(raw: &str) -> Option<Self> {
        Self::from_traceparent(raw, TraceSource::HookCliRequest).ok()
    }

    /// 保留 trace_id，生成新的 span_id，返回子 span 上下文
    pub fn with_new_span(&self) -> Self {
        Self {
            trace_id: self.trace_id,
            entry_span_id: SpanId::generate(),
            source: self.source.clone(),
        }
    }

    /// 返回 trace_id 的十六进制字符串
    pub fn trace_id_hex(&self) -> String {
        self.trace_id.to_hex()
    }

    /// 返回 entry_span_id 的十六进制字符串
    pub fn entry_span_id_hex(&self) -> String {
        self.entry_span_id.to_hex()
    }

    /// 序列化为 W3C traceparent 格式
    pub fn to_traceparent(&self) -> String {
        format!("00-{}-{}-01", self.trace_id.to_hex(), self.entry_span_id.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_id_hex_round_trip() {
        let id = TraceId::generate();
        let hex = id.to_hex();
        assert_eq!(hex.len(), 32);
        let parsed = TraceId::from_hex(&hex).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn span_id_hex_round_trip() {
        let id = SpanId::generate();
        let hex = id.to_hex();
        assert_eq!(hex.len(), 16);
        let parsed = SpanId::from_hex(&hex).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn trace_id_from_hex_invalid_length() {
        assert!(TraceId::from_hex("abcdef").is_err());
    }

    #[test]
    fn traceparent_parse_valid() {
        let raw = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
        let ctx = TraceCtx::from_traceparent(raw, TraceSource::HookCliRequest).unwrap();
        assert_eq!(ctx.trace_id.to_hex(), "0af7651916cd43dd8448eb211c80319c");
        assert_eq!(ctx.entry_span_id.to_hex(), "b7ad6b7169203331");
    }

    #[test]
    fn traceparent_format() {
        let ctx = TraceCtx::from_traceparent(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            TraceSource::HookCliRequest,
        ).unwrap();
        let tp = ctx.to_traceparent();
        assert!(tp.starts_with("00-"));
        assert!(tp.ends_with("-01"));
    }

    #[test]
    fn parse_convenience_valid() {
        let raw = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
        let ctx = TraceCtx::parse(raw).unwrap();
        assert_eq!(ctx.trace_id.to_hex(), "0af7651916cd43dd8448eb211c80319c");
    }

    #[test]
    fn parse_convenience_invalid_returns_none() {
        assert!(TraceCtx::parse("garbage").is_none());
    }

    #[test]
    fn with_new_span_preserves_trace_id() {
        let ctx = TraceCtx::new(TraceSource::HookCliRequest);
        let child = ctx.with_new_span();
        assert_eq!(ctx.trace_id_hex(), child.trace_id_hex());
        assert_ne!(ctx.entry_span_id_hex(), child.entry_span_id_hex());
    }

    #[test]
    fn trace_id_hex_and_entry_span_id_hex() {
        let ctx = TraceCtx::new(TraceSource::HookCliRequest);
        assert_eq!(ctx.trace_id_hex().len(), 32);
        assert_eq!(ctx.entry_span_id_hex().len(), 16);
    }
}
