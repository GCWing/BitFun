//! Debug log owner for file persistence and remote dispatch.

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tokio::task;
use uuid::Uuid;

const DEFAULT_SESSION_ID: &str = "debug-session";

static DEFAULT_LOG_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Ok(env_path) = std::env::var("BITFUN_DEBUG_LOG_PATH") {
        return PathBuf::from(env_path);
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".bitfun")
        .join("debug.log")
});

static DEFAULT_INGEST_URL: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var("BITFUN_DEBUG_INGEST_URL").ok());

#[derive(Debug, Clone)]
pub struct DebugLogConfig {
    pub log_path: PathBuf,
    pub ingest_url: Option<String>,
    pub session_id: String,
}

impl Default for DebugLogConfig {
    fn default() -> Self {
        Self {
            log_path: DEFAULT_LOG_PATH.clone(),
            ingest_url: DEFAULT_INGEST_URL.clone(),
            session_id: DEFAULT_SESSION_ID.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugLogEntry {
    pub location: String,
    pub message: String,
    #[serde(default)]
    pub data: Value,
    #[serde(default)]
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hypothesis_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl DebugLogEntry {
    pub fn with_defaults(mut self, config: &DebugLogConfig) -> Self {
        if self.session_id.is_empty() {
            self.session_id = config.session_id.clone();
        }
        if self.timestamp.is_none() {
            self.timestamp = Some(current_timestamp_ms());
        }
        if self.id.is_none() {
            self.id = Some(format!("log_{}", Uuid::new_v4()));
        }
        self
    }
}

pub async fn append_log_async(
    entry: DebugLogEntry,
    config: Option<DebugLogConfig>,
    send_http: bool,
) -> Result<()> {
    let cfg = config.unwrap_or_default();
    let log_line = build_log_line(entry, &cfg);
    let log_path = cfg.log_path.clone();
    let ingest_url = cfg.ingest_url.clone().filter(|_| send_http);

    let log_line_for_file = log_line.clone();
    task::spawn_blocking(move || -> Result<()> {
        ensure_parent_exists(&log_path)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        writeln!(file, "{}", serde_json::to_string(&log_line_for_file)?)?;
        Ok(())
    })
    .await
    .map_err(|error| anyhow::anyhow!("Join error: {}", error))??;

    if let Some(url) = ingest_url {
        let _ = post_debug_log(&url, &log_line).await;
    }

    Ok(())
}

pub async fn post_debug_log(url: &str, log_line: &Value) -> Result<(), String> {
    reqwest::Client::new()
        .post(url)
        .json(log_line)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn current_timestamp_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn build_log_line(entry: DebugLogEntry, config: &DebugLogConfig) -> Value {
    let normalized = entry.with_defaults(config);
    let data = redact_value(normalized.data);

    serde_json::json!({
        "id": normalized.id,
        "timestamp": normalized.timestamp,
        "location": normalized.location,
        "message": normalized.message,
        "data": data,
        "sessionId": normalized.session_id,
        "runId": normalized.run_id,
        "hypothesisId": normalized.hypothesis_id,
    })
}

fn ensure_parent_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn redact_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sanitized = serde_json::Map::new();
            for (key, child) in map.into_iter() {
                if is_sensitive_key(&key) {
                    sanitized.insert(key, redact_scalar(child));
                } else {
                    sanitized.insert(key, redact_value(child));
                }
            }
            Value::Object(sanitized)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(redact_value).collect()),
        other => other,
    }
}

fn redact_scalar(value: Value) -> Value {
    match value {
        Value::String(value) => {
            let prefix: String = value.chars().take(10).collect();
            Value::String(format!("{}***", prefix))
        }
        Value::Number(_) => Value::String("***".to_string()),
        Value::Bool(_) => Value::Bool(false),
        Value::Array(_) | Value::Object(_) => Value::String("***".to_string()),
        Value::Null => Value::Null,
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "password"
            | "token"
            | "access_token"
            | "refresh_token"
            | "api_key"
            | "apikey"
            | "cookie"
            | "authorization"
            | "auth"
            | "secret"
    )
}
