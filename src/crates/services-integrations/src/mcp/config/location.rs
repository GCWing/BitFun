use serde::{Deserialize, Serialize};

/// Configuration location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigLocation {
    BuiltIn, // Built-in configuration
    User,    // User-level configuration
    Project, // Project-level configuration
}
