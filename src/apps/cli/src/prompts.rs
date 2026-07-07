// Embedded CLI prompts (auto-generated from `prompts/` directory at build time)

include!(concat!(env!("OUT_DIR"), "/embedded_cli_prompts.rs"));

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const CLI_PROMPTS_ENV: &str = "BITFUN_CLI_PROMPTS_DIR";
const SHARED_PROMPTS_ENV: &str = "BITFUN_PROMPTS_DIR";

static CLI_PROMPT_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub fn set_runtime_cli_prompt_dir(path: impl Into<PathBuf>) {
    let _ = CLI_PROMPT_DIR_OVERRIDE.set(path.into());
}

pub fn get_cli_prompt_text(name: &str) -> Option<String> {
    get_runtime_cli_prompt(name).or_else(|| get_cli_prompt(name).map(str::to_string))
}

fn get_runtime_cli_prompt(name: &str) -> Option<String> {
    let relative_path = prompt_relative_path(name)?;
    runtime_prompt_roots()
        .into_iter()
        .find_map(|root| read_prompt_file(&root, &relative_path))
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

    if let Some(path) = CLI_PROMPT_DIR_OVERRIDE.get() {
        add_prompt_roots(path, &mut roots);
    }

    if let Some(path) = std::env::var_os(CLI_PROMPTS_ENV) {
        add_prompt_roots(Path::new(&path), &mut roots);
    }

    if let Some(path) = std::env::var_os(SHARED_PROMPTS_ENV) {
        add_prompt_roots(Path::new(&path), &mut roots);
    }

    add_prompt_roots(Path::new(env!("CARGO_MANIFEST_DIR")), &mut roots);
    roots
}

fn add_prompt_roots(path: &Path, roots: &mut Vec<PathBuf>) {
    let candidates = [
        path.to_path_buf(),
        path.join("prompts"),
        path.join("src").join("apps").join("cli"),
        path.join("src").join("apps").join("cli").join("prompts"),
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

fn read_prompt_file(root: &Path, relative_path: &Path) -> Option<String> {
    for extension in ["md", "txt"] {
        let path = root.join(relative_path).with_extension(extension);
        if path.is_file() {
            return std::fs::read_to_string(path).ok();
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{prompt_relative_path, read_prompt_file};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn runtime_cli_prompt_reads_markdown_file() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        fs::write(tempdir.path().join("init.md"), "runtime init").expect("write prompt");

        let prompt = read_prompt_file(tempdir.path(), &PathBuf::from("init")).expect("prompt");
        assert_eq!(prompt, "runtime init");
    }

    #[test]
    fn runtime_cli_prompt_rejects_path_traversal() {
        assert!(prompt_relative_path("../init").is_none());
    }
}
