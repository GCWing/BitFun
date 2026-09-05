use crate::command_source::strip_jsonc;
use crate::local_source_paths::{
    local_source_plan, local_source_watch_roots, normalize_path_lexically, path_identity,
    LocalConfigDocument, LocalSourcePlanItem, OpenCodeLocalConfigOptions,
};
use openbitfun_product_domains::external_sources::{
    EcosystemId, ExternalSourceAssetKind, ExternalSourceContext, ExternalSourceDiagnostic,
    ExternalSourceDiagnosticSeverity, ExternalSourceHealth, ExternalSourceProviderError,
    ExternalSourceRecord, ExternalSourceScope, ExternalWatchRoot, SourceKey,
};
use openbitfun_product_domains::workspace_references::{
    ExternalWorkspaceReferenceDefinition, ExternalWorkspaceReferenceProviderIdentity,
    ExternalWorkspaceReferenceProviderSnapshot, ExternalWorkspaceReferenceSourceProvider,
};
use openbitfun_services_core::bounded_fs::BoundedTextRead;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const PROVIDER_ID: &str = "opencode.references";
const ECOSYSTEM_ID: &str = "opencode";
const MAX_CONFIG_FILE_BYTES: usize = 1024 * 1024;
const MAX_REFERENCES: usize = 1024;
const MAX_DIAGNOSTICS: usize = 256;

#[derive(Debug, Clone)]
pub struct OpenCodeWorkspaceReferenceProviderOptions {
    pub config: OpenCodeLocalConfigOptions,
    pub home_dir: Option<PathBuf>,
}

impl OpenCodeWorkspaceReferenceProviderOptions {
    pub fn from_environment() -> Self {
        let home_dir = dirs::home_dir();
        Self {
            config: OpenCodeLocalConfigOptions::from_environment(),
            home_dir,
        }
    }
}

impl Default for OpenCodeWorkspaceReferenceProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct OpenCodeWorkspaceReferenceProvider {
    options: OpenCodeWorkspaceReferenceProviderOptions,
}

struct EffectiveReference {
    precedence: usize,
    definition: ExternalWorkspaceReferenceDefinition,
}

impl OpenCodeWorkspaceReferenceProvider {
    pub fn new(options: OpenCodeWorkspaceReferenceProviderOptions) -> Self {
        Self { options }
    }
}

impl Default for OpenCodeWorkspaceReferenceProvider {
    fn default() -> Self {
        Self::new(OpenCodeWorkspaceReferenceProviderOptions::default())
    }
}

