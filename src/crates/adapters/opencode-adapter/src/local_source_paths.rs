use openbitfun_product_domains::external_sources::{ExternalSourceScope, ExternalWatchRoot};
use openbitfun_services_core::bounded_fs::{read_bounded_text, BoundedTextRead};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct OpenCodeLocalConfigOptions {
    pub user_config_dir: PathBuf,
    pub legacy_user_config_dir: Option<PathBuf>,
    pub explicit_config_file: Option<PathBuf>,
    pub explicit_config_dir: Option<PathBuf>,
    pub inline_config_content: Option<String>,
    pub project_config_enabled: bool,
}

impl std::fmt::Debug for OpenCodeLocalConfigOptions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenCodeLocalConfigOptions")
            .field("user_config_dir", &self.user_config_dir)
            .field("legacy_user_config_dir", &self.legacy_user_config_dir)
            .field("explicit_config_file", &self.explicit_config_file)
            .field("explicit_config_dir", &self.explicit_config_dir)
            .field(
                "inline_config_content",
                &self.inline_config_content.as_ref().map(|_| "<redacted>"),
            )
            .field("project_config_enabled", &self.project_config_enabled)
            .finish()
    }
}

impl OpenCodeLocalConfigOptions {
    pub fn from_environment() -> Self {
        let home = dirs::home_dir();
        Self {
            user_config_dir: user_config_dir(
                std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
                home.clone(),
            ),
            legacy_user_config_dir: home.map(|home| home.join(".opencode")),
            explicit_config_file: std::env::var_os("OPENCODE_CONFIG").map(PathBuf::from),
            explicit_config_dir: std::env::var_os("OPENCODE_CONFIG_DIR").map(PathBuf::from),
            inline_config_content: std::env::var("OPENCODE_CONFIG_CONTENT")
                .ok()
                .filter(|content| !content.is_empty()),
            project_config_enabled: !environment_truthy("OPENCODE_DISABLE_PROJECT_CONFIG"),
        }
    }
}

impl Default for OpenCodeLocalConfigOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub(crate) const INLINE_CONFIG_LOCATION: &str = "OPENCODE_CONFIG_CONTENT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalConfigDocumentKind {
    User,
    ExplicitFile,
    Project,
    Directory(LocalConfigDirectoryKind),
    Inline,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum LocalConfigDocumentSource {
    File(PathBuf),
    Inline(String),
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct LocalConfigDocument {
    pub(crate) source: LocalConfigDocumentSource,
    pub(crate) base_directory: Option<PathBuf>,
    pub(crate) kind: LocalConfigDocumentKind,
    pub(crate) scope: ExternalSourceScope,
}

impl std::fmt::Debug for LocalConfigDocument {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let source = match &self.source {
            LocalConfigDocumentSource::File(path) => path.to_string_lossy().into_owned(),
            LocalConfigDocumentSource::Inline(_) => INLINE_CONFIG_LOCATION.to_string(),
        };
        formatter
            .debug_struct("LocalConfigDocument")
            .field("source", &source)
            .field("base_directory", &self.base_directory)
            .field("kind", &self.kind)
            .field("scope", &self.scope)
            .finish()
    }
}

impl LocalConfigDocument {
    pub(crate) fn file_path(&self) -> Option<&Path> {
        match &self.source {
            LocalConfigDocumentSource::File(path) => Some(path),
            LocalConfigDocumentSource::Inline(_) => None,
        }
    }

    pub(crate) fn location(&self) -> String {
        match &self.source {
            LocalConfigDocumentSource::File(path) => path.to_string_lossy().into_owned(),
            LocalConfigDocumentSource::Inline(_) => INLINE_CONFIG_LOCATION.to_string(),
        }
    }

    pub(crate) fn identity(&self) -> String {
        match &self.source {
            LocalConfigDocumentSource::File(path) => {
                path_identity(path).to_string_lossy().into_owned()
            }
            LocalConfigDocumentSource::Inline(_) => INLINE_CONFIG_LOCATION.to_string(),
        }
    }

