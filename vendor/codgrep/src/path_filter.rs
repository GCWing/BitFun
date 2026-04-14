use std::path::{Path, PathBuf};

use ignore::{
    overrides::{Override, OverrideBuilder},
    types::{FileTypeDef, Types, TypesBuilder},
    Match,
};

use crate::{error::Result, path_utils::normalize_path as normalize_path_impl};

#[derive(Debug, Clone, Default)]
pub struct PathFilterArgs {
    pub roots: Vec<PathBuf>,
    pub globs: Vec<String>,
    pub iglobs: Vec<String>,
    pub type_add: Vec<String>,
    pub type_clear: Vec<String>,
    pub types: Vec<String>,
    pub type_not: Vec<String>,
}

#[derive(Debug)]
pub struct PathFilter {
    roots: Vec<PathRoot>,
    overrides: Override,
    types: Types,
}

impl PathFilter {
    pub fn new(args: PathFilterArgs, current_dir: &Path) -> Result<Self> {
        let roots = args
            .roots
            .into_iter()
            .map(|path| {
                let normalized = normalize_path(&path, current_dir);
                let is_dir = normalized.is_dir();
                PathRoot {
                    path: normalized,
                    is_dir,
                }
            })
            .collect::<Vec<_>>();
        let root = common_search_root(&roots).unwrap_or_else(|| current_dir.to_path_buf());

        let mut override_builder = OverrideBuilder::new(&root);
        for glob in &args.globs {
            override_builder.add(glob)?;
        }
        if !args.iglobs.is_empty() {
            override_builder.case_insensitive(true)?;
            for glob in &args.iglobs {
                override_builder.add(glob)?;
            }
        }

        let mut type_builder = TypesBuilder::new();
        type_builder.add_defaults();
        for name in &args.type_clear {
            type_builder.clear(name);
        }
        for definition in &args.type_add {
            type_builder.add_def(definition)?;
        }
        for name in &args.types {
            type_builder.select(name);
        }
        for name in &args.type_not {
            type_builder.negate(name);
        }

        Ok(Self {
            roots,
            overrides: override_builder.build()?,
            types: type_builder.build()?,
        })
    }

    pub fn matches_file(&self, path: &Path) -> bool {
        if !self.roots.is_empty() && !self.roots.iter().any(|root| root.matches(path)) {
            return false;
        }

        match self.overrides.matched(path, false) {
            Match::Ignore(_) => return false,
            Match::Whitelist(_) => return true,
            Match::None => {}
        }

        !self.types.matched(path, false).is_ignore()
    }

    pub fn type_definitions(&self) -> &[FileTypeDef] {
        self.types.definitions()
    }
}

#[derive(Debug, Clone)]
struct PathRoot {
    path: PathBuf,
    is_dir: bool,
}

impl PathRoot {
    fn matches(&self, candidate: &Path) -> bool {
        if self.is_dir {
            candidate.starts_with(&self.path)
        } else {
            candidate == self.path
        }
    }
}

fn common_search_root(roots: &[PathRoot]) -> Option<PathBuf> {
    let mut roots = roots
        .iter()
        .map(|root| {
            if root.is_dir {
                root.path.clone()
            } else {
                root.path.parent().unwrap_or(&root.path).to_path_buf()
            }
        })
        .collect::<Vec<_>>();
    let mut common = roots.pop()?;
    for root in roots {
        common = common_prefix(&common, &root)?;
    }
    Some(common)
}

fn common_prefix(left: &Path, right: &Path) -> Option<PathBuf> {
    let mut prefix = PathBuf::new();
    for (left_component, right_component) in left.components().zip(right.components()) {
        if left_component != right_component {
            break;
        }
        prefix.push(left_component.as_os_str());
    }
    if prefix.as_os_str().is_empty() {
        None
    } else {
        Some(prefix)
    }
}

pub fn normalize_path(path: &Path, current_dir: &Path) -> PathBuf {
    normalize_path_impl(path, current_dir)
}

#[cfg(test)]
mod tests {
    use super::{normalize_path, PathFilter, PathFilterArgs};
    use tempfile::tempdir;

    #[test]
    fn glob_filters_include_and_exclude_paths() {
        let temp = tempdir().expect("test should succeed");
        let current_dir = temp.path().to_path_buf();
        std::fs::create_dir_all(current_dir.join("src")).expect("test should succeed");
        std::fs::write(current_dir.join("src/lib.rs"), "").expect("test should succeed");
        std::fs::write(current_dir.join("src/lib.py"), "").expect("test should succeed");
        std::fs::write(current_dir.join("generated.rs"), "").expect("test should succeed");
        let filter = PathFilter::new(
            PathFilterArgs {
                roots: vec![current_dir.clone()],
                globs: vec!["*.rs".into(), "!generated.rs".into()],
                ..PathFilterArgs::default()
            },
            &current_dir,
        )
        .expect("test should succeed");

        assert!(filter.matches_file(&normalize_path(
            std::path::Path::new("src/lib.rs"),
            &current_dir,
        )));
        assert!(!filter.matches_file(&normalize_path(
            std::path::Path::new("src/lib.py"),
            &current_dir,
        )));
        assert!(!filter.matches_file(&normalize_path(
            std::path::Path::new("generated.rs"),
            &current_dir,
        )));
    }

    #[test]
    fn type_filters_select_and_negate_default_types() {
        let temp = tempdir().expect("test should succeed");
        let current_dir = temp.path().to_path_buf();
        std::fs::create_dir_all(current_dir.join("src")).expect("test should succeed");
        std::fs::write(current_dir.join("src/lib.rs"), "").expect("test should succeed");
        std::fs::write(current_dir.join("Cargo.toml"), "").expect("test should succeed");
        let filter = PathFilter::new(
            PathFilterArgs {
                roots: vec![current_dir.clone()],
                types: vec!["rust".into()],
                type_not: vec!["toml".into()],
                ..PathFilterArgs::default()
            },
            &current_dir,
        )
        .expect("test should succeed");

        assert!(filter.matches_file(&normalize_path(
            std::path::Path::new("src/lib.rs"),
            &current_dir,
        )));
        assert!(!filter.matches_file(&normalize_path(
            std::path::Path::new("Cargo.toml"),
            &current_dir,
        )));
    }
}
