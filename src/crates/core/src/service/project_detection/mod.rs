//! Project type detector
//!
//! Features:
//! - Scans the workspace to identify project types
//! - Detects programming languages in use
//! - Counts files by type
//! - Determines the primary programming language
//! - Supports monorepos and subdirectory project layouts

use anyhow::Result;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Project information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct ProjectInfo {
    /// Detected languages.
    pub languages: Vec<String>,
    /// Primary language (usually the one with the most files).
    pub primary_language: Option<String>,
    /// File counts per language.
    pub file_counts: HashMap<String, usize>,
    /// Project type tags.
    pub project_types: Vec<String>,
    /// Total file count.
    pub total_files: usize,
}

/// Project type detector.
pub struct ProjectDetector;

impl ProjectDetector {
    /// Detects the project type.
    pub async fn detect(workspace_path: &Path) -> Result<ProjectInfo> {
        debug!("Detecting project type for: {:?}", workspace_path);

        let mut info = ProjectInfo::default();

        Self::detect_by_file_extensions(workspace_path, &mut info).await?;

        Self::detect_by_config_files(workspace_path, &mut info).await;

        Self::determine_primary_language(&mut info);

        Self::deduplicate_languages(&mut info);

        info!(
            "Project detection complete: languages={:?}, primary={:?}, project_types={:?}",
            info.languages, info.primary_language, info.project_types
        );

        Ok(info)
    }

    /// Detects project type via config files (supports root and subdirectories).
    async fn detect_by_config_files(workspace_path: &Path, info: &mut ProjectInfo) {
        Self::detect_root_config_files(workspace_path, info);

        Self::detect_subdirectory_config_files(workspace_path, info).await;
    }

    /// Detects config files in the workspace root.
    fn detect_root_config_files(workspace_path: &Path, info: &mut ProjectInfo) {
        if workspace_path.join("tsconfig.json").exists() {
            Self::add_language(info, "typescript");
            Self::add_project_type(info, "typescript");
        }

        if workspace_path.join("package.json").exists() {
            if !info.languages.contains(&"typescript".to_string()) {
                Self::add_language(info, "javascript");
            }
            Self::add_project_type(info, "nodejs");
        }

        if workspace_path.join("Cargo.toml").exists() {
            Self::add_language(info, "rust");
            Self::add_project_type(info, "rust");
        }

        if workspace_path.join("pyproject.toml").exists()
            || workspace_path.join("setup.py").exists()
            || workspace_path.join("requirements.txt").exists()
        {
            Self::add_language(info, "python");
            Self::add_project_type(info, "python");
        }

        if workspace_path.join("go.mod").exists() {
            Self::add_language(info, "go");
            Self::add_project_type(info, "go");
        }

        if workspace_path.join("pom.xml").exists()
            || workspace_path.join("build.gradle").exists()
            || workspace_path.join("build.gradle.kts").exists()
        {
            Self::add_language(info, "java");
            Self::add_project_type(info, "java");
        }

        if workspace_path.join("CMakeLists.txt").exists()
            || workspace_path.join("Makefile").exists()
            || workspace_path.join("meson.build").exists()
        {
            Self::add_language(info, "cpp");
            Self::add_project_type(info, "cpp");
        }

        if Self::has_file_with_extension(workspace_path, "csproj")
            || Self::has_file_with_extension(workspace_path, "fsproj")
            || workspace_path.join("global.json").exists()
        {
            Self::add_language(info, "csharp");
            Self::add_project_type(info, "dotnet");
        }
    }

