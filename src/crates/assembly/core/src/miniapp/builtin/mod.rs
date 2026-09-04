//! Built-in MiniApps — bundled, seeded into miniapps_dir on first launch / upgrade.
//!
//! Each built-in app has a fixed id (so it can be located across runs). On startup
//! we compare `.builtin-manifest.json` with the bundled asset hash and only rewrite
//! source files when newer code is available.
//! The user's `storage.json` is preserved across upgrades.

use crate::miniapp::manager::MiniAppManager;
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use chrono::Utc;
use openbitfun_product_domains::miniapp::builtin::{
    seed_builtin_miniapps_with_host, BuiltinInstallMarker, BuiltinMiniAppSeedBundleRequest,
    BuiltinMiniAppSeedHost, BuiltinMiniAppSeedOutcome, BuiltinMiniAppSeedReport,
    BuiltinSeedArtifacts, BUILTIN_INSTALL_MARKER,
};
pub use openbitfun_product_domains::miniapp::builtin::{
    BuiltinMiniAppBundle as BuiltinApp, BUILTIN_APPS,
};
use openbitfun_product_domains::miniapp::ports::{
    MiniAppPortError, MiniAppPortErrorKind, MiniAppPortFuture,
};
use openbitfun_services_integrations::miniapp::builtin_io as miniapp_builtin_io;
use std::path::Path;
use std::sync::Arc;

/// Seed all built-in MiniApps into the user data directory. Idempotent: skips apps
/// whose on-disk marker hash matches the bundled content. User's `storage.json`
/// is preserved across reseeds; source files & meta.json (without timestamps) are
/// overwritten.
pub async fn seed_builtin_miniapps(manager: &Arc<MiniAppManager>) -> OpenBitFunResult<()> {
    let host = CoreBuiltinMiniAppSeedHost {
        manager: Arc::clone(manager),
    };
    for report in seed_builtin_miniapps_with_host(&host).await {
        log_builtin_seed_report(report);
    }
    Ok(())
}

struct CoreBuiltinMiniAppSeedHost {
    manager: Arc<MiniAppManager>,
}

impl BuiltinMiniAppSeedHost for CoreBuiltinMiniAppSeedHost {
    fn now_ms(&self) -> i64 {
        Utc::now().timestamp_millis()
    }

    fn installed_marker(
        &self,
        app_id: &'static str,
    ) -> MiniAppPortFuture<'_, Option<BuiltinInstallMarker>> {
        Box::pin(async move {
            let marker_path = self
                .manager
                .path_manager()
                .miniapp_dir(app_id)
                .join(BUILTIN_INSTALL_MARKER);
            read_builtin_install_marker(&marker_path)
                .await
                .map_err(map_openbitfun_error_to_miniapp_port_error)
        })
    }

    fn has_local_override(&self, app_id: &'static str) -> MiniAppPortFuture<'_, bool> {
        Box::pin(async move {
            match self.manager.load_customization_metadata(app_id).await {
                Ok(Some(metadata)) => Ok(metadata.local_override),
                Ok(None) => Ok(false),
                Err(e) => {
                    log::warn!(
                        "read customization metadata for builtin miniapp '{}' failed: {}",
                        app_id,
                        e
                    );
                    Ok(false)
                }
            }
        })
    }

    fn record_available_update(
        &self,
        app_id: &'static str,
        version: u32,
        content_hash: String,
        now_ms: i64,
    ) -> MiniAppPortFuture<'_, bool> {
        Box::pin(async move {
            self.manager
                .mark_builtin_update_available(app_id, version, &content_hash, now_ms)
                .await
                .map_err(map_openbitfun_error_to_miniapp_port_error)
        })
    }

    fn seed_bundle(&self, request: BuiltinMiniAppSeedBundleRequest) -> MiniAppPortFuture<'_, ()> {
        Box::pin(async move {
            prepare_builtin_seed_bundle(&self.manager, request)
                .await
                .map_err(map_openbitfun_error_to_miniapp_port_error)
        })
    }

    fn write_seed_markers(
        &self,
        app_id: &'static str,
        artifacts: BuiltinSeedArtifacts,
    ) -> MiniAppPortFuture<'_, ()> {
        Box::pin(async move {
            let app_dir = self.manager.path_manager().miniapp_dir(app_id);
            write_builtin_install_marker(&app_dir.join(BUILTIN_INSTALL_MARKER), &artifacts.marker)
                .await
                .map_err(map_openbitfun_error_to_miniapp_port_error)
        })
    }
}

