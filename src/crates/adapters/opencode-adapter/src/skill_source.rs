use crate::command_source::strip_jsonc;
use crate::local_source_paths::{
    find_project_root, local_source_plan, LocalConfigDocument, LocalSourcePlanItem,
    OpenCodeLocalConfigOptions,
};
use openbitfun_product_domains::external_sources::ExternalSourceScope;
use openbitfun_services_core::bounded_fs::BoundedTextRead;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MAX_CONFIG_FILE_BYTES: usize = 1024 * 1024;
const MAX_CONFIGURED_SKILL_ROOTS: usize = 64;

#[derive(Debug, Clone)]
pub struct OpenCodeSkillRootProviderOptions {
    pub config: OpenCodeLocalConfigOptions,
    pub home_dir: Option<PathBuf>,
}

impl OpenCodeSkillRootProviderOptions {
    pub fn from_environment() -> Self {
        Self {
            config: OpenCodeLocalConfigOptions::from_environment(),
            home_dir: dirs::home_dir(),
        }
    }
}

impl Default for OpenCodeSkillRootProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCodeConfiguredSkillRoot {
    pub path: PathBuf,
    pub scope: ExternalSourceScope,
    pub precedence: usize,
}

pub struct OpenCodeSkillRootProvider {
    config: OpenCodeLocalConfigOptions,
    home_dir: Option<PathBuf>,
}

impl OpenCodeSkillRootProvider {
    pub fn new(options: OpenCodeSkillRootProviderOptions) -> Self {
        Self {
            config: options.config,
            home_dir: options.home_dir,
        }
    }

    pub fn discover(&self, workspace_root: Option<&Path>) -> Vec<OpenCodeConfiguredSkillRoot> {
        let canonical_workspace = workspace_root
            .map(find_project_root)
            .and_then(|path| dunce::canonicalize(path).ok());
        let canonical_home = self
            .home_dir
            .as_deref()
            .and_then(|path| dunce::canonicalize(path).ok());
        let mut configured_paths = Vec::new();
        let mut precedence = 0usize;

        for item in local_source_plan(&self.config, workspace_root, None) {
            let LocalSourcePlanItem::Config(document) = item else {
                continue;
            };
            let Some(paths) = read_local_skill_paths(&document) else {
                continue;
            };
            for value in paths {
                let current_precedence = precedence;
                precedence = precedence.saturating_add(1);
                configured_paths.push((value, document.scope, current_precedence));
            }
        }

        let mut contributions = Vec::new();
        for (value, source_scope, current_precedence) in configured_paths
            .into_iter()
            .rev()
            .take(MAX_CONFIGURED_SKILL_ROOTS)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let Some(path) =
                resolve_configured_path(&value, workspace_root, self.home_dir.as_deref())
            else {
                continue;
            };
            let Ok(path) = dunce::canonicalize(path) else {
                continue;
            };
            if !path.is_dir() {
                continue;
            }
            let workspace_scoped = canonical_workspace
                .as_ref()
                .is_some_and(|workspace| path.starts_with(workspace));
            let home_scoped = canonical_home
                .as_ref()
                .is_some_and(|home| path.starts_with(home));
            let scope = match source_scope {
                ExternalSourceScope::Project | ExternalSourceScope::WorkspaceLocal
                    if workspace_scoped =>
                {
                    ExternalSourceScope::Project
                }
                ExternalSourceScope::UserGlobal if workspace_scoped => ExternalSourceScope::Project,
                ExternalSourceScope::UserGlobal if home_scoped => ExternalSourceScope::UserGlobal,
                _ => continue,
            };
            contributions.push(OpenCodeConfiguredSkillRoot {
                path,
                scope,
                precedence: current_precedence,
            });
        }

        let mut seen = BTreeSet::new();
        contributions
            .into_iter()
            .filter(|root| seen.insert(root.path.clone()))
            .collect()
    }
}

impl Default for OpenCodeSkillRootProvider {
    fn default() -> Self {
        Self::new(OpenCodeSkillRootProviderOptions::default())
    }
}

fn read_local_skill_paths(document: &LocalConfigDocument) -> Option<Vec<String>> {
    let content = match document.read_bounded(MAX_CONFIG_FILE_BYTES).ok()? {
        BoundedTextRead::Content(content) => content,
        BoundedTextRead::TooLarge | BoundedTextRead::InvalidUtf8 => return None,
    };
    let document = serde_json::from_str::<Value>(&strip_jsonc(&content)).ok()?;
    match document.get("skills")? {
        Value::Object(skills) => strict_string_array(skills.get("paths"))
            .map(|paths| local_only_paths(paths.into_iter())),
        Value::Array(skills) => skills
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<_>>>()
            .map(|paths| local_only_paths(paths.into_iter())),
        _ => None,
    }
}

fn strict_string_array(value: Option<&Value>) -> Option<Vec<&str>> {
    let Some(value) = value else {
        return Some(Vec::new());
    };
    value.as_array()?.iter().map(Value::as_str).collect()
}

fn local_only_paths<'a>(paths: impl Iterator<Item = &'a str>) -> Vec<String> {
    paths
        .filter(|value| !is_remote_url(value))
        .map(str::to_string)
        .collect()
}

fn is_remote_url(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

fn resolve_configured_path(
    value: &str,
    workspace_root: Option<&Path>,
    home_dir: Option<&Path>,
) -> Option<PathBuf> {
    let value = value.trim();
    if value.is_empty() || is_remote_url(value) || value.contains('\0') {
        return None;
    }
    if let Some(relative) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        return home_dir.map(|home| home.join(relative));
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Some(path)
    } else {
        workspace_root.map(|workspace| workspace.join(path))
    }
}
