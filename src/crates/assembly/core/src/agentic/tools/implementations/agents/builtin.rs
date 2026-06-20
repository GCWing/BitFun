//! Built-in agent markdown files shipped with BitFun.
//!
//! These agents are embedded into the `bitfun-core` binary and installed into
//! the user agents directory on first launch. They are the default custom
//! sub-agents for the Synod tool (Councillor, Judge) and Team Mode (CIO,
//! Honest-Worker, Sales).

use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{BitFunError, BitFunResult};
use log::info;
use tokio::fs;

const BUILTIN_AGENTS: &[(&str, &str)] = &[
    (
        "councillor.md",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/builtin_agents/councillor.md")),
    ),
    (
        "judge.md",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/builtin_agents/judge.md")),
    ),
    (
        "cio.md",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/builtin_agents/cio.md")),
    ),
    (
        "honest-worker.md",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/builtin_agents/honest-worker.md")),
    ),
    (
        "sales.md",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/builtin_agents/sales.md")),
    ),
];

/// Ensure all built-in agents are installed in the user agents directory.
///
/// This is called once at startup. If the `.system` marker file already exists
/// the installation is skipped. Individual agent files are written without
/// overwriting existing files so the user's edits are preserved.
pub async fn ensure_builtin_agents_installed() -> BitFunResult<()> {
    let path_manager = get_path_manager_arc();
    let agents_dir = path_manager.user_agents_dir();

    // Create the agents directory if it doesn't exist.
    fs::create_dir_all(&agents_dir).await.map_err(|e| {
        BitFunError::io(format!("Failed to create agents directory: {}", e))
    })?;

    // Use a marker file to track whether we've already installed built-in agents.
    let marker_path = agents_dir.join(".system.installed");
    if marker_path.exists() {
        return Ok(());
    }

    info!("Installing built-in agents to {:?}", agents_dir);

    for (filename, content) in BUILTIN_AGENTS {
        let target_path = agents_dir.join(filename);

        // Only write if the file doesn't exist yet, to preserve user edits.
        if !target_path.exists() {
            fs::write(&target_path, content).await.map_err(|e| {
                BitFunError::io(format!(
                    "Failed to write built-in agent {}: {}",
                    filename, e
                ))
            })?;
            info!("  Installed built-in agent: {}", filename);
        }
    }

    // Write the marker file to avoid re-installing on every launch.
    fs::write(&marker_path, b"").await.map_err(|e| {
        BitFunError::io(format!(
            "Failed to write built-in agents marker: {}",
            e
        ))
    })?;

    Ok(())
}
