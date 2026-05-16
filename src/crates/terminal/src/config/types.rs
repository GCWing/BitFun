//! Configuration type definitions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main terminal configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Default shell type to use
    pub default_shell: Option<String>,

    /// Default working directory
    pub default_cwd: Option<String>,

    /// Environment variables to set for all terminals
    pub env: HashMap<String, String>,

    /// Scrollback buffer size (lines)
    pub scrollback: u32,

    /// Enable flow control
    pub flow_control: FlowControlConfig,

    /// Data buffering configuration
    pub buffering: BufferingConfig,

    /// Session persistence configuration
    pub persistence: PersistenceConfig,

    /// Shell integration settings
    pub shell_integration: ShellIntegrationConfig,

    /// Terminal dimensions
    pub default_cols: u16,
    pub default_rows: u16,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            default_shell: None,
            default_cwd: None,
            env: HashMap::new(),
            scrollback: 10000,
            flow_control: FlowControlConfig::default(),
            buffering: BufferingConfig::default(),
            persistence: PersistenceConfig::default(),
            shell_integration: ShellIntegrationConfig::default(),
            default_cols: 80,
            default_rows: 24,
        }
    }
}

/// Flow control configuration (prevents data overflow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowControlConfig {
    /// Enable flow control
    pub enabled: bool,

    /// High water mark - pause PTY when unacknowledged chars exceed this
    pub high_water_mark: usize,

    /// Low water mark - resume PTY when unacknowledged chars fall below this
    pub low_water_mark: usize,
}

impl Default for FlowControlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            high_water_mark: 100_000, // ~100KB
            low_water_mark: 5_000,    // ~5KB
        }
    }
}

/// Data buffering configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferingConfig {
    /// Enable data buffering
    pub enabled: bool,

    /// Buffer flush interval in milliseconds
    pub flush_interval_ms: u64,

    /// Maximum buffer size before forced flush
    pub max_buffer_size: usize,
}

impl Default for BufferingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            flush_interval_ms: 5,       // 5ms
            max_buffer_size: 64 * 1024, // 64KB
        }
    }
}

/// Session persistence configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Enable session persistence
    pub enabled: bool,

    /// Grace time before orphaned session is killed (seconds)
    pub grace_time_secs: u64,

    /// Short grace time for reduced reconnection window
    pub short_grace_time_secs: u64,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            grace_time_secs: 60,      // 1 minute
            short_grace_time_secs: 6, // 6 seconds
        }
    }
}

/// Shell integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellIntegrationConfig {
    /// Enable shell integration
    pub enabled: bool,

    /// Enable command detection
    pub command_detection: bool,

    /// Enable CWD detection
    pub cwd_detection: bool,

    /// Shell integration nonce for security
    pub nonce: Option<String>,

    /// Directory for shell integration scripts.
    /// If None, uses default location: {cache_dir}/bitfun_terminal/scripts
    #[serde(default)]
    pub scripts_dir: Option<PathBuf>,
}

impl Default for ShellIntegrationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            command_detection: true,
            cwd_detection: true,
            nonce: None,
            scripts_dir: None,
        }
    }
}

/// Shell-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    /// Path to shell executable
    pub executable: String,

    /// Arguments to pass to shell
    pub args: Vec<String>,

    /// Environment variables specific to this shell
    pub env: HashMap<String, String>,

    /// Working directory
    pub cwd: Option<String>,

    /// Login shell flag
    pub login: bool,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            executable: default_shell_executable(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: None,
            login: false,
        }
    }
}

/// Get the default shell executable for the current platform
fn default_shell_executable() -> String {
    #[cfg(windows)]
    {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}
