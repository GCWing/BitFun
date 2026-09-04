use openbitfun_services_core::local_instructions::{
    local_instruction_path_exists, read_local_instruction_file, LocalInstructionFile,
};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CodexInstructionSourceOptions {
    pub codex_home: Option<PathBuf>,
}

impl CodexInstructionSourceOptions {
    pub fn from_environment() -> Self {
        let codex_home = match std::env::var_os("CODEX_HOME") {
            Some(value) => {
                let path = PathBuf::from(value);
                path.is_absolute().then_some(path)
            }
            None => dirs::home_dir()
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".codex")),
        };
        Self { codex_home }
    }
}

impl Default for CodexInstructionSourceOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub fn load_codex_user_instructions(
    options: &CodexInstructionSourceOptions,
) -> Result<Vec<LocalInstructionFile>, String> {
    let Some(codex_home) = options.codex_home.as_deref() else {
        return Ok(Vec::new());
    };
    for file_name in ["AGENTS.override.md", "AGENTS.md"] {
        let path = codex_home.join(file_name);
        if local_instruction_path_exists(&path)? {
            if let Some(file) =
                read_local_instruction_file(&path, codex_home, format!("$CODEX_HOME/{file_name}"))?
            {
                return Ok(vec![file]);
            }
        }
    }
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::{load_codex_user_instructions, CodexInstructionSourceOptions};

    #[test]
    fn an_empty_override_falls_back_to_the_base_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("AGENTS.override.md"), "").expect("empty override");
        std::fs::write(temp.path().join("AGENTS.md"), "base instructions\n")
            .expect("base instructions");
        let options = CodexInstructionSourceOptions {
            codex_home: Some(temp.path().to_path_buf()),
        };

        let files = load_codex_user_instructions(&options).expect("instructions");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "$CODEX_HOME/AGENTS.md");
        assert_eq!(files[0].content, "base instructions\n");
    }

    #[test]
    fn a_missing_codex_home_does_not_fall_back_to_the_process_directory() {
        let files =
            load_codex_user_instructions(&CodexInstructionSourceOptions { codex_home: None })
                .expect("instructions");

        assert!(files.is_empty());
    }
}