    /// Detects config files in subdirectories (supports monorepo layouts).
    async fn detect_subdirectory_config_files(workspace_path: &Path, info: &mut ProjectInfo) {
        let subdirectories = [
            "cli",
            "src-tauri",
            "crates",
            "rust",
            "backend",
            "core",
            "lib",
            "packages",
            "apps",
            "frontend",
            "web",
            "client",
            "server",
            "api",
            "src",
        ];

        for subdir in subdirectories {
            let subdir_path = workspace_path.join(subdir);
            if !subdir_path.exists() || !subdir_path.is_dir() {
                continue;
            }

            if subdir_path.join("Cargo.toml").exists() {
                Self::add_language(info, "rust");
                Self::add_project_type(info, "rust");
            }

            if subdir_path.join("go.mod").exists() {
                Self::add_language(info, "go");
                Self::add_project_type(info, "go");
            }

            if subdir_path.join("pyproject.toml").exists() || subdir_path.join("setup.py").exists()
            {
                Self::add_language(info, "python");
                Self::add_project_type(info, "python");
            }

            if subdir_path.join("pom.xml").exists()
                || subdir_path.join("build.gradle").exists()
                || subdir_path.join("build.gradle.kts").exists()
            {
                Self::add_language(info, "java");
                Self::add_project_type(info, "java");
            }
        }

        if let Ok(mut entries) = fs::read_dir(workspace_path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                    if matches!(
                        dir_name,
                        "node_modules" | "target" | ".git" | "dist" | "build" | "out"
                    ) {
                        continue;
                    }

                    if path.join("Cargo.toml").exists()
                        && !info.languages.contains(&"rust".to_string())
                    {
                        Self::add_language(info, "rust");
                        Self::add_project_type(info, "rust");
                    }
                }
            }
        }
    }

    /// Checks whether a directory contains any file with the given extension.
    fn has_file_with_extension(dir: &Path, ext: &str) -> bool {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(file_ext) = entry.path().extension() {
                    if file_ext
                        .to_str()
                        .map(|e| e.eq_ignore_ascii_case(ext))
                        .unwrap_or(false)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Detects languages by file extension (deep scan with a file count limit).
    async fn detect_by_file_extensions(
        workspace_path: &Path,
        info: &mut ProjectInfo,
    ) -> Result<()> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        let max_scan_files = 5000;
        let mut scanned = 0;

        Self::scan_directory_iterative(workspace_path, &mut counts, &mut scanned, max_scan_files)
            .await?;

        info.total_files = scanned;

        for (ext, count) in counts {
            let language = Self::extension_to_language(&ext);
            if language != "unknown" {
                *info.file_counts.entry(language.clone()).or_insert(0) += count;

                let threshold = Self::language_threshold(&language);
                if count >= threshold {
                    Self::add_language(info, &language);

                    if count >= 10 {
                        if let Some(project_type) = Self::language_to_project_type(&language) {
                            Self::add_project_type(info, project_type);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Mapping from language to project types.
    fn language_to_project_type(language: &str) -> Option<&'static str> {
        match language {
            "rust" => Some("rust"),
            "python" => Some("python"),
            "go" => Some("go"),
            "java" => Some("java"),
            "kotlin" => Some("kotlin"),
            "typescript" => Some("typescript"),
            "javascript" => Some("nodejs"),
            "cpp" | "c" => Some("cpp"),
            "csharp" => Some("dotnet"),
            "swift" => Some("swift"),
            "ruby" => Some("ruby"),
            "php" => Some("php"),
            "scala" => Some("scala"),
            _ => None,
        }
    }

    /// Returns the file-count threshold for a language.
    fn language_threshold(language: &str) -> usize {
        match language {
            "rust" | "go" | "java" | "python" | "typescript" | "javascript" => 3,
            "json5" | "yaml" | "toml" => 10,
            _ => 5,
        }
    }

    /// Iteratively scans directories (avoids recursion depth limits).
    async fn scan_directory_iterative(
        root: &Path,
        counts: &mut HashMap<String, usize>,
        scanned: &mut usize,
        max_files: usize,
    ) -> Result<()> {
        let mut dir_stack: Vec<PathBuf> = vec![root.to_path_buf()];

        while let Some(dir) = dir_stack.pop() {
            if *scanned >= max_files {
                break;
            }

            let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                dir_name,
                "node_modules"
                    | "target"
                    | ".git"
                    | "dist"
                    | "build"
                    | "out"
                    | "__pycache__"
                    | ".hvigor"
                    | "hvigor"
                    | "vendor"
                    | ".cargo"
                    | ".venv"
                    | "venv"
                    | "env"
                    | "screenshots"
                    | "signature"
            ) {
                continue;
            }

            let mut entries = match fs::read_dir(&dir).await {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            while let Some(entry) = entries.next_entry().await? {
                if *scanned >= max_files {
                    break;
                }

                let path = entry.path();
                let metadata = match entry.metadata().await {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                if metadata.is_dir() {
                    dir_stack.push(path);
                } else if metadata.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        *counts.entry(ext.to_lowercase()).or_insert(0) += 1;
                    }
                    *scanned += 1;
                }
            }
        }

        Ok(())
    }

    /// Determines the primary language based on file counts.
    fn determine_primary_language(info: &mut ProjectInfo) {
        if info.primary_language.is_some() {
            return;
        }

        if info.file_counts.is_empty() {
            return;
        }

        let programming_languages: HashSet<&str> = [
            "rust",
            "python",
            "go",
            "java",
            "javascript",
            "typescript",
            "cpp",
            "c",
            "csharp",
            "kotlin",
            "swift",
            "ruby",
            "php",
            "scala",
        ]
        .into_iter()
        .collect();

        let primary = info
            .file_counts
            .iter()
            .filter(|(lang, _)| programming_languages.contains(lang.as_str()))
            .max_by_key(|(_, count)| *count)
            .map(|(lang, _)| lang.clone());

        if let Some(lang) = primary {
            info.primary_language = Some(lang.clone());
        }
    }

    /// Deduplicates the language list.
    fn deduplicate_languages(info: &mut ProjectInfo) {
        let mut seen = HashSet::new();
        info.languages.retain(|lang| seen.insert(lang.clone()));

        let mut seen = HashSet::new();
        info.project_types.retain(|pt| seen.insert(pt.clone()));
    }

    /// Adds a language (avoids duplicates).
    fn add_language(info: &mut ProjectInfo, language: &str) {
        if !info.languages.contains(&language.to_string()) {
            info.languages.push(language.to_string());
        }
    }

    /// Adds a project type (avoids duplicates).
    fn add_project_type(info: &mut ProjectInfo, project_type: &str) {
        if !info.project_types.contains(&project_type.to_string()) {
            info.project_types.push(project_type.to_string());
        }
    }

    /// Mapping from file extension to language.
    fn extension_to_language(ext: &str) -> String {
        match ext {
            "json5" => "json5",
            "ts" | "tsx" | "ets" => "typescript",
            "js" | "jsx" | "mjs" | "cjs" => "javascript",
            "rs" => "rust",
            "py" | "pyw" => "python",
            "go" => "go",
            "java" => "java",
            "c" | "h" => "c",
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
            "cs" => "csharp",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "kt" | "kts" => "kotlin",
            "scala" => "scala",
            "sh" | "bash" => "shell",
            _ => "unknown",
        }
        .to_string()
    }
}
