use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWatchEvent {
    pub path: String,
    pub kind: FileWatchEventKind,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileWatchEventKind {
    Create,
    Modify,
    Remove,
    Rename { from: String, to: String },
    Other,
}

#[derive(Debug, Clone)]
pub struct FileWatcherConfig {
    pub watch_recursively: bool,
    pub ignore_hidden_files: bool,
    /// Skip common dependency/build output directories for broad workspace watches.
    /// Semantic source roots such as Skill catalogs can disable this because names
    /// like `build` and `target` are valid source identifiers there.
    pub ignore_common_build_directories: bool,
    pub debounce_interval_ms: u64,
    pub max_events_per_interval: usize,
}

impl Default for FileWatcherConfig {
    fn default() -> Self {
        Self {
            watch_recursively: true,
            ignore_hidden_files: true,
            ignore_common_build_directories: true,
            debounce_interval_ms: 500,
            max_events_per_interval: 100,
        }
    }
}