impl ExternalWorkspaceReferenceSourceProvider for OpenCodeWorkspaceReferenceProvider {
    fn identity(&self) -> ExternalWorkspaceReferenceProviderIdentity {
        ExternalWorkspaceReferenceProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "OpenCode")
            .expect("static OpenCode reference provider identity must be valid")
    }

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<ExternalWorkspaceReferenceProviderSnapshot, ExternalSourceProviderError> {
        if !self.options.config.user_config_dir.is_absolute() {
            return Err(ExternalSourceProviderError::new(
                "opencode.reference.global_config_invalid",
                "OpenCode global configuration root must be absolute",
                false,
            ));
        }
        if context
            .workspace_root
            .as_ref()
            .is_some_and(|workspace_root| !workspace_root.is_absolute())
        {
            return Err(ExternalSourceProviderError::new(
                "opencode.reference.workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }

        let mut sources = Vec::new();
        let mut diagnostics = Vec::new();
        let mut dropped_diagnostics = 0usize;
        let mut effective = BTreeMap::<String, EffectiveReference>::new();
        let mut precedence = 0usize;

        let layers = local_source_plan(
            &self.options.config,
            context.workspace_root.as_deref(),
            None,
        );
        for layer in layers {
            let LocalSourcePlanItem::Config(document) = layer else {
                continue;
            };
            let parsed = read_reference_document(&document);
            let (content, entries) = match parsed {
                Ok(Some(parsed)) => parsed,
                Ok(None) => continue,
                Err(ReferenceDocumentReadError::Diagnostic(diagnostic)) => {
                    push_bounded_diagnostic(&mut diagnostics, &mut dropped_diagnostics, diagnostic);
                    continue;
                }
                Err(ReferenceDocumentReadError::TransientIo) => {
                    return Err(ExternalSourceProviderError::new(
                        "opencode.reference.config_unreadable",
                        "OpenCode reference configuration could not be read",
                        true,
                    ));
                }
            };
            let source_key = source_key(&document);
            let mut source_diagnostics = Vec::new();
            let source_dropped_before = dropped_diagnostics;
            for (alias, entry) in entries {
                let current_precedence = precedence;
                precedence = precedence.saturating_add(1);
                if !valid_alias(&alias) {
                    push_source_diagnostic(
                        &mut diagnostics,
                        &mut source_diagnostics,
                        &mut dropped_diagnostics,
                        reference_diagnostic(
                            ExternalSourceDiagnosticSeverity::Warning,
                            "opencode.reference.alias_invalid",
                            format!(
                                "OpenCode reference alias '{}' is invalid",
                                diagnostic_alias(&alias)
                            ),
                            Some(source_key.clone()),
                        ),
                    );
                    continue;
                }
                match parse_entry(&entry) {
                    Ok(ParsedEntry::Local {
                        path,
                        description,
                        hidden,
                    }) => {
                        let Some(path) = resolve_local_path(
                            &path,
                            document.base_directory.as_deref(),
                            self.options.home_dir.as_deref(),
                        ) else {
                            effective.remove(&alias);
                            push_source_diagnostic(
                                &mut diagnostics,
                                &mut source_diagnostics,
                                &mut dropped_diagnostics,
                                reference_diagnostic(
                                    ExternalSourceDiagnosticSeverity::Warning,
                                    "opencode.reference.path_invalid",
                                    format!(
                                        "OpenCode reference '{alias}' has an invalid local path"
                                    ),
                                    Some(source_key.clone()),
                                ),
                            );
                            continue;
                        };
                        // OpenCode references are declarative paths. Resolve an
                        // existing target for stable identity, but retain a
                        // lexical absolute path when it has not been created
                        // yet. Catalog discovery never grants filesystem access.
                        let path = path_identity(&path);
                        let definition = ExternalWorkspaceReferenceDefinition {
                            source: source_key.clone(),
                            alias: alias.clone(),
                            content_version: reference_version(
                                &alias,
                                &path,
                                description.as_deref(),
                                hidden,
                            ),
                            path,
                            description,
                            hidden,
                        };
                        effective.insert(
                            alias,
                            EffectiveReference {
                                precedence: current_precedence,
                                definition,
                            },
                        );
                    }
                    Ok(ParsedEntry::Git) => {
                        effective.remove(&alias);
                        push_source_diagnostic(
                            &mut diagnostics,
                            &mut source_diagnostics,
                            &mut dropped_diagnostics,
                            reference_diagnostic(
                                ExternalSourceDiagnosticSeverity::Info,
                                "opencode.reference.git_unsupported",
                                format!(
                                "OpenCode Git reference '{alias}' is not available in this release"
                            ),
                                Some(source_key.clone()),
                            ),
                        );
                    }
                    Err(reason) => {
                        effective.remove(&alias);
                        push_source_diagnostic(
                            &mut diagnostics,
                            &mut source_diagnostics,
                            &mut dropped_diagnostics,
                            reference_diagnostic(
                                ExternalSourceDiagnosticSeverity::Warning,
                                "opencode.reference.entry_invalid",
                                format!("OpenCode reference '{alias}' is invalid: {reason}"),
                                Some(source_key.clone()),
                            ),
                        );
                    }
                }
            }

            sources.push(ExternalSourceRecord {
                key: source_key,
                ecosystem_id: EcosystemId::new(ECOSYSTEM_ID)
                    .expect("static OpenCode ecosystem id must be valid"),
                display_name: source_display_name(document.scope).to_string(),
                source_kind: "opencode_config".to_string(),
                scope: document.scope,
                location: document.location(),
                execution_domain_id: context.execution_domain_id.clone(),
                health: if source_diagnostics.is_empty()
                    && dropped_diagnostics == source_dropped_before
                {
                    ExternalSourceHealth::Available
                } else {
                    ExternalSourceHealth::Partial
                },
                content_version: content_version(&content),
                diagnostics: source_diagnostics,
            });
        }

        let dropped_reference_count = effective.len().saturating_sub(MAX_REFERENCES);
        let mut references = effective.into_values().collect::<Vec<_>>();
        references.sort_by(|left, right| {
            right
                .precedence
                .cmp(&left.precedence)
                .then(left.definition.alias.cmp(&right.definition.alias))
        });
        references.truncate(MAX_REFERENCES);
        references.sort_by(|left, right| left.definition.alias.cmp(&right.definition.alias));
        if dropped_reference_count > 0 {
            push_bounded_diagnostic(
                &mut diagnostics,
                &mut dropped_diagnostics,
                reference_diagnostic(
                ExternalSourceDiagnosticSeverity::Warning,
                "opencode.reference.limit",
                format!(
                    "OpenCode reference limit of {MAX_REFERENCES} retained the highest-priority declarations and omitted {dropped_reference_count} lower-priority aliases"
                ),
                None,
            ),
            );
        }
        finish_bounded_diagnostics(&mut diagnostics, dropped_diagnostics);

        let snapshot = ExternalWorkspaceReferenceProviderSnapshot {
            provider: self.identity(),
            sources,
            references: references
                .into_iter()
                .map(|reference| reference.definition)
                .collect(),
            diagnostics,
        };
        snapshot.validate().map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.reference.snapshot_invalid",
                error.to_string(),
                false,
            )
        })?;
        Ok(snapshot)
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        if !self.options.config.user_config_dir.is_absolute() {
            return Vec::new();
        }
        local_source_watch_roots(
            &self.options.config,
            context.workspace_root.as_deref(),
            None,
        )
    }
}

