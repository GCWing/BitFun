use serde_yaml::Value;
use std::sync::LazyLock;

/// Compiled once; front-matter parsing runs on every `.md` scan.
static FRONT_MATTER_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---").expect("front matter regex pattern is valid")
});

static PROMPT_ARGUMENT_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?:\[Image\s+\d+\]|"[^"]*"|'[^']*'|[^\s"']+)"#)
        .expect("prompt argument regex pattern is valid")
});

/// Returns a conservative upper bound for prompt argument expansion without
/// allocating the expanded string. Each `$` may begin at most one placeholder;
/// the fallback arguments section is included even when a placeholder exists.
pub fn prompt_template_expansion_upper_bound(template: &str, arguments: &str) -> Option<usize> {
    let placeholder_bound = template
        .bytes()
        .filter(|byte| *byte == b'$')
        .count()
        .checked_mul(arguments.len())?;
    template
        .len()
        .checked_add(placeholder_bound)?
        .checked_add(arguments.len())?
        .checked_add("\n\nARGUMENTS: ".len())
}

/// Expands Claude-compatible prompt arguments without executing dynamic content.
pub fn expand_prompt_template_arguments(template: &str, arguments: &str) -> String {
    expand_prompt_template_arguments_with_names(template, arguments, &[])
}

/// Expands positional and explicitly declared named prompt arguments without
/// executing dynamic content.
pub fn expand_prompt_template_arguments_with_names(
    template: &str,
    arguments: &str,
    argument_names: &[String],
) -> String {
    let arguments_by_position = PROMPT_ARGUMENT_REGEX
        .find_iter(arguments)
        .map(|item| {
            let value = item.as_str();
            if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                value[1..value.len() - 1].to_string()
            } else {
                value.to_string()
            }
        })
        .collect::<Vec<_>>();

    let mut expanded = String::with_capacity(template.len() + arguments.len());
    let mut cursor = 0;
    let mut used_placeholder = false;
    while cursor < template.len() {
        let remaining = &template[cursor..];
        if remaining.starts_with(r"\\$") {
            expanded.push_str(r"\\");
            cursor += 2;
            continue;
        }
        if remaining.starts_with(r"\$") {
            if let Some(length) = prompt_placeholder_length(&remaining[1..], argument_names) {
                expanded.push_str(&remaining[1..length + 1]);
                cursor += length + 1;
            } else {
                expanded.push('\\');
                cursor += 1;
            }
            continue;
        }
        if let Some((length, position)) = positional_placeholder(remaining) {
            used_placeholder = true;
            if let Some(argument) = position.and_then(|index| arguments_by_position.get(index)) {
                expanded.push_str(argument);
            } else {
                expanded.push_str(&remaining[..length]);
            }
            cursor += length;
            continue;
        }
        if let Some(length) = full_arguments_placeholder_length(remaining) {
            used_placeholder = true;
            expanded.push_str(arguments);
            cursor += length;
            continue;
        }
        if let Some((length, position)) = named_placeholder(remaining, argument_names) {
            used_placeholder = true;
            if let Some(argument) = arguments_by_position.get(position) {
                expanded.push_str(argument);
            }
            cursor += length;
            continue;
        }

        let character = remaining
            .chars()
            .next()
            .expect("cursor is inside the template");
        expanded.push(character);
        cursor += character.len_utf8();
    }

    if !used_placeholder && !arguments.trim().is_empty() {
        expanded.push_str("\n\nARGUMENTS: ");
        expanded.push_str(arguments);
    }
    expanded.trim().to_string()
}

fn prompt_placeholder_length(value: &str, argument_names: &[String]) -> Option<usize> {
    positional_placeholder(value)
        .map(|(length, _)| length)
        .or_else(|| full_arguments_placeholder_length(value))
        .or_else(|| named_placeholder(value, argument_names).map(|(length, _)| length))
}

fn full_arguments_placeholder_length(value: &str) -> Option<usize> {
    let remaining = value.strip_prefix("$ARGUMENTS")?;
    (!remaining.starts_with('[')).then_some("$ARGUMENTS".len())
}

fn positional_placeholder(value: &str) -> Option<(usize, Option<usize>)> {
    if let Some(indexed) = value.strip_prefix("$ARGUMENTS[") {
        let closing_bracket = indexed.find(']')?;
        let index = &indexed[..closing_bracket];
        if !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit()) {
            return Some((
                "$ARGUMENTS[".len() + closing_bracket + 1,
                index.parse::<usize>().ok(),
            ));
        }
    }

    let indexed = value.strip_prefix('$')?;
    let length = indexed
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if length == 0 {
        return None;
    }
    Some((length + 1, indexed[..length].parse::<usize>().ok()))
}

fn named_placeholder(value: &str, argument_names: &[String]) -> Option<(usize, usize)> {
    let value = value.strip_prefix('$')?;
    argument_names
        .iter()
        .enumerate()
        .filter(|(_, name)| value.starts_with(name.as_str()))
        .filter(|(_, name)| {
            value[name.len()..]
                .bytes()
                .next()
                .is_none_or(|byte| !is_argument_name_byte(byte))
        })
        .max_by_key(|(_, name)| name.len())
        .map(|(position, name)| (name.len() + 1, position))
}

fn is_argument_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

/// Parses and writes Markdown files with YAML front matter.
pub struct FrontMatterMarkdown;

impl FrontMatterMarkdown {
    pub fn load(path: &str) -> Result<(Value, String), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read markdown file: {}", e))?;
        Self::load_str(&content).map_err(|e| format!("Failed to parse markdown file: {}", e))
    }

    pub fn load_str(content: &str) -> Result<(Value, String), String> {
        let caps = FRONT_MATTER_REGEX
            .captures(content)
            .ok_or_else(|| "Failed to capture content".to_string())?;

        let yaml_content = caps
            .get(1)
            .ok_or_else(|| "Failed to get captures".to_string())?
            .as_str();

        let metadata: Value = serde_yaml::from_str(yaml_content)
            .map_err(|e| format!("Failed to parse YAML: {}", e))?;

        let after_front_matter = caps
            .get(0)
            .ok_or_else(|| "Failed to get captures".to_string())?
            .end();
        let markdown_body = content[after_front_matter..].trim_start();

        Ok((metadata, markdown_body.to_string()))
    }

    pub fn save(path: &str, metadata: &Value, body: &str) -> Result<(), String> {
        let yaml_str = serde_yaml::to_string(metadata)
            .map_err(|e| format!("Failed to serialize YAML: {}", e))?;
        let content = format!("---\n{}\n---\n\n{}", yaml_str.trim_end(), body.trim_start());
        std::fs::write(path, content).map_err(|e| format!("Failed to write markdown file: {}", e))
    }
}
