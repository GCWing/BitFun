//! Bot integration for Remote Connect.
//!
//! Supports Feishu, Telegram, and Weixin (iLink) bots as relay channels.
//! Shared command logic lives in `command_router`; platform-specific
//! I/O is handled by `telegram`, `feishu`, and `weixin`.

pub mod command_router;
pub mod feishu;
pub mod locale;
pub mod menu;
pub mod telegram;
pub mod weixin;

use serde::{Deserialize, Serialize};

pub use command_router::{BotChatState, ForwardRequest, ForwardedTurnResult, HandleResult};
pub use locale::BotLanguage;
pub use menu::{MenuItem, MenuItemStyle, MenuView};

/// Configuration for a bot-based connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "bot_type", rename_all = "snake_case")]
pub enum BotConfig {
    Feishu {
        app_id: String,
        app_secret: String,
    },
    Telegram {
        bot_token: String,
    },
    Weixin {
        ilink_token: String,
        base_url: String,
        bot_account_id: String,
    },
}

/// Pairing state for bot-based connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotPairingInfo {
    pub pairing_code: String,
    pub bot_type: String,
    pub bot_link: String,
    pub expires_at: i64,
}

/// Persisted bot connection — saved to disk so reconnect survives restarts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedBotConnection {
    pub bot_type: String,
    pub chat_id: String,
    pub config: BotConfig,
    pub chat_state: BotChatState,
    pub connected_at: i64,
}

/// Persisted remote-connect form values shown in the desktop dialog.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteConnectFormState {
    pub custom_server_url: String,
    pub telegram_bot_token: String,
    pub feishu_app_id: String,
    pub feishu_app_secret: String,
    /// Weixin iLink credentials after QR login (optional until user links WeChat).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub weixin_ilink_token: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub weixin_base_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub weixin_bot_account_id: String,
}

/// All persisted bot connections (one per bot type at most).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BotPersistenceData {
    #[serde(default)]
    pub connections: Vec<SavedBotConnection>,
    #[serde(default)]
    pub form_state: RemoteConnectFormState,
    /// Global verbose mode setting for all bot connections.
    /// When true, the agent's intermediate thinking summaries (one short
    /// `[Thinking] …` line per `ThinkingEnd`) are forwarded to the user.
    /// Tool-call notifications are intentionally NOT sent even in verbose
    /// mode — they were too noisy for IM channels (especially WeChat where
    /// each line costs a `context_token` slot) without giving the user
    /// information they could act on.
    /// Defaults to `false` (concise mode).
    #[serde(default)]
    pub verbose_mode: bool,
}

impl BotPersistenceData {
    pub fn upsert(&mut self, conn: SavedBotConnection) {
        self.connections.retain(|c| c.bot_type != conn.bot_type);
        self.connections.push(conn);
    }

    pub fn remove(&mut self, bot_type: &str) {
        self.connections.retain(|c| c.bot_type != bot_type);
    }

    pub fn get(&self, bot_type: &str) -> Option<&SavedBotConnection> {
        self.connections.iter().find(|c| c.bot_type == bot_type)
    }
}

// ── Shared workspace-file utilities ────────────────────────────────

/// File content read from the local workspace, ready to be sent over any channel.
pub struct WorkspaceFileContent {
    pub name: String,
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
    pub size: u64,
}

fn strip_workspace_path_prefix(raw: &str) -> &str {
    raw.strip_prefix("computer://")
        .or_else(|| raw.strip_prefix("file://"))
        .unwrap_or(raw)
}

fn is_absolute_workspace_path(path: &str) -> bool {
    path.starts_with('/') || (path.len() >= 3 && path.as_bytes()[1] == b':')
}