enum ParsedEntry {
    Local {
        path: String,
        description: Option<String>,
        hidden: bool,
    },
    Git,
}

enum ReferenceDocumentReadError {
    Diagnostic(ExternalSourceDiagnostic),
    TransientIo,
}

fn read_reference_document(
    document: &LocalConfigDocument,
) -> Result<Option<(String, Map<String, Value>)>, ReferenceDocumentReadError> {
    let content = match document.read_bounded(MAX_CONFIG_FILE_BYTES) {
        Ok(BoundedTextRead::Content(content)) => content,
        Ok(BoundedTextRead::TooLarge) => {
            return Err(ReferenceDocumentReadError::Diagnostic(
                reference_diagnostic(
                    ExternalSourceDiagnosticSeverity::Warning,
                    "opencode.reference.config_too_large",
                    "OpenCode reference configuration exceeds the size limit".to_string(),
                    None,
                ),
            ));
        }
        Ok(BoundedTextRead::InvalidUtf8) => {
            return Err(ReferenceDocumentReadError::Diagnostic(
                reference_diagnostic(
                    ExternalSourceDiagnosticSeverity::Warning,
                    "opencode.reference.config_invalid_utf8",
                    "OpenCode reference configuration is not UTF-8".to_string(),
                    None,
                ),
            ));
        }
        Err(_) => return Err(ReferenceDocumentReadError::TransientIo),
    };
    let document = serde_json::from_str::<Value>(&strip_jsonc(&content)).map_err(|_| {
        ReferenceDocumentReadError::Diagnostic(reference_diagnostic(
            ExternalSourceDiagnosticSeverity::Warning,
            "opencode.reference.config_invalid",
            "OpenCode reference configuration is invalid".to_string(),
            None,
        ))
    })?;
    let Some(entries) = document
        .get("references")
        .or_else(|| document.get("reference"))
    else {
        return Ok(None);
    };
    let entries = entries.as_object().cloned().ok_or_else(|| {
        ReferenceDocumentReadError::Diagnostic(reference_diagnostic(
            ExternalSourceDiagnosticSeverity::Warning,
            "opencode.reference.config_invalid",
            "OpenCode references must be an object".to_string(),
            None,
        ))
    })?;
    Ok(Some((content, entries)))
}

fn parse_entry(value: &Value) -> Result<ParsedEntry, &'static str> {
    match value {
        Value::String(value) => {
            if value.starts_with('.') || value.starts_with('/') || value.starts_with('~') {
                Ok(ParsedEntry::Local {
                    path: value.clone(),
                    description: None,
                    hidden: false,
                })
            } else {
                Ok(ParsedEntry::Git)
            }
        }
        Value::Object(entry) if entry.contains_key("path") && !entry.contains_key("repository") => {
            let path = entry
                .get("path")
                .and_then(Value::as_str)
                .ok_or("path must be a string")?
                .to_string();
            let description = optional_string(entry.get("description"), "description")?;
            let hidden = optional_bool(entry.get("hidden"), "hidden")?.unwrap_or(false);
            Ok(ParsedEntry::Local {
                path,
                description: description.filter(|description| !description.is_empty()),
                hidden,
            })
        }
        Value::Object(entry) if entry.contains_key("repository") && !entry.contains_key("path") => {
            if entry.get("repository").and_then(Value::as_str).is_none() {
                return Err("repository must be a string");
            }
            optional_string(entry.get("branch"), "branch")?;
            optional_string(entry.get("description"), "description")?;
            optional_bool(entry.get("hidden"), "hidden")?;
            Ok(ParsedEntry::Git)
        }
        Value::Object(_) => Err("entry must contain exactly one of path or repository"),
        _ => Err("entry must be a string or object"),
    }
}

fn optional_string(
    value: Option<&Value>,
    _field: &'static str,
) -> Result<Option<String>, &'static str> {
    match value {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_string()))
            .ok_or("optional text field must be a string"),
    }
}

fn optional_bool(
    value: Option<&Value>,
    _field: &'static str,
) -> Result<Option<bool>, &'static str> {
    match value {
        None => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or("optional boolean field must be a boolean"),
    }
}

