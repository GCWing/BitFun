use std::collections::HashMap;

const REMOTE_ENV_BOOTSTRAP: &str = r#"__bitfun_home="${HOME:-}"
if [ -n "$__bitfun_home" ]; then
  if [ -s "$__bitfun_home/.nvm/nvm.sh" ]; then
    . "$__bitfun_home/.nvm/nvm.sh" >/dev/null 2>&1
  fi
  for __bitfun_dir in "$__bitfun_home/.local/bin" "$__bitfun_home/.cargo/bin" "$__bitfun_home/.npm-global/bin"; do
    if [ -d "$__bitfun_dir" ]; then
      PATH="$__bitfun_dir:$PATH"
    fi
  done
fi
export PATH"#;

pub(super) fn remote_user_shell_command(body: &str) -> String {
    format!(
        "bash -lc {}",
        shell_escape(&format!("{REMOTE_ENV_BOOTSTRAP}\n{body}"))
    )
}

pub(super) fn render_remote_env_assignments(env: &HashMap<String, String>) -> Vec<String> {
    let mut entries = env
        .iter()
        .filter(|(key, _)| is_shell_env_key(key))
        .collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));
    entries
        .into_iter()
        .map(|(key, value)| format!("{key}={}", shell_escape(value)))
        .collect()
}

pub(super) fn shell_escape(value: &str) -> String {
    if value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '-' | '_' | ':' | '=' | '@')
    }) {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn is_shell_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_env_assignments_use_valid_keys_in_stable_order() {
        let env = HashMap::from([
            ("INVALID-NAME".to_string(), "ignored".to_string()),
            ("PATH".to_string(), "/remote/bin:/usr/bin".to_string()),
            ("ACP_HOME".to_string(), "/tmp/acp home".to_string()),
        ]);

        assert_eq!(
            render_remote_env_assignments(&env),
            vec![
                "ACP_HOME='/tmp/acp home'".to_string(),
                "PATH=/remote/bin:/usr/bin".to_string(),
            ]
        );
    }

    #[test]
    fn remote_user_shell_command_loads_common_user_toolchains() {
        let command = remote_user_shell_command("command -v codex");

        assert!(command.starts_with("bash -lc "));
        assert!(command.contains(".nvm/nvm.sh"));
        assert!(command.contains(".local/bin"));
        assert!(command.contains("command -v codex"));
    }
}
