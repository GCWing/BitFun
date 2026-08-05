use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

const IDENTITY_SCHEMA_VERSION: u8 = 1;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackIdentity<'a> {
    schema_version: u8,
    anonymous_id: &'a str,
}

pub(super) struct FeedbackIdentityStore {
    path: PathBuf,
}

impl FeedbackIdentityStore {
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub(super) async fn store(&self, anonymous_id: &str) -> Result<()> {
        uuid::Uuid::parse_str(anonymous_id).context("validate feedback anonymous identity")?;
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("create feedback identity directory")?;
        }
        let value = serde_json::to_vec_pretty(&FeedbackIdentity {
            schema_version: IDENTITY_SCHEMA_VERSION,
            anonymous_id,
        })
        .context("encode feedback identity")?;
        let temporary_path = self.path.with_extension("json.tmp");
        tokio::fs::write(&temporary_path, value)
            .await
            .context("write feedback identity")?;
        set_owner_only(&temporary_path);
        tokio::fs::rename(&temporary_path, &self.path)
            .await
            .context("replace feedback identity")?;
        set_owner_only(&self.path);
        Ok(())
    }
}

#[cfg(unix)]
fn set_owner_only(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
        log::warn!("Failed to restrict feedback identity file permissions: {error}");
    }
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::FeedbackIdentityStore;

    #[tokio::test]
    async fn stores_only_the_plain_anonymous_identity() {
        let directory = std::env::temp_dir().join(format!(
            "bitfun-feedback-identity-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let path = directory.join("identity.json");
        let store = FeedbackIdentityStore::new(path.clone());

        store
            .store("11111111-1111-4111-8111-111111111111")
            .await
            .unwrap();

        let value: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(&path).await.unwrap()).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["anonymousId"], "11111111-1111-4111-8111-111111111111");
        assert_eq!(value.as_object().unwrap().len(), 2);
        let _ = tokio::fs::remove_dir_all(directory).await;
    }
}
