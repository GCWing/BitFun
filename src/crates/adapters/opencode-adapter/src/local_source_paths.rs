use bitfun_product_domains::external_sources::{ExternalSourceScope, ExternalWatchRoot};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct OpenCodeLocalConfigOptions {
    pub user_config_dir: PathBuf,
    pub legacy_user_config_dir: Option<PathBuf>,
    pub explicit_config_file: Option<PathBuf>,
    pub explicit_config_dir: Option<PathBuf>,
    pub project_config_enabled: bool,
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
            project_config_enabled: !environment_truthy("OPENCODE_DISABLE_PROJECT_CONFIG"),
        }
    }
}

impl Default for OpenCodeLocalConfigOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalConfigFileLayer {
    pub(crate) path: PathBuf,
    pub(crate) scope: ExternalSourceScope,
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
            directories[index].scope = ExternalSourceScope::WorkspaceLocal;
        } else {
            push_config_directory(
                &mut directories,
                &mut indices,
                directory,
                LocalConfigDirectoryKind::Explicit,
                ExternalSourceScope::WorkspaceLocal,
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

/// Configuration documents consumed by OpenCode V2 configuration plugins,
/// ordered from lowest to highest priority. `OPENCODE_CONFIG_DIR` replaces
/// the default global root; project files then override global files, and
/// `.opencode` files override direct project files.
pub(crate) fn reference_config_file_layers(
    global_config_dir: &Path,
    workspace_root: Option<&Path>,
) -> std::io::Result<Vec<LocalConfigFileLayer>> {
    let mut layers = Vec::new();
    push_config_files(
        &mut layers,
        global_config_dir,
        ExternalSourceScope::UserGlobal,
        false,
    )?;
    let project_directories = workspace_root
        .map(|workspace| project_config_directories(&find_project_root(workspace), workspace))
        .unwrap_or_default();
    for directory in &project_directories {
        push_config_files(&mut layers, directory, ExternalSourceScope::Project, false)?;
    }
    for directory in &project_directories {
        push_config_files(
            &mut layers,
            &directory.join(".opencode"),
            ExternalSourceScope::Project,
            false,
        )?;
    }
    Ok(deduplicate_config_file_layers_keep_last(layers))
}

pub(crate) fn reference_watch_roots(
    global_config_dir: &Path,
    workspace_root: Option<&Path>,
) -> Vec<ExternalWatchRoot> {
    let mut roots = BTreeMap::new();
    add_directory_watch_roots(&mut roots, global_config_dir);
    if let Some(workspace) = workspace_root {
        for directory in project_config_directories(&find_project_root(workspace), workspace) {
            add_watch_root(&mut roots, directory.clone(), false);
            add_directory_watch_roots(&mut roots, &directory.join(".opencode"));
        }
    }
    roots
        .into_iter()
        .map(|(path, recursive)| ExternalWatchRoot { path, recursive })
        .collect()
}

fn push_config_files(
    layers: &mut Vec<LocalConfigFileLayer>,
    directory: &Path,
    scope: ExternalSourceScope,
    include_legacy_config_name: bool,
) -> std::io::Result<()> {
    if include_legacy_config_name {
        push_config_file(layers, &directory.join("config.json"), scope)?;
    }
    for name in ["opencode.json", "opencode.jsonc"] {
        push_config_file(layers, &directory.join(name), scope)?;
    }
    Ok(())
}

fn push_config_file(
    layers: &mut Vec<LocalConfigFileLayer>,
    path: &Path,
    scope: ExternalSourceScope,
) -> std::io::Result<()> {
    match config_file_metadata(std::fs::metadata(path))? {
        true => {
            layers.push(LocalConfigFileLayer {
                path: path.to_path_buf(),
                scope,
            });
            Ok(())
        }
        false => Ok(()),
    }
}

fn config_file_metadata(metadata: std::io::Result<std::fs::Metadata>) -> std::io::Result<bool> {
    match metadata {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn deduplicate_config_file_layers_keep_last(
    layers: Vec<LocalConfigFileLayer>,
) -> Vec<LocalConfigFileLayer> {
    let mut seen = std::collections::BTreeSet::new();
    let mut unique = layers
        .into_iter()
        .rev()
        .filter(|layer| seen.insert(path_identity(&layer.path)))
        .collect::<Vec<_>>();
    unique.reverse();
    unique
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
        config_file_metadata, ordered_local_config_directories, project_asset_directories,
        project_config_directories, user_config_dir, LocalConfigDirectory,
        LocalConfigDirectoryKind,
    };
    use bitfun_product_domains::external_sources::ExternalSourceScope;
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
    fn config_metadata_errors_are_not_treated_as_missing_files() {
        let error = config_file_metadata(Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        )))
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
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
    fn config_directory_aliases_keep_the_first_position_and_explicit_scope() {
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
                project_directories.as_slice()
            ),
            vec![
                LocalConfigDirectory {
                    path: user.to_path_buf(),
                    kind: LocalConfigDirectoryKind::Explicit,
                    scope: ExternalSourceScope::WorkspaceLocal,
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
}
