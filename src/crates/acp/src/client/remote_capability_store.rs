use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use bitfun_core::util::errors::{BitFunError, BitFunResult};
use log::warn;
use tokio::sync::RwLock;

use super::config::RemoteAcpClientRequirementSnapshot;

#[derive(Clone)]
pub(crate) struct RemoteAcpCapabilityStore {
    path: PathBuf,
    snapshots: Arc<RwLock<HashMap<String, RemoteAcpClientRequirementSnapshot>>>,
}

impl RemoteAcpCapabilityStore {
    pub(crate) fn new(path: PathBuf) -> Self {
        let snapshots = match std::fs::read_to_string(&path) {
            Ok(content) => {
                match serde_json::from_str::<Vec<RemoteAcpClientRequirementSnapshot>>(&content) {
                    Ok(entries) => entries
                        .into_iter()
                        .map(|entry| (entry.connection_id.clone(), entry))
                        .collect(),
                    Err(error) => {
                        warn!(
                            "Failed to parse remote ACP capability snapshots: path={} error={}",
                            path.display(),
                            error
                        );
                        HashMap::new()
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(error) => {
                warn!(
                    "Failed to read remote ACP capability snapshots: path={} error={}",
                    path.display(),
                    error
                );
                HashMap::new()
            }
        };

        Self {
            path,
            snapshots: Arc::new(RwLock::new(snapshots)),
        }
    }

    pub(crate) async fn get(
        &self,
        connection_id: &str,
    ) -> Option<RemoteAcpClientRequirementSnapshot> {
        self.snapshots.read().await.get(connection_id).cloned()
    }

    pub(crate) async fn set(
        &self,
        snapshot: RemoteAcpClientRequirementSnapshot,
    ) -> BitFunResult<()> {
        let entries = {
            let mut guard = self.snapshots.write().await;
            guard.insert(snapshot.connection_id.clone(), snapshot);
            guard.values().cloned().collect::<Vec<_>>()
        };
        self.persist(entries).await
    }

    async fn persist(
        &self,
        snapshots: Vec<RemoteAcpClientRequirementSnapshot>,
    ) -> BitFunResult<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                BitFunError::io(format!(
                    "Failed to create remote ACP capability snapshot directory: {}",
                    error
                ))
            })?;
        }

        let content = serde_json::to_string_pretty(&snapshots).map_err(|error| {
            BitFunError::serialization(format!(
                "Failed to serialize remote ACP capability snapshots: {}",
                error
            ))
        })?;
        tokio::fs::write(&self.path, content)
            .await
            .map_err(|error| {
                BitFunError::io(format!(
                    "Failed to write remote ACP capability snapshots: {}",
                    error
                ))
            })?;
        Ok(())
    }
}
