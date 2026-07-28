//! Authentication: TAIJI_API_KEY validation, Device Code login, tier resolution.
//!
//! Key format:
//!   tjr_free_<random>    → Tier::Free
//!   tjr_std_<random>     → Tier::Standard
//!   tjr_ult_<random>     → Tier::Ultimate

use std::path::PathBuf;

use tracing::info;

use crate::config::{load_product_config, ResolvedConfig, Tier};

/// Authentication result containing the resolved tier and config.
#[derive(Debug)]
pub(crate) struct AuthResult {
    pub(crate) tier: Tier,
    #[allow(dead_code)]
    pub(crate) config: ResolvedConfig,
    pub(crate) key_prefix: String,
}

/// Errors that can occur during authentication.
#[derive(Debug)]
pub(crate) enum AuthError {
    InvalidKey(String),
    ConfigError(String),
    IoError(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidKey(msg) => write!(f, "Invalid API key: {}", msg),
            Self::ConfigError(msg) => write!(f, "Config error: {}", msg),
            Self::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl std::error::Error for AuthError {}

/// Resolve tier from TAIJI_API_KEY environment variable.
///
/// Returns `(Tier, ResolvedConfig)` for the resolved tier.
/// If no key is set, returns `Tier::Free` with free config.
pub(crate) fn resolve_tier() -> Result<(Tier, ResolvedConfig), AuthError> {
    match std::env::var("TAIJI_API_KEY") {
        Ok(key) => {
            let key = key.trim().to_string();
            let tier = parse_key_tier(&key)?;
            info!("TAIJI_API_KEY detected: tier={:?}, prefix={}", tier, &key[..7.min(key.len())]);
            let config = load_product_config(tier)
                .map_err(|e| AuthError::ConfigError(e))?;
            Ok((tier, config))
        }
        Err(std::env::VarError::NotPresent) => {
            info!("TAIJI_API_KEY not set, defaulting to Free tier");
            let config = load_product_config(Tier::Free)
                .map_err(|e| AuthError::ConfigError(e))?;
            Ok((Tier::Free, config))
        }
        Err(e) => Err(AuthError::InvalidKey(format!("Env error: {}", e))),
    }
}

/// Parse the tier from an API key prefix.
///
/// Key format: `tjr_{tier}_{random}` where tier is one of `free`, `std`, `ult`.
fn parse_key_tier(key: &str) -> Result<Tier, AuthError> {
    let parts: Vec<&str> = key.splitn(3, '_').collect();
    if parts.len() < 3 || parts[0] != "tjr" {
        return Err(AuthError::InvalidKey(
            "Key must start with 'tjr_{tier}_'".to_string(),
        ));
    }
    match parts[1] {
        "free" => Ok(Tier::Free),
        "std" => Ok(Tier::Standard),
        "ult" => Ok(Tier::Ultimate),
        other => Err(AuthError::InvalidKey(
            format!("Unknown tier '{}' in key (expected free/std/ult)", other),
        )),
    }
}

/// Get the data directory for taiji config/token storage.
fn taiji_data_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".taiji");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Store authentication result to `~/.taiji/config.json`.
pub(crate) fn store_auth(result: &AuthResult) -> Result<(), AuthError> {
    let dir = taiji_data_dir();
    let path = dir.join("config.json");
    let json = serde_json::json!({
        "tier": format!("{:?}", result.tier).to_lowercase(),
        "key_prefix": result.key_prefix,
        "updated_at": chrono::Utc::now().to_rfc3339(),
    });
    let content = serde_json::to_string_pretty(&json)
        .map_err(|e| AuthError::IoError(format!("Serialization error: {}", e)))?;
    std::fs::write(&path, &content)
        .map_err(|e| AuthError::IoError(format!("Failed to write {}: {}", path.display(), e)))?;
    info!("Auth config saved to {}", path.display());
    Ok(())
}

/// Read stored auth status from `~/.taiji/config.json`.
pub(crate) fn read_auth_status() -> Result<Option<serde_json::Value>, AuthError> {
    let path = taiji_data_dir().join("config.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| AuthError::IoError(format!("Failed to read {}: {}", path.display(), e)))?;
    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AuthError::IoError(format!("Failed to parse config: {}", e)))?;
    Ok(Some(json))
}

/// Clear stored auth config.
pub(crate) fn clear_auth() -> Result<(), AuthError> {
    let path = taiji_data_dir().join("config.json");
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| AuthError::IoError(format!("Failed to remove {}: {}", path.display(), e)))?;
        info!("Auth config cleared");
    }
    Ok(())
}

// ── Device Code Login ──

