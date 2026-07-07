use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

pub const AGENT_PROMPTS_ENV: &str = "BITFUN_PROMPTS_DIR";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePrompt {
    pub content: String,
    pub cache_fingerprint: String,
}

static PROMPT_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub fn set_runtime_prompt_dir(path: impl Into<PathBuf>) {
    let _ = PROMPT_DIR_OVERRIDE.set(path.into());
}

pub fn get_runtime_prompt(name: &str) -> Option<RuntimePrompt> {
    let relative_path = prompt_relative_path(name)?;
    runtime_prompt_roots()
        .into_iter()
        .find_map(|root| read_runtime_prompt(&root, &relative_path))
}

#[cfg(test)]
pub(crate) fn get_runtime_prompt_from_root(root: &Path, name: &str) -> Option<RuntimePrompt> {
    let relative_path = prompt_relative_path(name)?;
    let mut roots = Vec::new();
    add_prompt_roots(root, &mut roots);
    roots
        .into_iter()
        .find_map(|root| read_runtime_prompt(&root, &relative_path))
}

pub fn cache_fingerprint(name: &str) -> Option<String> {
    get_runtime_prompt(name).map(|prompt| prompt.cache_fingerprint)
}

fn prompt_relative_path(name: &str) -> Option<PathBuf> {
    let name = name.trim();
    if name.is_empty()
        || name.starts_with('/')
        || name.starts_with('\\')
        || name
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return None;
    }

    let mut path = PathBuf::new();
    for component in name.split('/') {
        path.push(component);
    }
    Some(path)
}

fn runtime_prompt_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(path) = PROMPT_DIR_OVERRIDE.get() {
        add_prompt_roots(path, &mut roots);
    }

    if let Some(path) = std::env::var_os(AGENT_PROMPTS_ENV) {
        add_prompt_roots(Path::new(&path), &mut roots);
    }

    add_prompt_roots(Path::new(env!("CARGO_MANIFEST_DIR")), &mut roots);
    roots
}

fn add_prompt_roots(path: &Path, roots: &mut Vec<PathBuf>) {
    let candidates = [
        path.to_path_buf(),
        path.join("prompts"),
        path.join("src")
            .join("agentic")
            .join("agents")
            .join("prompts"),
        path.join("src").join("agentic").join("prompts"),
        path.join("src")
            .join("crates")
            .join("assembly")
            .join("core"),
        path.join("src")
            .join("crates")
            .join("assembly")
            .join("core")
            .join("src")
            .join("agentic")
            .join("agents")
            .join("prompts"),
        path.join("src")
            .join("crates")
            .join("assembly")
            .join("core")
            .join("src")
            .join("agentic")
            .join("prompts"),
    ];

    for candidate in candidates {
        if !candidate.is_dir() {
            continue;
        }
        let normalized = dunce::simplified(&candidate).to_path_buf();
        if !roots.iter().any(|existing| existing == &normalized) {
            roots.push(normalized);
        }
    }
}

fn read_runtime_prompt(root: &Path, relative_path: &Path) -> Option<RuntimePrompt> {
    for extension in ["md", "txt"] {
        let path = root.join(relative_path).with_extension(extension);
        if !path.is_file() {
            continue;
        }

        let content = std::fs::read_to_string(&path).ok()?;
        return Some(RuntimePrompt {
            content,
            cache_fingerprint: file_cache_fingerprint(&path),
        });
    }

    None
}

fn file_cache_fingerprint(path: &Path) -> String {
    let path = dunce::simplified(path);
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return format!("runtime:{}", path.display()),
    };
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    format!(
        "runtime:{}:{}:{}",
        path.display(),
        metadata.len(),
        modified_ms
    )
}

#[cfg(test)]
mod tests {
    use super::{prompt_relative_path, read_runtime_prompt};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn runtime_prompt_reads_markdown_override() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        fs::write(tempdir.path().join("agentic_mode.md"), "runtime prompt").expect("write prompt");

        let prompt =
            read_runtime_prompt(tempdir.path(), &PathBuf::from("agentic_mode")).expect("prompt");
        assert_eq!(prompt.content, "runtime prompt");
        assert!(prompt.cache_fingerprint.contains("agentic_mode.md"));
    }

    #[test]
    fn runtime_prompt_rejects_path_traversal() {
        assert!(prompt_relative_path("../secret").is_none());
    }
}