async fn prepare_builtin_seed_bundle(
    manager: &Arc<MiniAppManager>,
    request: BuiltinMiniAppSeedBundleRequest,
) -> OpenBitFunResult<()> {
    let app_dir = manager.path_manager().miniapp_dir(request.app.id);
    miniapp_builtin_io::prepare_builtin_seed_bundle_files(
        &app_dir,
        request.app,
        request.seeded_at_ms,
    )
    .await
    .map_err(map_builtin_io_error)?;

    // Recompile to assemble the final compiled.html with bridge + theme + import map.
    manager.recompile(request.app.id, "dark", None).await?;
    Ok(())
}

fn log_builtin_seed_report(report: BuiltinMiniAppSeedReport) {
    match report.outcome {
        Ok(BuiltinMiniAppSeedOutcome::Skipped) => {}
        Ok(BuiltinMiniAppSeedOutcome::Seeded {
            version,
            content_hash,
        }) => {
            log::info!(
                "seeded builtin miniapp '{}' (v{}, {})",
                report.app_id,
                version,
                content_hash
            );
        }
        Ok(BuiltinMiniAppSeedOutcome::PreservedLocalOverride {
            version,
            recorded_update,
            ..
        }) => {
            if recorded_update {
                log::info!(
                    "preserved customized builtin miniapp '{}' and recorded bundled update v{}",
                    report.app_id,
                    version
                );
            } else {
                log::info!(
                    "preserved customized builtin miniapp '{}' and skipped previously declined bundled update v{}",
                    report.app_id,
                    version
                );
            }
        }
        Err(error) => {
            log::warn!("seed builtin miniapp '{}' failed: {}", report.app_id, error);
        }
    }
}

async fn read_builtin_install_marker(
    path: &Path,
) -> OpenBitFunResult<Option<BuiltinInstallMarker>> {
    miniapp_builtin_io::read_builtin_install_marker(path)
        .await
        .map_err(map_builtin_io_error)
}

async fn write_builtin_install_marker(
    path: &Path,
    marker: &BuiltinInstallMarker,
) -> OpenBitFunResult<()> {
    miniapp_builtin_io::write_builtin_install_marker(path, marker)
        .await
        .map_err(map_builtin_io_error)
}

fn map_builtin_io_error(err: miniapp_builtin_io::MiniAppBuiltinIoError) -> OpenBitFunError {
    match err {
        err @ miniapp_builtin_io::MiniAppBuiltinIoError::Io { .. } => {
            OpenBitFunError::io(err.to_string())
        }
        miniapp_builtin_io::MiniAppBuiltinIoError::InvalidBundledMeta(source) => {
            OpenBitFunError::parse(format!("invalid bundled meta.json: {}", source))
        }
        miniapp_builtin_io::MiniAppBuiltinIoError::MarkerSerialization(source)
        | miniapp_builtin_io::MiniAppBuiltinIoError::MetaSerialization(source)
        | miniapp_builtin_io::MiniAppBuiltinIoError::PackageSerialization(source) => {
            OpenBitFunError::from(source)
        }
    }
}

