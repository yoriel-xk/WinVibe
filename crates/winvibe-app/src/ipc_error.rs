use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct IpcError {
    pub code: String,
    pub message: String,
}

impl IpcError {
    pub fn internal(message: &str) -> Self {
        Self {
            code: "ipc_internal".to_string(),
            message: message.to_string(),
        }
    }

    pub fn from_code(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_error_serializes_to_flat_json() {
        let err = IpcError {
            code: "approval_not_found".to_string(),
            message: "no such approval".to_string(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "approval_not_found");
        assert_eq!(json["message"], "no such approval");
        assert_eq!(json.as_object().unwrap().len(), 2);
    }

    #[test]
    fn ipc_error_from_string() {
        let err = IpcError::internal("something broke");
        assert_eq!(err.code, "ipc_internal");
    }
}
