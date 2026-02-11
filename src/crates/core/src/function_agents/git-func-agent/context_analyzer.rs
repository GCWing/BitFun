use super::types::*;
/**
 * Context analyzer
 *
 * Provides project context for AI to better understand code changes
 */
use log::debug;
use std::fs;
use std::path::Path;

pub struct ContextAnalyzer;

impl ContextAnalyzer {
    pub async fn analyze_project_context(repo_path: &Path) -> AgentResult<ProjectContext> {
        debug!("Analyzing project context: repo_path={:?}", repo_path);

        let project_type = Self::detect_project_type(repo_path)?;

        let tech_stack = Self::detect_tech_stack(repo_path)?;

        let project_docs = Self::read_project_docs(repo_path);

        let code_standards = Self::detect_code_standards(repo_path);

        Ok(ProjectContext {
            project_type,
            tech_stack,
            project_docs,
            code_standards,
        })
    }

    fn detect_project_type(repo_path: &Path) -> AgentResult<String> {
        if repo_path.join("Cargo.toml").exists() {
            if repo_path.join("src-tauri").exists() {
                return Ok("tauri-app".to_string());
            }

            if let Ok(content) = fs::read_to_string(repo_path.join("Cargo.toml")) {
                if content.contains("[lib]") {
                    return Ok("rust-library".to_string());
                }
            }

            return Ok("rust-application".to_string());
        }

        if repo_path.join("package.json").exists() {
            if let Ok(content) = fs::read_to_string(repo_path.join("package.json")) {
                if content.contains("\"react\"") {
                    return Ok("react-app".to_string());
                } else if content.contains("\"vue\"") {
                    return Ok("vue-app".to_string());
                } else if content.contains("\"next\"") {
                    return Ok("nextjs-app".to_string());
                } else if content.contains("\"express\"") {
                    return Ok("nodejs-backend".to_string());
                }
            }
            return Ok("nodejs-app".to_string());
        }

        if repo_path.join("go.mod").exists() {
            return Ok("go-application".to_string());
        }

        if repo_path.join("requirements.txt").exists() || repo_path.join("pyproject.toml").exists()
        {
            return Ok("python-application".to_string());
        }

        if repo_path.join("pom.xml").exists() {
            return Ok("java-maven-app".to_string());
        }

        if repo_path.join("build.gradle").exists() {
            return Ok("java-gradle-app".to_string());
        }

        Ok("unknown".to_string())
    }

    fn detect_tech_stack(repo_path: &Path) -> AgentResult<Vec<String>> {
        let mut stack = Vec::new();

        if repo_path.join("Cargo.toml").exists() {
            stack.push("Rust".to_string());

            if let Ok(content) = fs::read_to_string(repo_path.join("Cargo.toml")) {
                if content.contains("tokio") {
                    stack.push("Tokio".to_string());
                }
                if content.contains("axum") {
                    stack.push("Axum".to_string());
                }
                if content.contains("actix-web") {
                    stack.push("Actix-Web".to_string());
                }
                if content.contains("tauri") {
                    stack.push("Tauri".to_string());
                }
            }
        }

        if repo_path.join("package.json").exists() {
            if let Ok(content) = fs::read_to_string(repo_path.join("package.json")) {
                if content.contains("\"typescript\"") {
                    stack.push("TypeScript".to_string());
                } else {
                    stack.push("JavaScript".to_string());
                }

                if content.contains("\"react\"") {
                    stack.push("React".to_string());
                }
                if content.contains("\"vue\"") {
                    stack.push("Vue".to_string());
                }
                if content.contains("\"next\"") {
                    stack.push("Next.js".to_string());
                }
                if content.contains("\"vite\"") {
                    stack.push("Vite".to_string());
                }
            }
        }

        if repo_path.join("go.mod").exists() {
            stack.push("Go".to_string());
        }

        if repo_path.join("requirements.txt").exists() || repo_path.join("pyproject.toml").exists()
        {
            stack.push("Python".to_string());
        }

        if repo_path.join("pom.xml").exists() || repo_path.join("build.gradle").exists() {
            stack.push("Java".to_string());
        }

        if let Ok(entries) = fs::read_dir(repo_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.contains("postgres") || name.contains("pg") {
                        stack.push("PostgreSQL".to_string());
                    }
                    if name.contains("mysql") {
                        stack.push("MySQL".to_string());
                    }
                    if name.contains("mongo") {
                        stack.push("MongoDB".to_string());
                    }
                    if name.contains("redis") {
                        stack.push("Redis".to_string());
                    }
                }
            }
        }

        if stack.is_empty() {
            stack.push("Unknown".to_string());
        }

        Ok(stack)
    }

    fn read_project_docs(repo_path: &Path) -> Option<String> {
        let readme_paths = ["README.md", "README", "README.txt", "readme.md"];

        for readme_name in &readme_paths {
            let readme_path = repo_path.join(readme_name);
            if readme_path.exists() {
                if let Ok(content) = fs::read_to_string(&readme_path) {
                    let summary = content.chars().take(1000).collect::<String>();
                    return Some(summary);
                }
            }
        }

        None
    }

    fn detect_code_standards(repo_path: &Path) -> Option<String> {
        let mut standards = Vec::new();

        if repo_path.join("rustfmt.toml").exists() || repo_path.join(".rustfmt.toml").exists() {
            standards.push("rustfmt");
        }
        if repo_path.join("clippy.toml").exists() {
            standards.push("clippy");
        }

        if repo_path.join(".eslintrc.js").exists()
            || repo_path.join(".eslintrc.json").exists()
            || repo_path.join("eslint.config.js").exists()
        {
            standards.push("ESLint");
        }
        if repo_path.join(".prettierrc").exists() || repo_path.join("prettier.config.js").exists() {
            standards.push("Prettier");
        }

        if repo_path.join(".flake8").exists() {
            standards.push("flake8");
        }
        if repo_path.join(".pylintrc").exists() {
            standards.push("pylint");
        }

        if repo_path.join(".editorconfig").exists() {
            standards.push("EditorConfig");
        }

        if standards.is_empty() {
            None
        } else {
            Some(standards.join(", "))
        }
    }
}