fn map_openbitfun_error_to_miniapp_port_error(error: OpenBitFunError) -> MiniAppPortError {
    let kind = match &error {
        OpenBitFunError::NotFound(_) => MiniAppPortErrorKind::NotFound,
        OpenBitFunError::Validation(_) => MiniAppPortErrorKind::InvalidInput,
        OpenBitFunError::Deserialization(_) | OpenBitFunError::Serialization(_) => {
            MiniAppPortErrorKind::Deserialization
        }
        OpenBitFunError::Io(io_error)
            if io_error.kind() == std::io::ErrorKind::PermissionDenied =>
        {
            MiniAppPortErrorKind::PermissionDenied
        }
        OpenBitFunError::Io(_) => MiniAppPortErrorKind::Io,
        OpenBitFunError::ProcessError(_) | OpenBitFunError::Timeout(_) => {
            MiniAppPortErrorKind::RuntimeUnavailable
        }
        _ => MiniAppPortErrorKind::Backend,
    };
    MiniAppPortError::new(kind, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbitfun_product_domains::miniapp::builtin::{
        builtin_content_hash, should_seed_builtin_app,
    };
    use openbitfun_product_domains::miniapp::customization::{
        MiniAppCustomizationMetadata, MiniAppCustomizationOrigin, MiniAppCustomizationOriginKind,
    };

    struct TestMiniAppManager {
        manager: Arc<MiniAppManager>,
        root: std::path::PathBuf,
    }

    impl std::ops::Deref for TestMiniAppManager {
        type Target = Arc<MiniAppManager>;

        fn deref(&self) -> &Self::Target {
            &self.manager
        }
    }

    impl Drop for TestMiniAppManager {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn test_manager() -> TestMiniAppManager {
        let root = std::env::temp_dir().join(format!(
            "openbitfun-miniapp-builtin-customization-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root.clone()));
        TestMiniAppManager {
            manager: Arc::new(MiniAppManager::new(path_manager)),
            root,
        }
    }

    async fn write_outdated_builtin_marker(app_dir: &std::path::Path) {
        write_builtin_install_marker(
            &app_dir.join(BUILTIN_INSTALL_MARKER),
            &BuiltinInstallMarker {
                version: 0,
                hash: "sha256:outdated".to_string(),
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn builtin_reseed_preserves_local_override_and_records_available_update() {
        let manager = test_manager();
        let builtin = &BUILTIN_APPS[0];
        seed_builtin_miniapps(&manager).await.unwrap();

        let custom_css = "body { background: #f7f7f7; }";
        let app_dir = manager.path_manager().miniapp_dir(builtin.id);
        tokio::fs::write(app_dir.join("source").join("style.css"), custom_css)
            .await
            .unwrap();
        manager
            .save_customization_metadata(
                builtin.id,
                &MiniAppCustomizationMetadata {
                    origin: MiniAppCustomizationOrigin {
                        kind: MiniAppCustomizationOriginKind::Builtin,
                        builtin_id: Some(builtin.id.to_string()),
                        builtin_version: Some(builtin.version),
                        market: None,
                    },
                    local_override: true,
                    last_applied_draft_id: Some("draft-1".to_string()),
                    available_builtin_update: None,
                    declined_builtin_updates: Vec::new(),
                    updated_at: Utc::now().timestamp_millis(),
                },
            )
            .await
            .unwrap();
        write_outdated_builtin_marker(&app_dir).await;

        seed_builtin_miniapps(&manager).await.unwrap();

        let css = tokio::fs::read_to_string(app_dir.join("source").join("style.css"))
            .await
            .unwrap();
        assert_eq!(css, custom_css);

        let metadata = manager
            .load_customization_metadata(builtin.id)
            .await
            .unwrap()
            .unwrap();
        assert!(metadata.local_override);
        let update = metadata.available_builtin_update.unwrap();
        assert_eq!(update.builtin_version, builtin.version);
        assert!(!update.source_hash.is_empty());
    }

    #[tokio::test]
    async fn builtin_reseed_skips_declined_update_until_local_override_changes() {
        let manager = test_manager();
        let builtin = &BUILTIN_APPS[0];
        seed_builtin_miniapps(&manager).await.unwrap();

        let custom_css = "body { background: #fafafa; }";
        let app_dir = manager.path_manager().miniapp_dir(builtin.id);
        tokio::fs::write(app_dir.join("source").join("style.css"), custom_css)
            .await
            .unwrap();
        manager
            .save_customization_metadata(
                builtin.id,
                &MiniAppCustomizationMetadata {
                    origin: MiniAppCustomizationOrigin {
                        kind: MiniAppCustomizationOriginKind::Builtin,
                        builtin_id: Some(builtin.id.to_string()),
                        builtin_version: Some(builtin.version),
                        market: None,
                    },
                    local_override: true,
                    last_applied_draft_id: Some("draft-1".to_string()),
                    available_builtin_update: None,
                    declined_builtin_updates: Vec::new(),
                    updated_at: Utc::now().timestamp_millis(),
                },
            )
            .await
            .unwrap();

        write_outdated_builtin_marker(&app_dir).await;
        seed_builtin_miniapps(&manager).await.unwrap();
        let first_metadata = manager
            .load_customization_metadata(builtin.id)
            .await
            .unwrap()
            .unwrap();
        let first_update = first_metadata.available_builtin_update.unwrap();
        let source_hash = first_update.source_hash.clone();

        manager
            .decline_builtin_update(builtin.id, first_update.builtin_version, &source_hash, 1234)
            .await
            .unwrap();
        write_outdated_builtin_marker(&app_dir).await;
        seed_builtin_miniapps(&manager).await.unwrap();

        let declined_metadata = manager
            .load_customization_metadata(builtin.id)
            .await
            .unwrap()
            .unwrap();
        assert!(declined_metadata.available_builtin_update.is_none());
        assert_eq!(declined_metadata.declined_builtin_updates.len(), 1);
        assert_eq!(
            declined_metadata.declined_builtin_updates[0].source_hash,
            source_hash
        );
        let repeated_same_source = manager
            .mark_builtin_update_available(builtin.id, builtin.version + 1, &source_hash, 5678)
            .await
            .unwrap();
        assert!(!repeated_same_source);
        let css = tokio::fs::read_to_string(app_dir.join("source").join("style.css"))
            .await
            .unwrap();
        assert_eq!(css, custom_css);

        tokio::fs::write(
            app_dir.join("source").join("style.css"),
            "body { background: #ffffff; }",
        )
        .await
        .unwrap();
        manager
            .sync_from_fs(builtin.id, "dark", None)
            .await
            .unwrap();
        write_outdated_builtin_marker(&app_dir).await;
        seed_builtin_miniapps(&manager).await.unwrap();

        let updated_metadata = manager
            .load_customization_metadata(builtin.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            updated_metadata
                .available_builtin_update
                .as_ref()
                .map(|update| (update.builtin_version, update.source_hash.as_str())),
            Some((builtin.version, source_hash.as_str()))
        );
    }

    #[test]
    fn builtin_app_content_hash_changes_when_assets_change() {
        let app = &BUILTIN_APPS[0];

        let changed = super::BuiltinApp {
            ui_js: "changed ui",
            ..*app
        };

        assert_ne!(builtin_content_hash(app), builtin_content_hash(&changed));
    }

    #[test]
    fn builtin_seed_decision_uses_content_hash_before_version_marker() {
        let app = &BUILTIN_APPS[0];
        let current_marker = BuiltinInstallMarker {
            version: app.version,
            hash: builtin_content_hash(app),
        };
        let content_hash = builtin_content_hash(app);
        let stale_hash_marker = BuiltinInstallMarker {
            version: app.version,
            hash: "sha256:stale".to_string(),
        };
        let older_version_marker = BuiltinInstallMarker {
            version: app.version.saturating_sub(1),
            hash: content_hash.clone(),
        };

        assert!(!should_seed_builtin_app(
            app,
            &content_hash,
            Some(&current_marker)
        ));
        assert!(should_seed_builtin_app(
            app,
            &content_hash,
            Some(&stale_hash_marker)
        ));
        assert!(should_seed_builtin_app(
            app,
            &content_hash,
            Some(&older_version_marker)
        ));
        assert!(should_seed_builtin_app(app, &content_hash, None));
    }
}
