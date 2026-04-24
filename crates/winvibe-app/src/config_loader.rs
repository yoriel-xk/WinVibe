// config_loader.rs — App 端配置加载器
// 负责路径解析、首启默认文件生成、auth_token 自动引导

use std::path::{Path, PathBuf};
use winvibe_core::config::{ConfigValidationError, RawWinvibeConfig, WinvibeConfig};

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

/// 解析配置文件路径：CLI 参数 > WINVIBE_CONFIG 环境变量 > %LOCALAPPDATA%\WinVibe\winvibe.toml
pub fn resolve_config_path_app(cli_override: Option<&Path>) -> PathBuf {
    if let Some(p) = cli_override {
        return p.to_path_buf();
    }
    if let Ok(env_path) = std::env::var("WINVIBE_CONFIG") {
        return PathBuf::from(env_path);
    }
    let local_app_data = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());
    PathBuf::from(local_app_data)
        .join("WinVibe")
        .join("winvibe.toml")
}

/// 加载配置文件，不存在则创建默认文件；auth_token 为空则自动生成
pub fn load_or_init_config_app(path: &Path) -> Result<WinvibeConfig, ConfigLoadError> {
    ensure_default_config_file(path)?;
    let bytes = std::fs::read_to_string(path).map_err(|e| ConfigLoadError::Io {
        path: path.into(),
        source: e,
    })?;
    let mut raw: RawWinvibeConfig = toml::from_str(&bytes)?;

    if raw.auth_token.as_deref().is_none_or(str::is_empty) {
        let token = generate_auth_token_hex();
        persist_auth_token(path, &token)?;
        raw.auth_token = Some(token);
    }

    Ok(raw.validate()?)
}

fn ensure_default_config_file(path: &Path) -> Result<(), ConfigLoadError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ConfigLoadError::Io {
            path: path.into(),
            source: e,
        })?;
    }
    let default_content = "# WinVibe 配置文件\nbind = \"127.0.0.1\"\nport = \"59999\"\nauth_token = \"\"\napproval_ttl_ms = 300000\nmax_cached = 64\n";
    std::fs::write(path, default_content).map_err(|e| ConfigLoadError::Io {
        path: path.into(),
        source: e,
    })?;
    Ok(())
}

fn generate_auth_token_hex() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn persist_auth_token(path: &Path, token: &str) -> Result<(), ConfigLoadError> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigLoadError::Io {
        path: path.into(),
        source: e,
    })?;

    let updated = if content.contains("auth_token") {
        let mut result = String::new();
        for line in content.lines() {
            if line.trim_start().starts_with("auth_token") {
                result.push_str(&format!("auth_token = \"{token}\""));
            } else {
                result.push_str(line);
            }
            result.push('\n');
        }
        result
    } else {
        format!(
            "{content}auth_token = \"{token}\"
"
        )
    };

    // 原子写回：先写临时文件再 rename
    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &updated).map_err(|e| ConfigLoadError::Io {
        path: path.into(),
        source: e,
    })?;
    std::fs::rename(&tmp_path, path).map_err(|e| ConfigLoadError::Io {
        path: path.into(),
        source: e,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_or_init_creates_default_file_when_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("winvibe.toml");
        assert!(!path.exists());

        let config = load_or_init_config_app(&path).unwrap();
        assert!(path.exists());
        assert!(config.auth_token.as_str().len() >= 32);
    }

    #[test]
    fn load_or_init_preserves_existing_token() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("winvibe.toml");
        std::fs::write(
            &path,
            "bind = \"127.0.0.1\"\nport = \"59999\"\nauth_token = \"abcdef0123456789abcdef0123456789\"\napproval_ttl_ms = 300000\nmax_cached = 64\n",
        )
        .unwrap();

        let config = load_or_init_config_app(&path).unwrap();
        assert_eq!(
            config.auth_token.as_str(),
            "abcdef0123456789abcdef0123456789"
        );
    }

    #[test]
    fn load_or_init_generates_token_for_empty_string() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("winvibe.toml");
        std::fs::write(
            &path,
            "bind = \"127.0.0.1\"\nport = \"59999\"\nauth_token = \"\"\napproval_ttl_ms = 300000\nmax_cached = 64\n",
        )
        .unwrap();

        let config = load_or_init_config_app(&path).unwrap();
        assert!(config.auth_token.as_str().len() >= 32);
    }

    #[test]
    fn resolve_config_path_cli_overrides_env() {
        let cli_path = std::path::PathBuf::from("C:\\custom\\config.toml");
        let resolved = resolve_config_path_app(Some(&cli_path));
        assert_eq!(resolved, cli_path);
    }

    #[test]
    fn resolve_config_path_env_fallback() {
        // 注意：此测试修改环境变量，建议 --test-threads=1 运行
        std::env::set_var("WINVIBE_CONFIG", "C:\\env\\winvibe.toml");
        let resolved = resolve_config_path_app(None);
        std::env::remove_var("WINVIBE_CONFIG");
        assert_eq!(resolved, std::path::PathBuf::from("C:\\env\\winvibe.toml"));
    }
}
