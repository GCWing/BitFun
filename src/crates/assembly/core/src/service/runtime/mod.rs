//! Managed runtime service compatibility adapter.
//!
//! Command resolution and PATH merge rules are owned by
//! `bitfun-services-core`; core only supplies the product-managed runtime root.

use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::BitFunResult;
use bitfun_services_core::managed_runtime::ManagedRuntimeResolver;
use std::path::{Path, PathBuf};

pub use bitfun_services_core::managed_runtime::{
    ResolvedCommand, RuntimeCommandCapability, RuntimeSource,
};

#[derive(Debug, Clone)]
pub struct RuntimeManager {
    inner: ManagedRuntimeResolver,
}

impl RuntimeManager {
    pub fn new() -> BitFunResult<Self> {
        let pm = get_path_manager_arc();
        Ok(Self {
            inner: ManagedRuntimeResolver::new(pm.managed_runtimes_dir()),
        })
    }

    #[cfg(test)]
    fn with_runtime_root(runtime_root: PathBuf) -> Self {
        Self {
            inner: ManagedRuntimeResolver::new(runtime_root),
        }
    }

    pub fn runtime_root(&self) -> &Path {
        self.inner.runtime_root()
    }

    pub fn runtime_root_display(&self) -> String {
        self.inner.runtime_root_display()
    }

    pub fn resolve_command(&self, command: &str) -> Option<ResolvedCommand> {
        self.inner.resolve_command(command)
    }

    pub fn get_capabilities(&self) -> Vec<RuntimeCommandCapability> {
        self.inner.get_capabilities()
    }

    pub fn get_command_capability(&self, command: &str) -> RuntimeCommandCapability {
        self.inner.get_command_capability(command)
    }

    pub fn get_capabilities_for_commands(
        &self,
        commands: impl IntoIterator<Item = String>,
    ) -> Vec<RuntimeCommandCapability> {
        self.inner.get_capabilities_for_commands(commands)
    }

    pub fn managed_path_entries(&self) -> Vec<PathBuf> {
        self.inner.managed_path_entries()
    }

    pub fn merged_path_env(&self, existing_path: Option<&str>) -> Option<String> {
        self.inner.merged_path_env(existing_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_runtime_root() -> PathBuf {
        let mut p = std::env::temp_dir();
        let id = format!(
            "bitfun-core-runtime-adapter-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        p.push(id);
        p
    }

    #[test]
    fn adapter_uses_managed_runtime_owner() {
        let root = temp_runtime_root();
        let node_bin = root.join("node").join("current").join("bin");
        fs::create_dir_all(&node_bin).unwrap();

        let manager = RuntimeManager::with_runtime_root(root.clone());

        assert_eq!(manager.runtime_root(), root.as_path());
        assert!(manager
            .managed_path_entries()
            .iter()
            .any(|p| p == &node_bin));

        let _ = fs::remove_dir_all(root);
    }
}
