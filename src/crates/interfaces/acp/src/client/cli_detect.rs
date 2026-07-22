/// Check whether a command exists on the system PATH.
/// Uses `which::which()` — already a workspace dependency.
/// Returns `Some(path)` if found, `None` otherwise.
pub(crate) async fn detect_cli(command: &str) -> Option<String> {
    which::which(command)
        .ok()
        .map(|path| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn detects_existing_command() {
        let cmd = if cfg!(windows) { "cmd.exe" } else { "sh" };
        let result = detect_cli(cmd).await;
        assert!(result.is_some(), "expected {} to be found on PATH", cmd);
    }

    #[tokio::test]
    async fn returns_none_for_missing_command() {
        let result = detect_cli("bitfun-definitely-does-not-exist-xyz-12345").await;
        assert!(result.is_none());
    }
}