/// Resolve a raw path (with or without `computer://` / `file://` prefix) to an
/// absolute `PathBuf`.
///
/// Absolute paths are passed through directly. Relative paths are resolved
/// against `workspace_root` when provided, and paths escaping that root are
/// rejected.
pub fn resolve_workspace_path(
    raw: &str,
    workspace_root: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    let stripped = strip_workspace_path_prefix(raw);

    if is_absolute_workspace_path(stripped) {
        return Some(std::path::PathBuf::from(stripped));
    }

    let workspace_root = workspace_root?;
    let canonical_root = std::fs::canonicalize(workspace_root).ok()?;
    let candidate = canonical_root.join(stripped);
    let canonical_candidate = std::fs::canonicalize(candidate).ok()?;

    if canonical_candidate.starts_with(&canonical_root) {
        Some(canonical_candidate)
    } else {
        None
    }
}

/// Return the best-effort MIME type for a file based on its extension.
pub fn detect_mime_type(path: &std::path::Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "ts" | "tsx" | "jsx" | "rs" | "py" | "go" | "java" | "c" | "cpp" | "h" | "sh" | "toml"
        | "yaml" | "yml" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "zip" => "application/zip",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp4" => "video/mp4",
        "opus" => "audio/opus",
        _ => "application/octet-stream",
    }
}

/// Read a workspace file, resolving `computer://` prefixes.
///
/// `max_size` is the caller-specific byte limit (e.g. 50 MB for Telegram,
/// 30 MB for Feishu, 10 MB for mobile relay).
///
/// Returns an error when the file is missing, is a directory, or exceeds
/// `max_size`.
pub async fn read_workspace_file(
    raw_path: &str,
    max_size: u64,
    workspace_root: Option<&std::path::Path>,
) -> anyhow::Result<WorkspaceFileContent> {
    let abs_path = resolve_workspace_path(raw_path, workspace_root)
        .ok_or_else(|| anyhow::anyhow!("Remote file path could not be resolved: {raw_path}"))?;

    if !abs_path.exists() {
        return Err(anyhow::anyhow!("File not found: {}", abs_path.display()));
    }
    if !abs_path.is_file() {
        return Err(anyhow::anyhow!(
            "Path is not a regular file: {}",
            abs_path.display()
        ));
    }

    let metadata = tokio::fs::metadata(&abs_path).await.map_err(|e| {
        anyhow::anyhow!("Cannot read file metadata for {}: {e}", abs_path.display())
    })?;

    if metadata.len() > max_size {
        return Err(anyhow::anyhow!(
            "File too large ({} bytes, limit {max_size} bytes): {}",
            metadata.len(),
            abs_path.display()
        ));
    }

    let bytes = tokio::fs::read(&abs_path)
        .await
        .map_err(|e| anyhow::anyhow!("Cannot read file {}: {e}", abs_path.display()))?;

    let name = abs_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    let mime_type = detect_mime_type(&abs_path);

    Ok(WorkspaceFileContent {
        name,
        bytes,
        mime_type,
        size: metadata.len(),
    })
}

/// Get file metadata (name and size in bytes) without reading the full content.
/// Returns `None` if the path cannot be resolved, does not exist, or is not a
/// regular file.
pub fn get_file_metadata(
    raw_path: &str,
    workspace_root: Option<&std::path::Path>,
) -> Option<(String, u64)> {
    let abs = resolve_workspace_path(raw_path, workspace_root)?;
    if !abs.is_file() {
        return None;
    }
    let name = abs
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let size = std::fs::metadata(&abs).ok()?.len();
    Some((name, size))
}

/// Format a byte count as a human-readable string (e.g. "1.4 MB", "320 KB").
pub fn format_file_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{bytes} B")
    }
}

// ── Downloadable file link extraction ──────────────────────────────

/// Extensions that are source-code / config files — excluded from download
/// when referenced via absolute paths (matches mobile-web `CODE_FILE_EXTENSIONS`).
const CODE_FILE_EXTENSIONS: &[&str] = &[
    "js",
    "jsx",
    "ts",
    "tsx",
    "mjs",
    "cjs",
    "mts",
    "cts",
    "py",
    "pyw",
    "pyi",
    "rs",
    "go",
    "java",
    "kt",
    "kts",
    "scala",
    "groovy",
    "c",
    "cpp",
    "cc",
    "cxx",
    "h",
    "hpp",
    "hxx",
    "hh",
    "cs",
    "rb",
    "php",
    "swift",
    "vue",
    "svelte",
    "css",
    "scss",
    "less",
    "sass",
    "json",
    "jsonc",
    "yaml",
    "yml",
    "toml",
    "xml",
    "md",
    "mdx",
    "rst",
    "txt",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "bat",
    "cmd",
    "sql",
    "graphql",
    "gql",
    "proto",
    "lock",
    "env",
    "ini",
    "cfg",
    "conf",
    "cj",
    "ets",
    "editorconfig",
    "gitignore",
    "log",
];

