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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallDepsPlan {
    SkipMissingPackageJson,
    Run(InstallCommand),
}

const WORKER_POOL_CAPACITY: usize = 5;
const WORKER_IDLE_TIMEOUT_MS: i64 = 3 * 60 * 1000;

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

pub fn plan_install_deps(
    package_json_exists: bool,
    kind: &RuntimeKind,
    pnpm_available: bool,
) -> InstallDepsPlan {
    if !package_json_exists {
        return InstallDepsPlan::SkipMissingPackageJson;
    }
    InstallDepsPlan::Run(install_command_for_runtime(kind, pnpm_available))
}

pub fn worker_pool_capacity() -> usize {
    WORKER_POOL_CAPACITY
}

pub fn worker_idle_timeout_ms() -> i64 {
    WORKER_IDLE_TIMEOUT_MS
}

pub fn worker_pool_at_capacity(worker_count: usize) -> bool {
    worker_count >= WORKER_POOL_CAPACITY
}

pub fn worker_is_idle(now_ms: i64, last_activity_ms: i64) -> bool {
    now_ms - last_activity_ms > WORKER_IDLE_TIMEOUT_MS
}

pub fn select_lru_worker<I, K>(entries: I) -> Option<String>
where
    I: IntoIterator<Item = (K, i64)>,
    K: Into<String>,
{
    entries
        .into_iter()
        .min_by_key(|(_, last_activity_ms)| *last_activity_ms)
        .map(|(key, _)| key.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_pool_policy_keeps_existing_capacity_and_idle_timeout_contract() {
        assert_eq!(worker_pool_capacity(), 5);
        assert_eq!(worker_idle_timeout_ms(), 3 * 60 * 1000);
        assert!(!worker_pool_at_capacity(4));
        assert!(worker_pool_at_capacity(5));
        assert!(!worker_is_idle(10_000, 10_000 - worker_idle_timeout_ms()));
        assert!(worker_is_idle(
            10_000,
            10_000 - worker_idle_timeout_ms() - 1
        ));
    }

    #[test]
    fn select_lru_worker_returns_oldest_activity_key() {
        let oldest = select_lru_worker([("active", 3_000), ("oldest", 1_000), ("middle", 2_000)])
            .expect("one worker should be selected");

        assert_eq!(oldest, "oldest");
    }

    #[test]
    fn install_deps_plan_preserves_no_package_noop_and_runtime_commands() {
        assert_eq!(
            plan_install_deps(false, &RuntimeKind::Node, true),
            InstallDepsPlan::SkipMissingPackageJson
        );
        assert_eq!(
            plan_install_deps(true, &RuntimeKind::Node, true),
            InstallDepsPlan::Run(InstallCommand {
                program: "pnpm",
                args: &["install", "--prod"],
            })
        );
        assert_eq!(
            plan_install_deps(true, &RuntimeKind::Node, false),
            InstallDepsPlan::Run(InstallCommand {
                program: "npm",
                args: &["install", "--production"],
            })
        );
        assert_eq!(
            plan_install_deps(true, &RuntimeKind::Bun, false),
            InstallDepsPlan::Run(InstallCommand {
                program: "bun",
                args: &["install", "--production"],
            })
        );
    }
}
