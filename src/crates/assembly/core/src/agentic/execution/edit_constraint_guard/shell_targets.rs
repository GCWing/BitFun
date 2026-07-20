use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub(super) fn explicit_bash_mutation_targets(command: &str) -> Vec<String> {
    let mut targets = Vec::new();
    // Python `-c` programs commonly contain semicolons inside the quoted
    // program. Scan the complete command before shell-level segmentation so a
    // later `Path(...).write_text()` expression keeps its Python context.
    push_python_mutation_targets(&mut targets, command);
    push_node_mutation_targets(&mut targets, command);
    for segment in command
        .split(['\n', ';', '|'])
        .flat_map(|part| part.split("&&"))
        .flat_map(|part| part.split("||"))
    {
        let words = segment
            .split_whitespace()
            .map(|word| {
                word.trim_matches(|c: char| matches!(c, '\'' | '"' | '(' | ')' | '[' | ']'))
            })
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        if words.is_empty() {
            continue;
        }

        for (index, word) in words.iter().enumerate() {
            let redirection = word.trim_start_matches(|c| matches!(c, '0'..='9'));
            if matches!(redirection, ">" | ">>" | "1>" | "1>>") {
                if let Some(path) = words.get(index + 1) {
                    push_bash_target(&mut targets, path);
                }
            } else if let Some(path) = redirection
                .strip_prefix(">>")
                .or_else(|| redirection.strip_prefix('>'))
            {
                if !path.is_empty() {
                    push_bash_target(&mut targets, path);
                }
            }
        }

        let Some(command_index) = words.iter().position(|word| !word.contains('=')) else {
            continue;
        };
        let command_name = Path::new(words[command_index])
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(words[command_index])
            .to_ascii_lowercase();
        let arguments = &words[command_index + 1..];
        match command_name.as_str() {
            "tee" => {
                for argument in arguments
                    .iter()
                    .filter(|argument| !argument.starts_with('-'))
                {
                    push_bash_target(&mut targets, argument);
                }
            }
            "cp" | "install" => {
                if let Some(path) = arguments
                    .iter()
                    .rev()
                    .find(|argument| !argument.starts_with('-'))
                {
                    push_bash_target(&mut targets, path);
                }
            }
            "mv" => {
                // Moving a protected source removes it from its original
                // location, so both sides are mutation targets. The previous
                // destination-only handling let `mv tests/a.rs src/a.rs`
                // bypass a test-file constraint.
                for argument in arguments
                    .iter()
                    .filter(|argument| !argument.starts_with('-'))
                {
                    push_bash_target(&mut targets, argument);
                }
            }
            "touch" | "truncate" | "rm" | "rmdir" | "unlink" => {
                for argument in arguments
                    .iter()
                    .filter(|argument| !argument.starts_with('-'))
                {
                    push_bash_target(&mut targets, argument);
                }
            }
            "sed" | "perl" => {
                if arguments
                    .iter()
                    .any(|argument| *argument == "-i" || argument.starts_with("-i"))
                {
                    let mut script_seen = false;
                    for argument in arguments
                        .iter()
                        .filter(|argument| !argument.starts_with('-'))
                    {
                        if !script_seen {
                            script_seen = true;
                            continue;
                        }
                        if argument.starts_with('/')
                            || argument.starts_with("./")
                            || argument.starts_with("../")
                            || argument.contains('.')
                            || argument.starts_with("test/")
                            || argument.starts_with("tests/")
                        {
                            push_bash_target(&mut targets, argument);
                        }
                    }
                }
            }
            "git" => push_git_mutation_targets(&mut targets, arguments),
            _ => {}
        }
    }
    targets
}

fn push_git_mutation_targets(targets: &mut Vec<String>, arguments: &[&str]) {
    let Some((subcommand_index, subcommand)) = arguments
        .iter()
        .enumerate()
        .find(|(_, argument)| !argument.starts_with('-'))
    else {
        return;
    };
    let remaining = &arguments[subcommand_index + 1..];
    match *subcommand {
        "mv" | "rm" | "restore" => {
            for argument in remaining
                .iter()
                .filter(|argument| !argument.starts_with('-'))
            {
                push_bash_target(targets, argument);
            }
        }
        "checkout" => {
            // `git checkout <ref>` is not a path mutation by itself. The
            // pathspec form is unambiguous only after `--`.
            if let Some(separator) = remaining.iter().position(|argument| *argument == "--") {
                for argument in remaining[separator + 1..]
                    .iter()
                    .filter(|argument| !argument.starts_with('-'))
                {
                    push_bash_target(targets, argument);
                }
            }
        }
        _ => {}
    }
}

fn push_python_mutation_targets(targets: &mut Vec<String>, segment: &str) {
    static OPEN_FOR_WRITE: OnceLock<Regex> = OnceLock::new();
    static PATH_MUTATION: OnceLock<Regex> = OnceLock::new();
    let open_for_write = OPEN_FOR_WRITE.get_or_init(|| {
        Regex::new(r#"(?i)\bopen\s*\(\s*["']([^"']+)["']\s*,\s*["'][wax][^"']*["']"#)
            .expect("valid Python open-for-write regex")
    });
    let path_mutation = PATH_MUTATION.get_or_init(|| {
        Regex::new(
            r#"(?i)\bPath\s*\(\s*["']([^"']+)["']\s*\)\s*\.\s*(?:write_text|write_bytes|unlink|rename|replace)\s*\("#,
        )
        .expect("valid pathlib mutation regex")
    });

    for captures in open_for_write
        .captures_iter(segment)
        .chain(path_mutation.captures_iter(segment))
    {
        if let Some(path) = captures.get(1) {
            push_bash_target(targets, path.as_str());
        }
    }
}

fn push_node_mutation_targets(targets: &mut Vec<String>, segment: &str) {
    static SINGLE_PATH_MUTATION: OnceLock<Regex> = OnceLock::new();
    static TWO_PATH_MUTATION: OnceLock<Regex> = OnceLock::new();
    let single_path_mutation = SINGLE_PATH_MUTATION.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(?:fs\s*\.\s*)?(?:writefilesync|appendfilesync|unlinksync|rmsync)\s*\(\s*["']([^"']+)["']"#,
        )
        .expect("valid Node single-path mutation regex")
    });
    let two_path_mutation = TWO_PATH_MUTATION.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(?:fs\s*\.\s*)?(?:renamesync|copyfilesync)\s*\(\s*["']([^"']+)["']\s*,\s*["']([^"']+)["']"#,
        )
        .expect("valid Node two-path mutation regex")
    });

    for captures in single_path_mutation.captures_iter(segment) {
        if let Some(path) = captures.get(1) {
            push_bash_target(targets, path.as_str());
        }
    }
    for captures in two_path_mutation.captures_iter(segment) {
        for index in [1, 2] {
            if let Some(path) = captures.get(index) {
                push_bash_target(targets, path.as_str());
            }
        }
    }
}

fn push_bash_target(targets: &mut Vec<String>, raw_path: &str) {
    let path = raw_path.trim_matches(|c: char| matches!(c, '\'' | '"' | ','));
    if !path.is_empty() && !targets.iter().any(|existing| existing == path) {
        targets.push(path.to_string());
    }
}
