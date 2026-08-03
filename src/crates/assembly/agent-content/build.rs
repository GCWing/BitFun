use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(error) = generate_agent_prompt_catalog() {
        eprintln!("Warning: Failed to embed built-in Agent prompts: {error}");
    }
}

fn generate_agent_prompt_catalog() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let prompt_root = manifest_dir.join("prompts");
    let roots = [
        prompt_root.join("agents"),
        prompt_root.join("shared"),
        prompt_root.join("memories"),
    ];
    let mut prompts = HashMap::new();

    for root in &roots {
        println!("cargo:rerun-if-changed={}", root.display());
        if root.exists() {
            read_prompts_recursive(root, root, &mut prompts)?;
        } else {
            eprintln!("Warning: built-in Agent prompt directory not found at {root:?}");
        }
    }

    let mut prompts: Vec<_> = prompts.into_iter().collect();
    prompts.sort_by(|left, right| left.0.cmp(&right.0));
    write_catalog(&prompts)
}

fn read_prompts_recursive(
    current_dir: &Path,
    base_dir: &Path,
    prompts: &mut HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(current_dir)? {
        let path = entry?.path();
        if path.is_dir() {
            read_prompts_recursive(&path, base_dir, prompts)?;
            continue;
        }

        let extension = path.extension().and_then(|extension| extension.to_str());
        if !matches!(extension, Some("md" | "txt")) {
            continue;
        }

        let relative_path = path
            .strip_prefix(base_dir)?
            .to_string_lossy()
            .replace('\\', "/");
        let key = relative_path
            .trim_end_matches(".txt")
            .trim_end_matches(".md")
            .to_string();
        let content = fs::read_to_string(&path)?;
        if prompts.insert(key.clone(), content).is_some() {
            return Err(format!("duplicate built-in Agent prompt key: {key}").into());
        }
    }

    Ok(())
}

fn write_catalog(prompts: &[(String, String)]) -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let mut file = fs::File::create(out_dir.join("embedded_agent_prompts.rs"))?;

    writeln!(file, "use std::collections::HashMap;")?;
    writeln!(file, "use std::sync::LazyLock;")?;
    writeln!(file)?;
    writeln!(
        file,
        "pub static EMBEDDED_PROMPTS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {{"
    )?;
    writeln!(file, "    let mut prompts = HashMap::new();")?;
    for (key, content) in prompts {
        writeln!(
            file,
            "    prompts.insert(r###\"{key}\"###, r###\"{content}\"###);"
        )?;
    }
    writeln!(file, "    prompts")?;
    writeln!(file, "}});")?;
    writeln!(file)?;

    writeln!(
        file,
        "pub fn agent_prompt(name: &str) -> Option<&'static str> {{ EMBEDDED_PROMPTS.get(name).copied() }}"
    )?;
    writeln!(
        file,
        "pub fn agent_prompt_names() -> Vec<&'static str> {{ EMBEDDED_PROMPTS.keys().copied().collect() }}"
    )?;

    Ok(())
}
