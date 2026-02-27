fn main() {
    emit_rerun_if_changed(std::path::Path::new("builtin_skills"));

    // Run the build script to embed prompts data
    if let Err(e) = build_embedded_prompts() {
        eprintln!("Warning: Failed to embed prompts data: {}", e);
    }
}

fn build_embedded_prompts() -> Result<(), Box<dyn std::error::Error>> {
    // Embed prompts data
    embed_agents_prompt_data()
}

fn escape_rust_string(s: &str) -> String {
    // To avoid quote issues, return the original string directly
    // Using r### syntax can include any character
    s.to_string()
}

fn emit_rerun_if_changed(path: &std::path::Path) {
    if !path.exists() {
        return;
    }

    println!("cargo:rerun-if-changed={}", path.display());

    if path.is_dir() {
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            emit_rerun_if_changed(&entry.path());
        }
    }
}

// Function to embed prompts data
fn embed_agents_prompt_data() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;
    use std::path::Path;

    println!("cargo:rerun-if-changed=src/agentic/agents/prompts");

    // Get CARGO_MANIFEST_DIR (i.e. crates/core directory)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;

    let mut prompts = HashMap::new();

    // Read agentic agents prompts
    let agentic_prompt_path = Path::new(&manifest_dir)
        .join("src")
        .join("agentic")
        .join("agents")
        .join("prompts");

    if agentic_prompt_path.exists() {
        println!("Embedding agentic prompts from: {:?}", agentic_prompt_path);
        read_prompts_recursive(&agentic_prompt_path, &agentic_prompt_path, &mut prompts)?;
    } else {
        eprintln!(
            "Warning: agentic prompts directory not found at {:?}",
            agentic_prompt_path
        );
    }

    if prompts.is_empty() {
        // Create empty embedded data
        create_empty_embedded_prompts(&manifest_dir)?;
        return Ok(());
    }

    // Generate embedded Rust code
    generate_embedded_prompts_code(&manifest_dir, &prompts)?;

    println!("Successfully embedded {} prompt files", prompts.len());

    Ok(())
}

fn check_extension(path: &std::path::Path) -> bool {
    if let Some(ext) = path.extension() {
        match ext.to_str() {
            Some(ext) => ext == "txt" || ext == "md",
            None => false,
        }
    } else {
        false
    }
}

fn gen_key(relative_path: &str, prefix: &str) -> String {
    format!(
        "{}{}",
        prefix,
        relative_path
            .trim_end_matches(".txt")
            .trim_end_matches(".md")
    )
}

// Read prompts and add prefix
#[allow(dead_code)]
fn read_prompts_with_prefix(
    current_dir: &std::path::Path,
    base_dir: &std::path::Path,
    prefix: &str,
    prompts: &mut std::collections::HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            read_prompts_with_prefix(&path, base_dir, prefix, prompts)?;
        } else if check_extension(&path) {
            let content = fs::read_to_string(&path)?;
            let relative_path = path
                .strip_prefix(base_dir)?
                .to_string_lossy()
                .replace('\\', "/"); // Use forward slash

            // Remove extension and add prefix as key
            let key = gen_key(&relative_path, prefix);
            prompts.insert(key, content);
        }
    }

    Ok(())
}

fn read_prompts_recursive(
    current_dir: &std::path::Path,
    base_dir: &std::path::Path,
    prompts: &mut std::collections::HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            read_prompts_recursive(&path, base_dir, prompts)?;
        } else if check_extension(&path) {
            let content = fs::read_to_string(&path)?;
            let relative_path = path
                .strip_prefix(base_dir)?
                .to_string_lossy()
                .replace('\\', "/"); // Use forward slash

            // Remove extension as key
            let key = gen_key(&relative_path, "");
            prompts.insert(key, content);
        }
    }

    Ok(())
}

fn generate_embedded_prompts_code(
    _manifest_dir: &str,
    prompts: &std::collections::HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    let out_dir = std::env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("embedded_agents_prompt.rs");
    let mut file = fs::File::create(&dest_path)?;

    writeln!(file, "// Embedded Agent Prompt data")?;
    writeln!(
        file,
        "// This file is automatically generated by the build script, do not modify manually"
    )?;
    writeln!(file)?;
    writeln!(file, "use std::collections::HashMap;")?;
    writeln!(file, "use once_cell::sync::Lazy;")?;
    writeln!(file)?;

    // Embed all prompt content
    writeln!(file, "/// Embedded prompt content mapping")?;
    writeln!(
        file,
        "pub static EMBEDDED_PROMPTS: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {{"
    )?;
    writeln!(file, "    let mut map = HashMap::new();")?;

    for (name, content) in prompts {
        writeln!(
            file,
            "    map.insert(r###\"{}\"###, r###\"{}\"###);",
            escape_rust_string(name),
            escape_rust_string(content)
        )?;
    }

    writeln!(file, "    map")?;
    writeln!(file, "}});")?;
    writeln!(file)?;

    // Add convenient functions
    writeln!(file, "/// Get embedded prompt content")?;
    writeln!(
        file,
        "pub fn get_embedded_prompt(prompt_name: &str) -> Option<&'static str> {{"
    )?;
    writeln!(file, "    EMBEDDED_PROMPTS.get(prompt_name).copied()")?;
    writeln!(file, "}}")?;
    writeln!(file)?;

    writeln!(file, "/// Get all embedded prompt names")?;
    writeln!(file, "#[allow(dead_code)]")?;
    writeln!(
        file,
        "pub fn get_all_embedded_prompt_names() -> Vec<&'static str> {{"
    )?;
    writeln!(file, "    EMBEDDED_PROMPTS.keys().copied().collect()")?;
    writeln!(file, "}}")?;

    Ok(())
}

fn create_empty_embedded_prompts(_manifest_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    let out_dir = std::env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("embedded_agents_prompt.rs");
    let mut file = fs::File::create(&dest_path)?;

    writeln!(file, "// Empty embedded data (prompts directory not found)")?;
    writeln!(file, "use std::collections::HashMap;")?;
    writeln!(file, "use once_cell::sync::Lazy;")?;
    writeln!(file)?;
    writeln!(file, "pub static EMBEDDED_PROMPTS: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| HashMap::new());")?;
    writeln!(
        file,
        "pub fn get_embedded_prompt(_prompt_name: &str) -> Option<&'static str> {{ None }}"
    )?;
    writeln!(
        file,
        "pub fn get_all_embedded_prompt_names() -> Vec<&'static str> {{ Vec::new() }}"
    )?;

    Ok(())
}