/// Extensions that should be treated as downloadable when referenced via
/// relative markdown links (matches mobile-web `DOWNLOADABLE_EXTENSIONS`).
const DOWNLOADABLE_EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "pages",
    "numbers", "key", "png", "jpg", "jpeg", "gif", "bmp", "svg", "webp", "ico", "tiff", "tif",
    "zip", "tar", "gz", "bz2", "7z", "rar", "dmg", "iso", "xz", "mp3", "wav", "ogg", "flac", "aac",
    "m4a", "wma", "mp4", "avi", "mkv", "mov", "webm", "wmv", "flv", "csv", "tsv", "sqlite", "db",
    "parquet", "epub", "mobi", "apk", "ipa", "exe", "msi", "deb", "rpm", "ttf", "otf", "woff",
    "woff2",
];

/// Check whether a bare file path (no protocol prefix) should be treated as
/// a downloadable file based on its extension.
///
/// Absolute local file paths exclude source/config files. Relative links
/// are allowed when they point to known downloadable file types.
fn is_downloadable_by_extension(file_path: &str) -> bool {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext.is_empty() {
        return false;
    }
    let is_absolute = file_path.starts_with('/')
        || (file_path.len() >= 3 && file_path.as_bytes().get(1) == Some(&b':'));
    if is_absolute {
        !CODE_FILE_EXTENSIONS.contains(&ext.as_str())
    } else {
        DOWNLOADABLE_EXTENSIONS.contains(&ext.as_str())
    }
}

/// Only file paths that can be resolved to existing files are returned.
/// Directories and missing paths are skipped. Duplicate paths are deduplicated
/// before returning.
pub fn extract_computer_file_paths(
    text: &str,
    workspace_root: Option<&std::path::Path>,
) -> Vec<String> {
    const PREFIX: &str = "computer://";
    let mut paths: Vec<String> = Vec::new();
    let mut search = text;

    while let Some(idx) = search.find(PREFIX) {
        let rest = &search[idx + PREFIX.len()..];
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '(' | ')' | '"' | '\''))
            .unwrap_or(rest.len());
        let raw_suffix = rest[..end].trim_end_matches(['.', ',', ';', ':', ')', ']']);
        if !raw_suffix.is_empty() {
            push_if_existing_file(&format!("{PREFIX}{raw_suffix}"), &mut paths, workspace_root);
        }
        search = &rest[end..];
    }

    paths
}

/// Try to resolve `file_path` and, if it exists as a regular file, push
/// its absolute path into `out` (deduplicating).
fn push_if_existing_file(
    file_path: &str,
    out: &mut Vec<String>,
    workspace_root: Option<&std::path::Path>,
) {
    if let Some(abs) = resolve_workspace_path(file_path, workspace_root) {
        let abs_str = abs.to_string_lossy().into_owned();
        if abs.exists() && abs.is_file() && !out.contains(&abs_str) {
            out.push(abs_str);
        }
    }
}