fn resolve_local_path(
    value: &str,
    config_directory: Option<&Path>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    if value.is_empty() || value.contains('\0') {
        return None;
    }
    if let Some(relative) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        return home
            .filter(|home| home.is_absolute())
            .map(|home| normalize_path_lexically(&home.join(relative)));
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Some(normalize_path_lexically(&path))
    } else {
        config_directory.map(|directory| normalize_path_lexically(&directory.join(path)))
    }
}

fn valid_alias(alias: &str) -> bool {
    !alias.is_empty()
        && alias.len() <= 160
        && !alias.chars().any(|character| {
            character == '/' || character.is_whitespace() || matches!(character, '`' | ',')
        })
}

fn source_key(document: &LocalConfigDocument) -> SourceKey {
    let digest = content_version(&document.identity());
    SourceKey::new(PROVIDER_ID, format!("opencode-config-{}", &digest[..24]))
        .expect("hashed OpenCode reference source id must be valid")
}

fn source_display_name(scope: ExternalSourceScope) -> &'static str {
    match scope {
        ExternalSourceScope::UserGlobal => "OpenCode user references",
        ExternalSourceScope::Project => "OpenCode project references",
        ExternalSourceScope::WorkspaceLocal => "OpenCode workspace references",
        ExternalSourceScope::RemoteUser | ExternalSourceScope::RemoteProject => {
            "OpenCode remote references"
        }
        _ => "OpenCode references",
    }
}

fn content_version(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

fn reference_version(alias: &str, path: &Path, description: Option<&str>, hidden: bool) -> String {
    let mut hasher = Sha256::new();
    for part in [
        alias,
        path.to_string_lossy().as_ref(),
        description.unwrap_or(""),
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hasher.update([u8::from(hidden)]);
    hex::encode(hasher.finalize())
}

fn reference_diagnostic(
    severity: ExternalSourceDiagnosticSeverity,
    code: impl Into<String>,
    message: String,
    source: Option<SourceKey>,
) -> ExternalSourceDiagnostic {
    ExternalSourceDiagnostic {
        severity,
        asset_kind: ExternalSourceAssetKind::Reference,
        code: code.into(),
        message,
        source,
    }
}

fn push_source_diagnostic(
    diagnostics: &mut Vec<ExternalSourceDiagnostic>,
    source_diagnostics: &mut Vec<ExternalSourceDiagnostic>,
    dropped: &mut usize,
    diagnostic: ExternalSourceDiagnostic,
) {
    if push_bounded_diagnostic(diagnostics, dropped, diagnostic.clone()) {
        source_diagnostics.push(diagnostic);
    }
}

fn push_bounded_diagnostic(
    diagnostics: &mut Vec<ExternalSourceDiagnostic>,
    dropped: &mut usize,
    diagnostic: ExternalSourceDiagnostic,
) -> bool {
    if diagnostics.len() < MAX_DIAGNOSTICS.saturating_sub(1) {
        diagnostics.push(diagnostic);
        true
    } else {
        *dropped = dropped.saturating_add(1);
        false
    }
}

fn finish_bounded_diagnostics(diagnostics: &mut Vec<ExternalSourceDiagnostic>, dropped: usize) {
    if dropped == 0 {
        return;
    }
    diagnostics.push(reference_diagnostic(
        ExternalSourceDiagnosticSeverity::Warning,
        "opencode.reference.diagnostic_limit",
        format!(
            "OpenCode reference diagnostics were limited to {MAX_DIAGNOSTICS}; {dropped} additional diagnostics were omitted"
        ),
        None,
    ));
}

fn diagnostic_alias(alias: &str) -> String {
    const MAX_ALIAS_CHARS: usize = 160;
    let mut characters = alias.chars();
    let prefix = characters
        .by_ref()
        .take(MAX_ALIAS_CHARS)
        .collect::<String>();
    if characters.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::{read_reference_document, ReferenceDocumentReadError};
    use crate::local_source_paths::{
        LocalConfigDocument, LocalConfigDocumentKind, LocalConfigDocumentSource,
    };
    use openbitfun_product_domains::external_sources::ExternalSourceScope;

    #[test]
    fn unreadable_configuration_is_classified_as_transient() {
        let directory = tempfile::TempDir::new().unwrap();
        let document = LocalConfigDocument {
            source: LocalConfigDocumentSource::File(directory.path().to_path_buf()),
            base_directory: directory.path().parent().map(std::path::Path::to_path_buf),
            kind: LocalConfigDocumentKind::User,
            scope: ExternalSourceScope::UserGlobal,
        };

        assert!(matches!(
            read_reference_document(&document),
            Err(ReferenceDocumentReadError::TransientIo)
        ));
    }
}
