use openbitfun_runtime_ports::{WorkspaceDirEntry, WorkspaceFileSystem};
use std::collections::VecDeque;

#[derive(Clone)]
pub(crate) struct DirectoryIgnore {
    depth: usize,
    override_gitignore: bool,
    rules: super::workspace_ignore::WorkspaceIgnoreRules,
}

pub(crate) async fn add_directory_ignores(
    fs: &dyn WorkspaceFileSystem,
    entries: &[openbitfun_runtime_ports::WorkspaceDirEntry],
    depth: usize,
    rules: &mut Vec<DirectoryIgnore>,
) -> Result<(), String> {
    // At the same directory, .ignore overrides .gitignore, as in ignore's walker.
    for name in [".gitignore", ".ignore"] {
        if let Some(entry) = entries
            .iter()
            .find(|entry| entry.name == name && !entry.is_dir && !entry.is_symlink)
        {
            let text = fs
                .read_file_text_bounded(&entry.path, 1024 * 1024)
                .await
                .map_err(|error| format!("Failed to read {}: {error}", entry.path))?
                .ok_or_else(|| format!("Ignore file exceeds 1048576 bytes: {}", entry.path))?;
            rules.push(DirectoryIgnore {
                depth,
                override_gitignore: name == ".ignore",
                rules: super::workspace_ignore::WorkspaceIgnoreRules::parse(&text)
                    .map_err(|error| format!("Invalid ignore file {}: {error}", entry.path))?,
            });
        }
    }
    Ok(())
}

pub(crate) struct WorkspaceWalkFile {
    pub entry: WorkspaceDirEntry,
    pub relative_path: String,
}

/// Streams file candidates without buffering a complete workspace listing.
/// Ignore files and filenames use provider-relative POSIX components.
pub(crate) struct WorkspaceFileWalker<'a> {
    fs: &'a dyn WorkspaceFileSystem,
    pending: VecDeque<(String, Vec<String>, Vec<DirectoryIgnore>)>,
    entries: std::vec::IntoIter<WorkspaceDirEntry>,
    components: Vec<String>,
    ignores: Vec<DirectoryIgnore>,
    base_depth: usize,
    hide_hidden: bool,
    apply_ignore: bool,
    include_symlink_entries: bool,
}

impl<'a> WorkspaceFileWalker<'a> {
    pub(crate) fn new(
        fs: &'a dyn WorkspaceFileSystem,
        root: String,
        hide_hidden: bool,
        apply_ignore: bool,
    ) -> Self {
        Self::with_scope(fs, root, Vec::new(), Vec::new(), hide_hidden, apply_ignore)
    }

    pub(crate) fn with_scope(
        fs: &'a dyn WorkspaceFileSystem,
        root: String,
        prefix: Vec<String>,
        ignores: Vec<DirectoryIgnore>,
        hide_hidden: bool,
        apply_ignore: bool,
    ) -> Self {
        let base_depth = prefix.len();
        Self {
            fs,
            pending: VecDeque::from([(root, prefix, ignores)]),
            entries: Vec::new().into_iter(),
            components: Vec::new(),
            ignores: Vec::new(),
            base_depth,
            hide_hidden,
            apply_ignore,
            include_symlink_entries: false,
        }
    }

    /// Let consumers inspect final symlink targets without ever following a
    /// linked directory during traversal. Grep preserves native file-link reads;
    /// callers with no-follow policies leave this disabled.
    pub(crate) fn with_symlink_entries(mut self) -> Self {
        self.include_symlink_entries = true;
        self
    }

    pub(crate) async fn next(&mut self) -> Result<Option<WorkspaceWalkFile>, String> {
        loop {
            if let Some(entry) = self.entries.next() {
                if (entry.is_symlink && !self.include_symlink_entries)
                    || [".git", ".svn", ".hg", ".bzr", ".jj", ".sl"].contains(&entry.name.as_str())
                    || (self.hide_hidden && entry.name.starts_with('.'))
                {
                    continue;
                }
                let mut components = self.components.clone();
                components.push(entry.name.clone());
                let mut ignored = false;
                for override_gitignore in [false, true] {
                    for scope in self
                        .ignores
                        .iter()
                        .filter(|scope| scope.override_gitignore == override_gitignore)
                    {
                        if let Some(decision) = scope
                            .rules
                            .matched(&components[scope.depth..].join("/"), entry.is_dir)
                        {
                            ignored = decision;
                        }
                    }
                }
                if ignored {
                    continue;
                }
                if entry.is_dir && !entry.is_symlink {
                    self.pending
                        .push_back((entry.path, components, self.ignores.clone()));
                    continue;
                }
                return Ok(Some(WorkspaceWalkFile {
                    entry,
                    relative_path: components[self.base_depth..].join("/"),
                }));
            }
            let Some((path, components, mut ignores)) = self.pending.pop_front() else {
                return Ok(None);
            };
            let entries = self
                .fs
                .read_dir(&path)
                .await
                .map_err(|error| format!("Failed to list {path}: {error}"))?;
            if self.apply_ignore {
                add_directory_ignores(self.fs, &entries, components.len(), &mut ignores).await?;
            }
            self.entries = entries.into_iter();
            self.components = components;
            self.ignores = ignores;
        }
    }
}