/// Extract all downloadable file paths from agent response markdown text.
///
/// Detects three kinds of references:
/// 1. `computer://` links in plain text.
/// 2. `file://` links in plain text.
/// 3. Markdown hyperlinks `[text](href)` pointing to absolute local files
///    (excluding code/config source files).
///
/// Only paths that exist as regular files on disk are returned.
/// Duplicate paths are deduplicated.
pub fn extract_downloadable_file_paths(
    text: &str,
    workspace_root: Option<&std::path::Path>,
) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();

    // Phase 1 — protocol-prefixed links (`computer://` and `file://`).
    for prefix in ["computer://", "file://"] {
        let mut search = text;
        while let Some(idx) = search.find(prefix) {
            let rest = &search[idx + prefix.len()..];
            let end = rest
                .find(|c: char| {
                    c.is_whitespace() || matches!(c, '<' | '>' | '(' | ')' | '"' | '\'')
                })
                .unwrap_or(rest.len());
            let raw_suffix = rest[..end].trim_end_matches(['.', ',', ';', ':', ')', ']']);
            if !raw_suffix.is_empty() {
                let resolve_input = if prefix == "computer://" {
                    format!("{prefix}{raw_suffix}")
                } else {
                    raw_suffix.to_string()
                };
                push_if_existing_file(&resolve_input, &mut paths, workspace_root);
            }
            search = &rest[end..];
        }
    }

    // Phase 2 — markdown hyperlinks `[text](href)` referencing local files.
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i + 2 < len {
        if bytes[i] == b']' && bytes[i + 1] == b'(' {
            let href_start = i + 2;
            if let Some(rel_end) = text[href_start..].find(')') {
                let href = text[href_start..href_start + rel_end].trim();
                // Skip protocols already handled above and non-local URLs.
                if !href.is_empty()
                    && !href.starts_with("computer://")
                    && !href.starts_with("file://")
                    && !href.starts_with("http://")
                    && !href.starts_with("https://")
                    && !href.starts_with("mailto:")
                    && !href.starts_with("tel:")
                    && !href.starts_with('#')
                    && !href.starts_with("//")
                    && is_downloadable_by_extension(href)
                {
                    push_if_existing_file(href, &mut paths, workspace_root);
                }
                i = href_start + rel_end + 1;
            } else {
                i += 2;
            }
        } else {
            i += 1;
        }
    }

    paths
}

// ── Auto-push file delivery helpers ───────────────────────────────

/// One file to be auto-pushed to the IM peer alongside an agent reply.
#[derive(Debug, Clone)]
pub struct AutoPushFile {
    /// Absolute path on the desktop (already resolved).
    pub abs_path: String,
    /// User-visible filename (basename of `abs_path`).
    pub name: String,
    /// Plaintext file size in bytes (for size-limit checks and UI).
    pub size: u64,
}

/// Scan an agent reply for downloadable file references and resolve their
/// metadata so each platform adapter can push them directly to the user
/// without an intermediate "tap to download" prompt.
pub fn collect_auto_push_files(
    text: &str,
    workspace_root: Option<&std::path::Path>,
) -> Vec<AutoPushFile> {
    extract_downloadable_file_paths(text, workspace_root)
        .into_iter()
        .filter_map(|path| {
            get_file_metadata(&path, workspace_root).map(|(name, size)| AutoPushFile {
                abs_path: path,
                name,
                size,
            })
        })
        .collect()
}

/// Caption sent once before the first auto-pushed file.
pub fn auto_push_intro(language: BotLanguage, count: usize) -> String {
    let strings = locale::strings_for(language);
    if count <= 1 {
        strings.auto_push_intro_one.to_string()
    } else {
        locale::fmt_count(strings.auto_push_intro_many_fmt, count)
    }
}

/// Notice sent when a single file exceeds the platform's size limit and is skipped.
pub fn auto_push_skip_too_large_message(
    language: BotLanguage,
    file_name: &str,
    size: u64,
    limit: u64,
) -> String {
    let strings = locale::strings_for(language);
    strings
        .auto_push_skip_too_large_fmt
        .replace("{name}", file_name)
        .replace("{size}", &format_file_size(size))
        .replace("{limit}", &format_file_size(limit))
}

/// Notice sent when an upload/send call fails for a single file.
pub fn auto_push_failed_message(language: BotLanguage, file_name: &str, err: &str) -> String {
    let strings = locale::strings_for(language);
    strings
        .auto_push_failed_fmt
        .replace("{name}", file_name)
        .replace("{err}", err)
}

const REMOTE_CONNECT_PERSISTENCE_FILENAME: &str = "remote_connect_persistence.json";
const LEGACY_BOT_PERSISTENCE_FILENAME: &str = "bot_connections.json";

