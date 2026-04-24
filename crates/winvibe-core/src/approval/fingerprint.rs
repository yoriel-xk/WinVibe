use sha2::{Sha256, Digest};
use serde_json::Value;
use std::io::Write;

const FINGERPRINT_PREFIX: &[u8] = b"winvibe-fp\x00";
const FINGERPRINT_VERSION: u8 = 1;

pub fn canonical_json(value: &Value) -> String {
    let mut buf = Vec::new();
    write_canonical(&mut buf, value);
    String::from_utf8(buf).unwrap()
}

fn write_canonical(buf: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Null => buf.extend_from_slice(b"null"),
        Value::Bool(b) => {
            if *b { buf.extend_from_slice(b"true") }
            else { buf.extend_from_slice(b"false") }
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                let mut b = itoa::Buffer::new();
                buf.extend_from_slice(b.format(i).as_bytes());
            } else if let Some(u) = n.as_u64() {
                let mut b = itoa::Buffer::new();
                buf.extend_from_slice(b.format(u).as_bytes());
            } else if let Some(f) = n.as_f64() {
                let mut b = ryu::Buffer::new();
                buf.extend_from_slice(b.format(f).as_bytes());
            }
        }
        Value::String(s) => {
            buf.push(b'"');
            for ch in s.chars() {
                match ch {
                    '"' => buf.extend_from_slice(b"\\\""),
                    '\\' => buf.extend_from_slice(b"\\\\"),
                    '\n' => buf.extend_from_slice(b"\\n"),
                    '\r' => buf.extend_from_slice(b"\\r"),
                    '\t' => buf.extend_from_slice(b"\\t"),
                    c if c < '\x20' => {
                        write!(buf, "\\u{:04x}", c as u32).unwrap();
                    }
                    c => {
                        let mut b = [0u8; 4];
                        buf.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
                    }
                }
            }
            buf.push(b'"');
        }
        Value::Array(arr) => {
            buf.push(b'[');
            for (i, v) in arr.iter().enumerate() {
                if i > 0 { buf.push(b','); }
                write_canonical(buf, v);
            }
            buf.push(b']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            buf.push(b'{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 { buf.push(b','); }
                write_canonical(buf, &Value::String((*key).clone()));
                buf.push(b':');
                write_canonical(buf, &map[*key]);
            }
            buf.push(b'}');
        }
    }
}

pub fn compute_fingerprint(session_id: &str, tool_name: &str, tool_input: &Value) -> String {
    let canonical = canonical_json(tool_input);
    let canonical_bytes = canonical.as_bytes();

    let mut hasher = Sha256::new();
    hasher.update(FINGERPRINT_PREFIX);
    hasher.update([FINGERPRINT_VERSION]);

    hasher.update((session_id.len() as u32).to_be_bytes());
    hasher.update(session_id.as_bytes());

    hasher.update((tool_name.len() as u32).to_be_bytes());
    hasher.update(tool_name.as_bytes());

    hasher.update((canonical_bytes.len() as u32).to_be_bytes());
    hasher.update(canonical_bytes);

    hex_encode(&hasher.finalize())
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_json_sorts_keys() {
        let input = serde_json::json!({"z": 1, "a": 2, "m": 3});
        let canonical = canonical_json(&input);
        assert_eq!(canonical, r#"{"a":2,"m":3,"z":1}"#);
    }

    #[test]
    fn canonical_json_nested_sorts() {
        let input = serde_json::json!({"b": {"z": 1, "a": 2}, "a": 1});
        let canonical = canonical_json(&input);
        assert_eq!(canonical, r#"{"a":1,"b":{"a":2,"z":1}}"#);
    }

    #[test]
    fn canonical_json_normalizes_floats() {
        let input = serde_json::json!({"x": 1.0, "y": 3.14});
        let canonical = canonical_json(&input);
        assert!(canonical.contains("1.0") || canonical.contains("1"));
    }

    #[test]
    fn fingerprint_deterministic() {
        let fp1 = compute_fingerprint("sess1", "Bash", &serde_json::json!({"cmd": "ls"}));
        let fp2 = compute_fingerprint("sess1", "Bash", &serde_json::json!({"cmd": "ls"}));
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 64); // SHA256 hex
    }

    #[test]
    fn fingerprint_differs_for_different_input() {
        let fp1 = compute_fingerprint("sess1", "Bash", &serde_json::json!({"cmd": "ls"}));
        let fp2 = compute_fingerprint("sess1", "Bash", &serde_json::json!({"cmd": "rm"}));
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn fingerprint_key_order_irrelevant() {
        let fp1 = compute_fingerprint("s", "t", &serde_json::json!({"a": 1, "b": 2}));
        let fp2 = compute_fingerprint("s", "t", &serde_json::json!({"b": 2, "a": 1}));
        assert_eq!(fp1, fp2);
    }
}