/// Device Code login flow (mock implementation).
///
/// For now, mock: prompt user to enter a key.
pub(crate) fn device_code_login() -> Result<AuthResult, AuthError> {
    println!("╔══════════════════════════════════════════╗");
    println!("║        Taiji Quant 设备授权登录          ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║ 1. 请访问 https://taiji-quant.dev/activate ║");
    println!("║ 2. 输入以下设备码完成授权：              ║");
    println!("║                                          ║");
    println!("║     {:^36}  ║", generate_device_code());
    println!("║                                          ║");
    println!("║ 3. 登录后，输入您的 API Key 完成配置     ║");
    println!("╚══════════════════════════════════════════╝");
    println!();
    print!("请输入 API Key (留空跳过): ");
    use std::io::Write;
    std::io::stdout().flush().ok();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let key = input.trim();

    if key.is_empty() {
        println!("未输入 Key，使用免费版。");
        let config = load_product_config(Tier::Free)
            .map_err(|e| AuthError::ConfigError(e))?;
        let result = AuthResult {
            tier: Tier::Free,
            config,
            key_prefix: "none".to_string(),
        };
        store_auth(&result)?;
        return Ok(result);
    }

    let tier = parse_key_tier(key)?;
    let config = load_product_config(tier)
        .map_err(|e| AuthError::ConfigError(e))?;
    let prefix = key[..7.min(key.len())].to_string();
    let result = AuthResult {
        tier,
        config,
        key_prefix: prefix,
    };
    store_auth(&result)?;
    println!("✅ 登录成功！当前版本：{:?}", tier);
    Ok(result)
}

/// Generate a mock device code.
fn generate_device_code() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let short = (seed % 999999) as u32;
    format!("TAIJI-{:06}", short)
}

/// Handle auth CLI subcommands.
///
/// Called from main.rs `Command::Auth { command }`.
pub(crate) fn handle_auth_command(command: super::AuthCommand) -> Result<(), String> {
    match command {
        super::AuthCommand::Login => {
            device_code_login().map(|_| ())
                .map_err(|e| e.to_string())
        }
        super::AuthCommand::Logout => {
            clear_auth().map_err(|e| e.to_string())?;
            println!("✅ 已登出，本地 token 已清除。");
            Ok(())
        }
        super::AuthCommand::Status => {
            match read_auth_status() {
                Ok(Some(json)) => {
                    let tier_str = json["tier"].as_str().unwrap_or("?");
                    println!("╔════════════════════════════════╗");
                    println!("║     Taiji Quant 认证状态       ║");
                    println!("╠════════════════════════════════╣");
                    println!("║ Tier:    {:20} ║", tier_str);
                    println!("║ Key:     {:20} ║", json["key_prefix"].as_str().unwrap_or("?"));
                    if let Some(updated) = json["updated_at"].as_str() {
                        println!("║ 更新:    {:20} ║", updated);
                    }
                    println!("╚════════════════════════════════╝");
                    Ok(())
                }
                Ok(None) => {
                    // No stored auth — check env var
                    match resolve_tier() {
                        Ok((tier, _config)) => {
                            let tier_str = format!("{:?}", tier).to_lowercase();
                            println!("╔════════════════════════════════╗");
                            println!("║     Taiji Quant 认证状态       ║");
                            println!("╠════════════════════════════════╣");
                            println!("║ Tier:    {:20} ║", tier_str);
                            println!("║ Source:  {:20} ║", "TAIJI_API_KEY env");
                            println!("╚════════════════════════════════╝");
                            Ok(())
                        }
                        Err(e) => {
                            println!("未登录。使用 TAIJI_API_KEY 环境变量或运行 `taiji auth login` 登录。");
                            Err(e.to_string())
                        }
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_tier_free() {
        assert_eq!(parse_key_tier("tjr_free_abc123").unwrap(), Tier::Free);
    }

    #[test]
    fn test_parse_key_tier_standard() {
        assert_eq!(parse_key_tier("tjr_std_xyz789").unwrap(), Tier::Standard);
    }

    #[test]
    fn test_parse_key_tier_ultimate() {
        assert_eq!(parse_key_tier("tjr_ult_def456").unwrap(), Tier::Ultimate);
    }

    #[test]
    fn test_parse_key_invalid_prefix() {
        assert!(parse_key_tier("invalid_key").is_err());
    }

    #[test]
    fn test_parse_key_unknown_tier() {
        assert!(parse_key_tier("tjr_unknown_xxx").is_err());
    }

    #[test]
    fn test_parse_key_too_short() {
        assert!(parse_key_tier("tjr_free").is_err());
    }
}