    pub(crate) fn read_bounded(&self, max_bytes: usize) -> std::io::Result<BoundedTextRead> {
        match &self.source {
            LocalConfigDocumentSource::File(path) => read_bounded_text(path, max_bytes),
            LocalConfigDocumentSource::Inline(content) if content.len() > max_bytes => {
                Ok(BoundedTextRead::TooLarge)
            }
            LocalConfigDocumentSource::Inline(content) => {
                Ok(BoundedTextRead::Content(content.clone()))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LocalSourcePlanItem {
    Config(LocalConfigDocument),
    Directory(LocalConfigDirectory),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalConfigDirectoryKind {
    User,
    Project,
    Legacy,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalConfigDirectory {
    pub(crate) path: PathBuf,
    pub(crate) kind: LocalConfigDirectoryKind,
    pub(crate) scope: ExternalSourceScope,
}

/// Builds the local, runtime-free slice of OpenCode's source plan from lowest
/// to highest precedence. Remote, managed, and executable plugin sources are
/// deliberately outside this adapter contract.
pub(crate) fn local_source_plan(
    options: &OpenCodeLocalConfigOptions,
    workspace_root: Option<&Path>,
    project_root_override: Option<&Path>,
) -> Vec<LocalSourcePlanItem> {
    let mut items = Vec::new();
    let project_root = workspace_root.map(|workspace| {
        project_root_override
            .map(Path::to_path_buf)
            .unwrap_or_else(|| find_project_root(workspace))
    });
    push_document_file(
        &mut items,
        &options.user_config_dir.join("config.json"),
        ExternalSourceScope::UserGlobal,
        LocalConfigDocumentKind::User,
    );
    push_document_files(
        &mut items,
        &options.user_config_dir,
        ExternalSourceScope::UserGlobal,
        LocalConfigDocumentKind::User,
    );

    if let Some(path) = options.explicit_config_file.as_deref() {
        if let Some(path) = resolve_workspace_path(path, workspace_root) {
            let scope = explicit_source_scope(&path, workspace_root, project_root.as_deref());
            push_document_file(
                &mut items,
                &path,
                scope,
                LocalConfigDocumentKind::ExplicitFile,
            );
        }
    }

    if options.project_config_enabled {
        if let (Some(workspace), Some(project_root)) = (workspace_root, project_root.as_deref()) {
            for directory in project_config_directories(project_root, workspace) {
                push_document_files(
                    &mut items,
                    &directory,
                    ExternalSourceScope::Project,
                    LocalConfigDocumentKind::Project,
                );
            }
        }
    }

    let project_directories = if options.project_config_enabled {
        match (workspace_root, project_root.as_deref()) {
            (Some(workspace), Some(project_root)) => {
                project_asset_directories(project_root, workspace)
            }
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let explicit_config_dir = options
        .explicit_config_dir
        .as_deref()
        .and_then(|path| resolve_workspace_path(path, workspace_root));
    let explicit_config_scope = explicit_config_dir
        .as_deref()
        .map_or(ExternalSourceScope::UserGlobal, |path| {
            explicit_source_scope(path, workspace_root, project_root.as_deref())
        });
    for directory in ordered_local_config_directories(
        &options.user_config_dir,
        options.legacy_user_config_dir.as_deref(),
        explicit_config_dir.as_deref(),
        explicit_config_scope,
        &project_directories,
    ) {
        if directory.kind != LocalConfigDirectoryKind::User {
            push_document_files(
                &mut items,
                &directory.path,
                directory.scope,
                LocalConfigDocumentKind::Directory(directory.kind),
            );
        }
        items.push(LocalSourcePlanItem::Directory(directory));
    }

    if let Some(content) = options
        .inline_config_content
        .as_ref()
        .filter(|content| !content.is_empty())
    {
        items.push(LocalSourcePlanItem::Config(LocalConfigDocument {
            source: LocalConfigDocumentSource::Inline(content.clone()),
            base_directory: workspace_root.map(opened_directory).map(Path::to_path_buf),
            kind: LocalConfigDocumentKind::Inline,
            scope: ExternalSourceScope::WorkspaceLocal,
        }));
    }

    deduplicate_plan_keep_last(items)
}

pub(crate) fn local_source_watch_roots(
    options: &OpenCodeLocalConfigOptions,
    workspace_root: Option<&Path>,
    project_root_override: Option<&Path>,
) -> Vec<ExternalWatchRoot> {
    let project_directories = if options.project_config_enabled {
        workspace_root
            .map(|workspace| {
                let project_root = project_root_override
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| find_project_root(workspace));
                project_config_directories(&project_root, workspace)
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let explicit_file = options
        .explicit_config_file
        .as_deref()
        .and_then(|path| resolve_workspace_path(path, workspace_root));
    let explicit_dir = options
        .explicit_config_dir
        .as_deref()
        .and_then(|path| resolve_workspace_path(path, workspace_root));
    local_watch_roots(
        &options.user_config_dir,
        options.legacy_user_config_dir.as_deref(),
        explicit_file.as_deref(),
        explicit_dir.as_deref(),
        &project_directories,
    )
}

fn push_document_files(
    items: &mut Vec<LocalSourcePlanItem>,
    directory: &Path,
    scope: ExternalSourceScope,
    kind: LocalConfigDocumentKind,
) {
    for name in ["opencode.json", "opencode.jsonc"] {
        push_document_file(items, &directory.join(name), scope, kind);
    }
}

fn push_document_file(
    items: &mut Vec<LocalSourcePlanItem>,
    path: &Path,
    scope: ExternalSourceScope,
    kind: LocalConfigDocumentKind,
) {
    let should_inspect = match std::fs::metadata(path) {
        Ok(metadata) => metadata.is_file(),
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    };
    if should_inspect {
        items.push(LocalSourcePlanItem::Config(LocalConfigDocument {
            source: LocalConfigDocumentSource::File(path.to_path_buf()),
            base_directory: path.parent().map(Path::to_path_buf),
            kind,
            scope,
        }));
    }
}

fn resolve_workspace_path(path: &Path, workspace_root: Option<&Path>) -> Option<PathBuf> {
    if path.is_absolute() {
        Some(path.to_path_buf())
    } else {
        workspace_root.map(|workspace| opened_directory(workspace).join(path))
    }
}

fn explicit_source_scope(
    path: &Path,
    workspace_root: Option<&Path>,
    project_root: Option<&Path>,
) -> ExternalSourceScope {
    let identity = path_identity(path);
    let is_local = workspace_root
        .map(opened_directory)
        .into_iter()
        .chain(project_root)
        .map(path_identity)
        .any(|root| identity.starts_with(root));
    if is_local {
        ExternalSourceScope::WorkspaceLocal
    } else {
        ExternalSourceScope::UserGlobal
    }
}

fn deduplicate_plan_keep_last(items: Vec<LocalSourcePlanItem>) -> Vec<LocalSourcePlanItem> {
    let mut seen = std::collections::BTreeSet::new();
    let mut unique = items
        .into_iter()
        .rev()
        .filter(|item| match item {
            LocalSourcePlanItem::Config(LocalConfigDocument {
                source: LocalConfigDocumentSource::File(path),
                ..
            }) => seen.insert(("config", path_identity(path))),
            LocalSourcePlanItem::Config(LocalConfigDocument {
                source: LocalConfigDocumentSource::Inline(_),
                ..
            }) => seen.insert(("inline", PathBuf::from(INLINE_CONFIG_LOCATION))),
            LocalSourcePlanItem::Directory(directory) => {
                seen.insert(("directory", path_identity(&directory.path)))
            }
        })
        .collect::<Vec<_>>();
    unique.reverse();
    unique
}

pub(crate) fn user_config_dir(xdg_config_home: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
    xdg_config_home
        .or_else(|| home.map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("opencode")
}

pub(crate) fn find_project_root(start: &Path) -> PathBuf {
    let start = opened_directory(start);
    start
        .ancestors()
        .find(|path| path.join(".git").exists())
        .unwrap_or(start)
        .to_path_buf()
}

/// OpenCode loads project config files from the project boundary toward the
/// opened directory so that the closest config file is applied last.
pub(crate) fn project_config_directories(root: &Path, opened: &Path) -> Vec<PathBuf> {
    let opened = opened_directory(opened);
    let mut directories = opened
        .ancestors()
        .take_while(|path| path.starts_with(root))
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    directories.reverse();
    directories
}

/// OpenCode's ConfigPaths directory phase keeps the upward traversal order:
/// the opened directory is applied first and the outer project directory last.
pub(crate) fn project_asset_directories(root: &Path, opened: &Path) -> Vec<PathBuf> {
    let mut directories = project_config_directories(root, opened);
    directories.reverse();
    directories
}

/// Reproduces OpenCode's `ConfigPaths.directories` order for the local slice.
/// Physical aliases keep their first position, while an explicit alias keeps
/// the `OPENCODE_CONFIG_DIR` scope and loading semantics at that position.
pub(crate) fn ordered_local_config_directories(
    user_config_dir: &Path,
    legacy_user_config_dir: Option<&Path>,
    explicit_config_dir: Option<&Path>,
    explicit_config_scope: ExternalSourceScope,
    project_directories: &[PathBuf],
) -> Vec<LocalConfigDirectory> {
    let mut directories = Vec::new();
    let mut indices = BTreeMap::new();
    push_config_directory(
        &mut directories,
        &mut indices,
        user_config_dir,
        LocalConfigDirectoryKind::User,
        ExternalSourceScope::UserGlobal,
    );
    for directory in project_directories {
        push_config_directory(
            &mut directories,
            &mut indices,
            &directory.join(".opencode"),
            LocalConfigDirectoryKind::Project,
            ExternalSourceScope::Project,
        );
    }
    if let Some(directory) = legacy_user_config_dir {
        push_config_directory(
            &mut directories,
            &mut indices,
            directory,
            LocalConfigDirectoryKind::Legacy,
            ExternalSourceScope::UserGlobal,
        );
    }
    if let Some(directory) = explicit_config_dir {
        let identity = path_identity(directory);
        if let Some(index) = indices.get(&identity).copied() {
            directories[index].kind = LocalConfigDirectoryKind::Explicit;
            directories[index].scope = explicit_config_scope;
        } else {
            push_config_directory(
                &mut directories,
                &mut indices,
                directory,
                LocalConfigDirectoryKind::Explicit,
                explicit_config_scope,
            );
        }
    }
    directories
}

pub(crate) fn local_watch_roots(
    user_config_dir: &Path,
    legacy_user_config_dir: Option<&Path>,
    explicit_config_file: Option<&Path>,
    explicit_config_dir: Option<&Path>,
    project_directories: &[PathBuf],
) -> Vec<ExternalWatchRoot> {
    let mut roots = BTreeMap::new();
    add_directory_watch_roots(&mut roots, user_config_dir);
    if let Some(directory) = legacy_user_config_dir {
        add_directory_watch_roots(&mut roots, directory);
    }
    if let Some(file) = explicit_config_file {
        if let Some(parent) = file.parent() {
            add_nearest_existing_watch_root(&mut roots, parent);
        }
    }
    if let Some(directory) = explicit_config_dir {
        add_directory_watch_roots(&mut roots, directory);
    }
    for directory in project_directories {
        add_watch_root(&mut roots, directory.clone(), false);
        add_directory_watch_roots(&mut roots, &directory.join(".opencode"));
    }
    roots
        .into_iter()
        .map(|(path, recursive)| ExternalWatchRoot { path, recursive })
        .collect()
}

fn push_config_directory(
    directories: &mut Vec<LocalConfigDirectory>,
    indices: &mut BTreeMap<PathBuf, usize>,
    path: &Path,
    kind: LocalConfigDirectoryKind,
    scope: ExternalSourceScope,
) {
    let identity = path_identity(path);
    if indices.contains_key(&identity) {
        return;
    }
    indices.insert(identity, directories.len());
    directories.push(LocalConfigDirectory {
        path: path.to_path_buf(),
        kind,
        scope,
    });
}

pub(crate) fn path_identity(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| normalize_path_lexically(path))
}

fn environment_truthy(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true"))
}

pub(crate) fn normalize_path_lexically(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn opened_directory(path: &Path) -> &Path {
    if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    }
}

fn nearest_existing_path(mut path: PathBuf) -> Option<PathBuf> {
    loop {
        if path.exists() {
            return Some(path);
        }
        if !path.pop() {
            return None;
        }
    }
}

fn add_watch_root(roots: &mut BTreeMap<PathBuf, bool>, path: PathBuf, recursive: bool) {
    roots
        .entry(path)
        .and_modify(|existing| *existing |= recursive)
        .or_insert(recursive);
}

fn add_nearest_existing_watch_root(roots: &mut BTreeMap<PathBuf, bool>, path: &Path) {
    if let Some(path) = nearest_existing_path(path.to_path_buf()) {
        add_watch_root(roots, path, false);
    }
}

fn add_directory_watch_roots(roots: &mut BTreeMap<PathBuf, bool>, directory: &Path) {
    if let Some(parent) = directory.parent() {
        add_nearest_existing_watch_root(roots, parent);
    }
    // Preserve the desired recursive root even before it exists. The shared
    // watcher can promote it after the nearest existing parent reports creation.
    add_watch_root(roots, directory.to_path_buf(), true);
}

#[cfg(test)]
mod tests {
    use super::{
        local_source_plan, ordered_local_config_directories, project_asset_directories,
        project_config_directories, user_config_dir, LocalConfigDirectory,
        LocalConfigDirectoryKind, LocalConfigDocumentKind, LocalConfigDocumentSource,
        LocalSourcePlanItem, OpenCodeLocalConfigOptions,
    };
    use openbitfun_product_domains::external_sources::ExternalSourceScope;
    use std::path::{Path, PathBuf};

    #[test]
    fn default_config_root_uses_xdg_semantics_on_every_platform() {
        assert_eq!(
            user_config_dir(None, Some(PathBuf::from("home"))),
            PathBuf::from("home/.config/opencode")
        );
        assert_eq!(
            user_config_dir(
                Some(PathBuf::from("custom-config")),
                Some(PathBuf::from("home"))
            ),
            PathBuf::from("custom-config/opencode")
        );
    }

    #[test]
    fn project_config_and_asset_phases_use_distinct_upstream_orders() {
        let root = Path::new("workspace");
        let opened = root.join("packages/app");

        assert_eq!(
            project_config_directories(root, &opened),
            vec![
                PathBuf::from("workspace"),
                PathBuf::from("workspace/packages"),
                PathBuf::from("workspace/packages/app"),
            ]
        );
        assert_eq!(
            project_asset_directories(root, &opened),
            vec![
                PathBuf::from("workspace/packages/app"),
                PathBuf::from("workspace/packages"),
                PathBuf::from("workspace"),
            ]
        );
    }

    #[test]
    fn config_directory_aliases_keep_the_first_position_and_path_derived_scope() {
        let user = Path::new("user/opencode");
        let project_directories = vec![
            PathBuf::from("workspace/packages/app"),
            PathBuf::from("workspace"),
        ];

        assert_eq!(
            ordered_local_config_directories(
                user,
                None,
                Some(user),
                ExternalSourceScope::UserGlobal,
                project_directories.as_slice()
            ),
            vec![
                LocalConfigDirectory {
                    path: user.to_path_buf(),
                    kind: LocalConfigDirectoryKind::Explicit,
                    scope: ExternalSourceScope::UserGlobal,
                },
                LocalConfigDirectory {
                    path: PathBuf::from("workspace/packages/app/.opencode"),
                    kind: LocalConfigDirectoryKind::Project,
                    scope: ExternalSourceScope::Project,
                },
                LocalConfigDirectory {
                    path: PathBuf::from("workspace/.opencode"),
                    kind: LocalConfigDirectoryKind::Project,
                    scope: ExternalSourceScope::Project,
                },
            ]
        );

        assert_eq!(
            ordered_local_config_directories(
                user,
                None,
                Some(Path::new("workspace/packages/app/.opencode")),
                ExternalSourceScope::WorkspaceLocal,
                project_directories.as_slice(),
            ),
            vec![
                LocalConfigDirectory {
                    path: user.to_path_buf(),
                    kind: LocalConfigDirectoryKind::User,
                    scope: ExternalSourceScope::UserGlobal,
                },
                LocalConfigDirectory {
                    path: PathBuf::from("workspace/packages/app/.opencode"),
                    kind: LocalConfigDirectoryKind::Explicit,
                    scope: ExternalSourceScope::WorkspaceLocal,
                },
                LocalConfigDirectory {
                    path: PathBuf::from("workspace/.opencode"),
                    kind: LocalConfigDirectoryKind::Project,
                    scope: ExternalSourceScope::Project,
                },
            ]
        );
    }

    #[test]
    fn source_plan_matches_upstream_local_precedence_and_keeps_inline_last() {
        let temp = tempfile::tempdir().unwrap();
        let user = temp.path().join("user/opencode");
        let project = temp.path().join("workspace");
        let opened = project.join("packages/app");
        let explicit_file = temp.path().join("explicit.json");
        let explicit_dir = temp.path().join("explicit-dir");
        for directory in [
            user.as_path(),
            project.as_path(),
            opened.as_path(),
            project.join(".opencode").as_path(),
            opened.join(".opencode").as_path(),
            explicit_dir.as_path(),
        ] {
            std::fs::create_dir_all(directory).unwrap();
        }
        std::fs::create_dir_all(project.join(".git")).unwrap();
        for path in [
            user.join("config.json"),
            user.join("opencode.json"),
            explicit_file.clone(),
            project.join("opencode.json"),
            opened.join("opencode.jsonc"),
            project.join(".opencode/opencode.json"),
            opened.join(".opencode/opencode.json"),
            explicit_dir.join("opencode.json"),
        ] {
            std::fs::write(path, "{}").unwrap();
        }

        let items = local_source_plan(
            &OpenCodeLocalConfigOptions {
                user_config_dir: user.clone(),
                legacy_user_config_dir: None,
                explicit_config_file: Some(explicit_file.clone()),
                explicit_config_dir: Some(explicit_dir.clone()),
                inline_config_content: Some(r#"{"command":{}}"#.to_string()),
                project_config_enabled: true,
            },
            Some(&opened),
            Some(&project),
        );
        let labels = items
            .iter()
            .map(|item| match item {
                LocalSourcePlanItem::Config(document) => match &document.source {
                    LocalConfigDocumentSource::File(path) => path
                        .strip_prefix(temp.path())
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    LocalConfigDocumentSource::Inline(_) => "inline".to_string(),
                },
                LocalSourcePlanItem::Directory(directory) => format!(
                    "dir:{}",
                    directory
                        .path
                        .strip_prefix(temp.path())
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/")
                ),
            })
            .collect::<Vec<_>>();

        assert_eq!(
            labels,
            vec![
                "user/opencode/config.json",
                "user/opencode/opencode.json",
                "explicit.json",
                "workspace/opencode.json",
                "workspace/packages/app/opencode.jsonc",
                "dir:user/opencode",
                "workspace/packages/app/.opencode/opencode.json",
                "dir:workspace/packages/app/.opencode",
                "dir:workspace/packages/.opencode",
                "workspace/.opencode/opencode.json",
                "dir:workspace/.opencode",
                "explicit-dir/opencode.json",
                "dir:explicit-dir",
                "inline",
            ]
        );
        let inline = items.last().unwrap();
        assert!(matches!(
            inline,
            LocalSourcePlanItem::Config(document)
                if document.kind == LocalConfigDocumentKind::Inline
                    && document.scope == ExternalSourceScope::WorkspaceLocal
                    && document.base_directory.as_deref() == Some(opened.as_path())
        ));
        assert!(items.iter().any(|item| matches!(
            item,
            LocalSourcePlanItem::Config(document)
                if document.kind == LocalConfigDocumentKind::ExplicitFile
                    && document.scope == ExternalSourceScope::UserGlobal
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            LocalSourcePlanItem::Directory(directory)
                if directory.kind == LocalConfigDirectoryKind::Explicit
                    && directory.scope == ExternalSourceScope::UserGlobal
        )));
    }

    #[test]
    fn relative_environment_paths_require_and_resolve_against_the_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let user = temp.path().join("user/opencode");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(workspace.join("custom-dir")).unwrap();
        std::fs::write(workspace.join("custom.json"), "{}").unwrap();
        std::fs::write(workspace.join("custom-dir/opencode.json"), "{}").unwrap();
        let options = OpenCodeLocalConfigOptions {
            user_config_dir: user,
            legacy_user_config_dir: None,
            explicit_config_file: Some(PathBuf::from("custom.json")),
            explicit_config_dir: Some(PathBuf::from("custom-dir")),
            inline_config_content: None,
            project_config_enabled: false,
        };

        let without_workspace = local_source_plan(&options, None, None);
        assert!(without_workspace.iter().all(|item| !matches!(
            item,
            LocalSourcePlanItem::Config(document)
                if document.kind == LocalConfigDocumentKind::ExplicitFile
        )));

        let with_workspace = local_source_plan(&options, Some(&workspace), None);
        assert!(with_workspace.iter().any(|item| matches!(
            item,
            LocalSourcePlanItem::Config(document)
                if document.file_path() == Some(workspace.join("custom.json").as_path())
                    && document.scope == ExternalSourceScope::WorkspaceLocal
        )));
        assert!(with_workspace.iter().any(|item| matches!(
            item,
            LocalSourcePlanItem::Directory(directory)
                if directory.path == workspace.join("custom-dir")
                    && directory.scope == ExternalSourceScope::WorkspaceLocal
        )));
    }
}
