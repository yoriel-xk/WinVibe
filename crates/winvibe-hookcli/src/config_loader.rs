use std::path::{Path, PathBuf};
use winvibe_core::config::{ConfigValidationError, RawWinvibeConfig, WinvibeConfig};

/// 配置加载错误
#[derive(Debug, thiserror::Error)]
pub enum ConfigLoadError {
    #[error("io error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("toml decode error: {0}")]
    TomlDecode(#[from] toml::de::Error),
    #[error(transparent)]
    Validation(#[from] ConfigValidationError),
}

/// 查找配置文件路径：CLI --config > env WINVIBE_CONFIG > 默认路径
pub fn resolve_config_path(cli_path: Option<&Path>) -> PathBuf {
    if let Some(p) = cli_path {
        return p.to_path_buf();
    }
    if let Ok(p) = std::env::var("WINVIBE_CONFIG") {
        return PathBuf::from(p);
    }
    default_config_path()
}

/// 默认配置文件路径：%LOCALAPPDATA%/WinVibe/winvibe.toml
fn default_config_path() -> PathBuf {
    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(local_app_data)
        .join("WinVibe")
        .join("winvibe.toml")
}

/// 严格加载配置（auth_token 缺失视为错误）
pub fn load_config_strict(path: &Path) -> Result<WinvibeConfig, ConfigLoadError> {
    let bytes = std::fs::read_to_string(path).map_err(|e| ConfigLoadError::Io {
        path: path.into(),
        source: e,
    })?;
    let raw: RawWinvibeConfig = toml::from_str(&bytes)?;
    if raw.auth_token.is_none() {
        return Err(ConfigValidationError::MissingAuthToken.into());
    }
    Ok(raw.validate()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn load_valid_config() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r#"
bind = "127.0.0.1"
port = "59999"
auth_token = "{}"
approval_ttl_ms = 300000
"#,
            "a".repeat(32)
        )
        .unwrap();
        let config = load_config_strict(f.path()).unwrap();
        assert_eq!(config.port, 59999);
    }

    #[test]
    fn load_missing_file_returns_io_error() {
        let result = load_config_strict(std::path::Path::new("/nonexistent/winvibe.toml"));
        assert!(matches!(result, Err(ConfigLoadError::Io { .. })));
    }

    #[test]
    fn load_missing_auth_token_returns_error() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r#"
bind = "127.0.0.1"
port = "59999"
"#
        )
        .unwrap();
        let result = load_config_strict(f.path());
        assert!(matches!(result, Err(ConfigLoadError::Validation(_))));
    }
}
