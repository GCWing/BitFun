use serde_yaml::Value;
use std::sync::LazyLock;

static FRONT_MATTER_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---").expect("front matter regex pattern is valid")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionPathScope {
    Unscoped,
    Scoped { paths: Vec<String>, body: String },
}

pub(crate) fn parse_front_matter(content: &str) -> Result<(Value, String), String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let captures = FRONT_MATTER_REGEX
        .captures(content)
        .ok_or_else(|| "Failed to capture content".to_string())?;
    let yaml = captures
        .get(1)
        .ok_or_else(|| "Failed to get captures".to_string())?
        .as_str();
    let metadata =
        serde_yaml::from_str(yaml).map_err(|error| format!("Failed to parse YAML: {error}"))?;
    let body_start = captures
        .get(0)
        .ok_or_else(|| "Failed to get captures".to_string())?
        .end();
    Ok((metadata, content[body_start..].trim_start().to_string()))
}

/// Parses a declarative `paths` scope without assigning provider-specific
/// meaning to the Markdown body. Malformed front matter fails closed so a
/// conditional document cannot accidentally become startup context.
pub fn parse_instruction_path_scope(content: &str) -> Result<InstructionPathScope, String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return Ok(InstructionPathScope::Unscoped);
    }

    let (metadata, body) = parse_front_matter(content)?;
    let Some(raw_paths) = metadata.get("paths") else {
        return Ok(InstructionPathScope::Unscoped);
    };
    let Some(raw_paths) = raw_paths.as_sequence() else {
        return Err("paths must be a sequence of non-empty strings".to_string());
    };
    let paths = raw_paths
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or_else(|| "paths must be a sequence of non-empty strings".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if paths.is_empty() {
        return Err("paths must be a sequence of non-empty strings".to_string());
    }

    Ok(InstructionPathScope::Scoped { paths, body })
}

#[cfg(test)]
mod tests {
    use super::{parse_instruction_path_scope, InstructionPathScope};

    #[test]
    fn extracts_ordered_patterns_and_body() {
        let content =
            "---\npaths:\n  - src/**/*.{ts,tsx}\n  - tests/**/*.test.ts\n---\n\n# TypeScript rules\n";

        assert_eq!(
            parse_instruction_path_scope(content).expect("valid path scope"),
            InstructionPathScope::Scoped {
                paths: vec![
                    "src/**/*.{ts,tsx}".to_string(),
                    "tests/**/*.test.ts".to_string(),
                ],
                body: "# TypeScript rules\n".to_string(),
            }
        );
    }

    #[test]
    fn distinguishes_unscoped_and_invalid_metadata() {
        assert_eq!(
            parse_instruction_path_scope("Always applies\n").expect("plain markdown"),
            InstructionPathScope::Unscoped
        );
        assert_eq!(
            parse_instruction_path_scope("---\ntitle: General\n---\nGeneral rules\n")
                .expect("front matter without paths"),
            InstructionPathScope::Unscoped
        );

        let error = parse_instruction_path_scope("---\npaths: src/**/*.rs\n---\nInvalid\n")
            .expect_err("paths must be a sequence");
        assert!(error.contains("sequence of non-empty strings"));
    }

    #[test]
    fn accepts_utf8_bom_and_crlf_before_scoped_front_matter() {
        assert_eq!(
            parse_instruction_path_scope(
                "\u{feff}---\r\npaths:\r\n  - src/**/*.rs\r\n---\r\n\r\nWindows rule\r\n",
            )
            .expect("BOM-prefixed path scope"),
            InstructionPathScope::Scoped {
                paths: vec!["src/**/*.rs".to_string()],
                body: "Windows rule\r\n".to_string(),
            }
        );
    }
}
