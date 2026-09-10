//! Redacted catalog construction shared by static ecosystem adapters.
use openbitfun_product_domains::external_hook_catalog::*;
use openbitfun_product_domains::external_sources::*;
use sha2::{Digest, Sha256};

pub struct StaticHookCatalog {
    snapshot: ExternalHookProviderSnapshot,
}

fn location_summary(location: &str) -> String {
    let text: String = location
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect();
    let mut end = text.len().min(512);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    if end == 0 {
        "<source>".into()
    } else {
        text[..end].to_string()
    }
}

impl StaticHookCatalog {
    pub fn new(provider: ExternalHookProviderIdentity) -> Self {
        Self {
            snapshot: ExternalHookProviderSnapshot {
                provider,
                sources: Vec::new(),
                entries: Vec::new(),
                diagnostics: Vec::new(),
            },
        }
    }

    pub fn source(
        &mut self,
        identity: &str,
        location: &str,
        kind: ExternalHookSourceKind,
        scope: ExternalSourceScope,
    ) -> Result<usize, ExternalSourceProviderError> {
        let key = SourceKey::new(
            self.snapshot.provider.provider_id.as_str(),
            format!("sha256:{:x}", Sha256::digest(identity.as_bytes())),
        )
        .expect("hashed source identity");
        if let Some(index) = self
            .snapshot
            .sources
            .iter()
            .position(|source| source.key == key)
        {
            return Ok(index);
        }
        if self.snapshot.sources.len() >= 128 {
            return Err(ExternalSourceProviderError::new(
                "external_hook.source_limit",
                "Static Hook source limit reached",
                false,
            ));
        }
        let index = self.snapshot.sources.len();
        self.snapshot.sources.push(ExternalHookSource {
            key,
            ecosystem_id: self.snapshot.provider.ecosystem_id.clone(),
            display_name: location_summary(location),
            source_kind: kind,
            scope,
            location_hint: location_summary(location),
            health: ExternalSourceHealth::Available,
            content_version: "pending".into(),
            diagnostics: Vec::new(),
        });
        Ok(index)
    }

    pub fn warning(&mut self, index: usize, code: &str, message: &str) {
        let source = &mut self.snapshot.sources[index];
        source.health = ExternalSourceHealth::Partial;
        if source
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code)
        {
            return;
        }
        let mut diagnostic =
            ExternalSourceDiagnostic::warning(code, message, Some(source.key.clone()));
        diagnostic.asset_kind = ExternalSourceAssetKind::Hook;
        source.diagnostics.push(diagnostic);
    }

    pub fn event(
        &mut self,
        index: usize,
        event: &str,
        kind: ExternalHookHandlerKind,
        matcher: ExternalHookMatcherSummary,
        status: ExternalHookProjectionStatus,
        activation: ExternalHookNativeActivation,
    ) -> Result<(), ExternalSourceProviderError> {
        if self.snapshot.entries.len() >= 2048 {
            return Err(ExternalSourceProviderError::new(
                "external_hook.entry_limit",
                "Static Hook entry limit reached",
                false,
            ));
        }
        let source = &self.snapshot.sources[index];
        let ordinal = self
            .snapshot
            .entries
            .iter()
            .filter(|entry| entry.source == source.key)
            .count();
        self.snapshot.entries.push(ExternalHookCatalogEntry {
            stable_key: format!("{}:{ordinal}", source.key.stable_key()),
            source: source.key.clone(),
            native_event: event.into(),
            matcher,
            handler_kind: kind,
            projection_status: status,
            native_activation: activation,
            mapping: None,
            content_version: "pending".into(),
        });
        Ok(())
    }

    pub fn finish(mut self) -> Result<ExternalHookProviderSnapshot, ExternalSourceProviderError> {
        // Hash only public facts. Commands, source code and credentials never affect
        // the public version, avoiding a digest oracle for low-entropy secrets.
        for source in &mut self.snapshot.sources {
            let mut digest = Sha256::new();
            digest
                .update(serde_json::to_vec(&source.diagnostics).expect("diagnostic serialization"));
            for entry in self
                .snapshot
                .entries
                .iter_mut()
                .filter(|entry| entry.source == source.key)
            {
                entry.content_version = format!(
                    "sha256:{:x}",
                    Sha256::digest(serde_json::to_vec(&entry).expect("entry serialization"))
                );
                digest.update(entry.content_version.as_bytes());
            }
            source.content_version = format!("sha256:{:x}", digest.finalize());
        }
        self.snapshot.validate().map_err(|_| {
            ExternalSourceProviderError::new(
                "external_hook.catalog_invalid",
                "Static Hook catalog contains invalid metadata",
                false,
            )
        })?;
        Ok(self.snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_locations_obey_the_wire_byte_limit() {
        let mut catalog = StaticHookCatalog::new(
            ExternalHookProviderIdentity::new("fixture.hooks", "fixture", "Fixture").unwrap(),
        );
        catalog
            .source(
                "source",
                &"\u{6587}".repeat(300),
                ExternalHookSourceKind::PluginFile,
                ExternalSourceScope::Project,
            )
            .unwrap();
        let snapshot = catalog.finish().unwrap();
        assert!(snapshot.sources[0].location_hint.len() <= 512);
        assert!(snapshot.sources[0].location_hint.ends_with('\u{6587}'));
    }
}