pub fn bot_persistence_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| {
        home.join(".bitfun")
            .join(REMOTE_CONNECT_PERSISTENCE_FILENAME)
    })
}

fn legacy_bot_persistence_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".bitfun").join(LEGACY_BOT_PERSISTENCE_FILENAME))
}

pub fn load_bot_persistence() -> BotPersistenceData {
    let Some(path) = bot_persistence_path() else {
        return BotPersistenceData::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => {
            let Some(legacy_path) = legacy_bot_persistence_path() else {
                return BotPersistenceData::default();
            };
            match std::fs::read_to_string(&legacy_path) {
                Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
                Err(_) => BotPersistenceData::default(),
            }
        }
    }
}

pub fn save_bot_persistence(data: &BotPersistenceData) {
    let Some(path) = bot_persistence_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(data) {
        if let Err(e) = std::fs::write(&path, json) {
            log::error!("Failed to save bot persistence: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_auto_push_files, extract_downloadable_file_paths, resolve_workspace_path};

    fn make_temp_workspace() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "bitfun-remote-connect-test-{}",
            uuid::Uuid::new_v4()
        ));
        let workspace = base.join("workspace");
        let artifacts = workspace.join("artifacts");
        let report = artifacts.join("report.pptx");
        std::fs::create_dir_all(&artifacts).unwrap();
        std::fs::write(&report, b"ppt").unwrap();
        (base, workspace, report)
    }

    #[test]
    fn resolves_relative_paths_within_workspace_root() {
        let (base, workspace, report) = make_temp_workspace();

        let resolved =
            resolve_workspace_path("computer://artifacts/report.pptx", Some(&workspace)).unwrap();

        assert_eq!(resolved, std::fs::canonicalize(report).unwrap());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn rejects_relative_paths_that_escape_workspace_root() {
        let (base, workspace, _report) = make_temp_workspace();
        let secret = base.join("secret.txt");
        std::fs::write(&secret, b"secret").unwrap();

        let resolved = resolve_workspace_path("computer://../secret.txt", Some(&workspace));

        assert!(resolved.is_none());
        let _ = std::fs::remove_dir_all(base);
    }

    /// Regression: `[name.pptx](name.pptx)` style relative markdown links
    /// emitted by the agent must be auto-pushed when the active workspace
    /// (Pro mode `current_workspace` OR Assistant mode `current_assistant`)
    /// is known. Previously only `current_workspace` was consulted, so
    /// assistant-mode replies silently dropped attachments — see
    /// `BotChatState::active_workspace_path` and the per-platform
    /// `notify_files_ready` callers.
    #[test]
    fn collects_relative_pptx_link_against_assistant_workspace_root() {
        let (base, workspace, _report) = make_temp_workspace();
        let pptx = workspace.join("apple-vision-pro-keynote-style.pptx");
        std::fs::write(&pptx, b"pptx-bytes").unwrap();

        let text = "[apple-vision-pro-keynote-style.pptx](apple-vision-pro-keynote-style.pptx)";
        let files = collect_auto_push_files(text, Some(&workspace));

        assert_eq!(files.len(), 1, "relative pptx link must be auto-pushed");
        assert_eq!(files[0].name, "apple-vision-pro-keynote-style.pptx");
        assert_eq!(files[0].size, b"pptx-bytes".len() as u64);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn extracts_relative_computer_links_when_workspace_root_is_known() {
        let (base, workspace, _report) = make_temp_workspace();
        let text = "Download [deck](computer://artifacts/report.pptx)";

        let paths = extract_downloadable_file_paths(text, Some(&workspace));

        assert_eq!(paths.len(), 1);
        assert!(std::path::Path::new(&paths[0]).is_absolute());
        assert!(std::path::Path::new(&paths[0])
            .ends_with(std::path::Path::new("artifacts").join("report.pptx")));
        assert!(std::path::Path::new(&paths[0]).exists());
        let _ = std::fs::remove_dir_all(base);
    }
}
