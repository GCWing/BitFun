//! MiniApp worker DTOs and pure command selection helpers.

use serde::{Deserialize, Serialize};

use crate::miniapp::runtime::RuntimeKind;

/// Result of npm/bun install.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallCommand {
    pub program: &'static str,
    pub args: &'static [&'static str],
}

pub fn install_command_for_runtime(kind: &RuntimeKind, pnpm_available: bool) -> InstallCommand {
    match kind {
        RuntimeKind::Bun => InstallCommand {
            program: "bun",
            args: &["install", "--production"],
        },
        RuntimeKind::Node if pnpm_available => InstallCommand {
            program: "pnpm",
            args: &["install", "--prod"],
        },
        RuntimeKind::Node => InstallCommand {
            program: "npm",
            args: &["install", "--production"],
        },
    }
}
