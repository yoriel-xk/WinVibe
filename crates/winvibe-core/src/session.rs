use sha2::{Digest, Sha256};

/// 计算 session_hash：SHA256(session_id || "winvibe-sess-v1") 取前 8 字节转 16 hex
pub fn compute_session_hash(session_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update(b"winvibe-sess-v1");
    let hash = hasher.finalize();
    hex_encode(&hash[..8])
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_hash_deterministic() {
        let h1 = compute_session_hash("session-abc-123");
        let h2 = compute_session_hash("session-abc-123");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16); // 8 bytes → 16 hex
    }

    #[test]
    fn session_hash_different_for_different_ids() {
        let h1 = compute_session_hash("session-1");
        let h2 = compute_session_hash("session-2");
        assert_ne!(h1, h2);
    }
}
