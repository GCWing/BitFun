//! DesignArtifact tool — produces and updates structured design deliverables that render
//! in the right-side Design Canvas tab (not inside the FlowChat stream).
//!
//! An artifact is a versioned multi-file design project rooted directly inside the
//! workspace-local `.design/` directory:
//!
//! * `<workspace>/.design/<artifact_id>/manifest.json`
//! * `<workspace>/.design/<artifact_id>/tokens.json`
//! * `<workspace>/.design/<artifact_id>/current/...`
//! * `<workspace>/.design/<artifact_id>/versions/<version_id>/...`
//! * `<workspace>/.design/<artifact_id>/thumbnails/...`
//!
//! `current/` is the live preview source of truth. Version snapshots and thumbnails
//! stay alongside it so the whole design harness is self-contained under `.design/`.
//!
//! One tool with an `action` discriminator is used so the Design agent has a compact
//! surface: `create | update_file | delete_file | set_entry | snapshot | get | list`.
//!
//! The frontend discovers artifacts via the JSON result returned on every call, which
//! always includes the full manifest plus an `artifact_event` describing what changed.

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use base64::Engine as _;
use chrono::Utc;
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write as IoWrite};
use std::path::{Path, PathBuf};
use tokio::fs;

const MAX_CREATE_INLINE_FILES: usize = 8;
const MAX_INLINE_FILE_CHARS: usize = 6_000;
const MAX_TOTAL_CREATE_INLINE_CHARS: usize = 12_000;
const MAX_UPDATE_FILE_CHARS: usize = 8_000;

/// Lock is considered stale after this many seconds without heartbeat/refresh.
/// The UI surfaces the holder + elapsed time; a human action can always force-override.
const LOCK_STALE_SECONDS: i64 = 120;

#[derive(Debug, Clone, Deserialize)]
struct CommittedTokensEnvelope {
    #[serde(default)]
    proposals: Vec<Value>,
    #[serde(default)]
    committed_id: Option<String>,
}

/// Canonical design-tokens schema consumed by the artifact scaffolder.
///
/// Mirrors `prompts/design_mode.md` and the `DesignTokensDocument.proposals[*]` shape.
/// Kept intentionally permissive (`#[serde(default)]` + `Option`) so legacy / partial
/// proposals do not fail deserialization, but every known key has a canonical home
/// — the scaffold reads from these canonical keys, not arbitrary JSON pointers,
/// so adding a new alias requires updating this struct (and nothing else).
#[derive(Debug, Clone, Default, Deserialize)]
struct TokensColors {
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    surface: Option<String>,
    #[serde(default, alias = "surface_elevated", alias = "surface-elevated")]
    surface_elevated: Option<String>,
    #[serde(default)]
    border: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default, alias = "text_secondary", alias = "text-secondary")]
    text_secondary: Option<String>,
    #[serde(default, alias = "text_muted", alias = "text-muted")]
    text_muted: Option<String>,
    #[serde(default)]
    primary: Option<String>,
    #[serde(default, alias = "primary_hover", alias = "primary-hover")]
    primary_hover: Option<String>,
    #[serde(default)]
    accent: Option<String>,
    #[serde(default)]
    success: Option<String>,
    #[serde(default)]
    warning: Option<String>,
    #[serde(default)]
    danger: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TokensTypographyScale {
    #[serde(default)]
    display: Option<String>,
    #[serde(default)]
    headline: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    caption: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TokensTypography {
    #[serde(default, alias = "family", alias = "font_family")]
    #[serde(rename = "fontFamily")]
    font_family: Option<String>,
    #[serde(default, alias = "familyMono", alias = "font_family_mono")]
    #[serde(rename = "fontFamilyMono")]
    font_family_mono: Option<String>,
    #[serde(default)]
    scale: TokensTypographyScale,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TokensRadius {
    #[serde(default)]
    sm: Option<String>,
    #[serde(default)]
    md: Option<String>,
    #[serde(default)]
    lg: Option<String>,
    #[serde(default)]
    full: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TokensShadow {
    #[serde(default)]
    sm: Option<String>,
    #[serde(default)]
    md: Option<String>,
    #[serde(default)]
    lg: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TokensSpacing {
    #[serde(default)]
    xs: Option<String>,
    #[serde(default)]
    sm: Option<String>,
    #[serde(default)]
    md: Option<String>,
    #[serde(default)]
    lg: Option<String>,
    #[serde(default)]
    xl: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CanonicalTokens {
    #[serde(default)]
    colors: TokensColors,
    #[serde(default)]
    typography: TokensTypography,
    #[serde(default)]
    radius: TokensRadius,
    #[serde(default)]
    shadow: TokensShadow,
    #[serde(default)]
    spacing: TokensSpacing,
}

impl CanonicalTokens {
    fn from_value(value: &Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }
}

/// Descriptor of a single file inside an artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignArtifactFileEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub updated_at: String,
}

/// Descriptor of a single version snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignArtifactVersion {
    pub id: String,
    pub parent: Option<String>,
    pub author: String,
    pub summary: String,
    pub created_at: String,
}

/// A short-lived edit lock record. When present, clients should treat Monaco as
/// read-only. The lock is always advisory — snapshot / update_file still work
/// once released, and the tool also rejects writes with a stale expected_version
/// (rebase protocol).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignArtifactLock {
    pub holder: String,
    pub since: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Persisted manifest for an artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignArtifactManifest {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub entry: String,
    pub viewports: Vec<String>,
    pub files: Vec<DesignArtifactFileEntry>,
    pub root: String,
    pub current_version: Option<String>,
    pub versions: Vec<DesignArtifactVersion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editing_lock: Option<DesignArtifactLock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct DesignArtifactTool;

impl Default for DesignArtifactTool {
    fn default() -> Self {
        Self::new()
    }
}

impl DesignArtifactTool {
    pub fn new() -> Self {
        Self
    }

    fn slugify(input: &str) -> String {
        let lower: String = input
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let trimmed = lower.trim_matches('-');
        let collapsed: String = trimmed.chars().fold(String::new(), |mut acc, c| {
            if c == '-' && acc.ends_with('-') {
                return acc;
            }
            acc.push(c);
            acc
        });
        if collapsed.is_empty() {
            "artifact".to_string()
        } else {
            collapsed
        }
    }

    fn workspace_root(context: &ToolUseContext) -> BitFunResult<PathBuf> {
        context
            .workspace_root()
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                BitFunError::tool("DesignArtifact requires an active workspace binding".to_string())
            })
    }

    fn artifact_dir(context: &ToolUseContext, artifact_id: &str) -> BitFunResult<PathBuf> {
        let workspace_root = Self::workspace_root(context)?;
        Ok(get_path_manager_arc().workspace_design_artifact_dir(&workspace_root, artifact_id))
    }

    fn manifest_path(artifact_dir: &Path) -> PathBuf {
        artifact_dir.join("manifest.json")
    }

    fn versions_dir(artifact_dir: &Path) -> PathBuf {
        artifact_dir.join("versions")
    }

    fn current_dir(artifact_dir: &Path) -> PathBuf {
        artifact_dir.join("current")
    }

    fn thumbnails_dir(artifact_dir: &Path) -> PathBuf {
        artifact_dir.join("thumbnails")
    }

    async fn ensure_dir(dir: &Path) -> BitFunResult<()> {
        if !dir.exists() {
            fs::create_dir_all(dir).await.map_err(|e| {
                BitFunError::tool(format!(
                    "DesignArtifact: failed to create {}: {}",
                    dir.display(),
                    e
                ))
            })?;
        }
        Ok(())
    }

    async fn load_manifest(runtime_dir: &Path) -> BitFunResult<DesignArtifactManifest> {
        let path = Self::manifest_path(runtime_dir);
        let raw = fs::read_to_string(&path).await.map_err(|e| {
            BitFunError::tool(format!(
                "DesignArtifact: manifest not found at {}: {}",
                path.display(),
                e
            ))
        })?;
        serde_json::from_str(&raw).map_err(|e| {
            BitFunError::tool(format!(
                "DesignArtifact: manifest at {} is corrupt: {}",
                path.display(),
                e
            ))
        })
    }

    async fn save_manifest(
        runtime_dir: &Path,
        manifest: &DesignArtifactManifest,
    ) -> BitFunResult<()> {
        Self::ensure_dir(runtime_dir).await?;
        let path = Self::manifest_path(runtime_dir);
        let serialized = serde_json::to_string_pretty(manifest)
            .map_err(|e| BitFunError::tool(format!("DesignArtifact: serialize manifest: {}", e)))?;
        fs::write(&path, serialized).await.map_err(|e| {
            BitFunError::tool(format!(
                "DesignArtifact: write manifest {}: {}",
                path.display(),
                e
            ))
        })
    }

    fn now_iso() -> String {
        Utc::now().to_rfc3339()
    }

    fn hash_bytes(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    fn validate_relative_path(path: &str) -> BitFunResult<PathBuf> {
        let normalized = path.trim().replace('\\', "/");
        if normalized.is_empty() {
            return Err(BitFunError::tool(
                "DesignArtifact: file path cannot be empty".to_string(),
            ));
        }
        if normalized.starts_with('/') || normalized.contains("..") {
            return Err(BitFunError::tool(format!(
                "DesignArtifact: file path '{}' must be relative and must not escape the artifact root",
                normalized
            )));
        }
        Ok(PathBuf::from(normalized))
    }

    fn validate_create_payload(input: &Value) -> ValidationResult {
        let Some(files) = input.get("files").and_then(|v| v.as_object()) else {
            return ValidationResult::default();
        };

        if files.len() > MAX_CREATE_INLINE_FILES {
            return ValidationResult {
                result: false,
                message: Some(format!(
                    "DesignArtifact.create received {} inline files, but the limit is {}. Create the artifact with a tiny scaffold first, then call update_file multiple times.",
                    files.len(),
                    MAX_CREATE_INLINE_FILES
                )),
                error_code: Some(400),
                meta: None,
            };
        }

        let mut total_chars = 0usize;
        for (path, value) in files {
            let Some(content) = value.as_str() else {
                return ValidationResult {
                    result: false,
                    message: Some(format!(
                        "DesignArtifact.create expects files['{}'] to be a string.",
                        path
                    )),
                    error_code: Some(400),
                    meta: None,
                };
            };
            total_chars += content.chars().count();
            if content.chars().count() > MAX_INLINE_FILE_CHARS {
                return ValidationResult {
                    result: false,
                    message: Some(format!(
                        "DesignArtifact.create inline file '{}' is too large (>{} chars). Create the artifact with a lightweight scaffold, then send this file via update_file in smaller steps.",
                        path,
                        MAX_INLINE_FILE_CHARS
                    )),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        if total_chars > MAX_TOTAL_CREATE_INLINE_CHARS {
            return ValidationResult {
                result: false,
                message: Some(format!(
                    "DesignArtifact.create inline payload is too large ({} chars > {}). Do not send full HTML/CSS/JS in create; scaffold first and then call update_file multiple times.",
                    total_chars,
                    MAX_TOTAL_CREATE_INLINE_CHARS
                )),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult::default()
    }

    fn validate_update_payload(input: &Value) -> ValidationResult {
        let Some(content) = input.get("content").and_then(|v| v.as_str()) else {
            return ValidationResult::default();
        };

        let content_chars = content.chars().count();
        if content_chars > MAX_UPDATE_FILE_CHARS {
            return ValidationResult {
                result: false,
                message: Some(format!(
                    "DesignArtifact.update_file payload is too large ({} chars > {}). Split the file into smaller files or update it incrementally. Large one-shot content often gets truncated into invalid JSON.",
                    content_chars,
                    MAX_UPDATE_FILE_CHARS
                )),
                error_code: Some(400),
                meta: None,
            };
        }

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if path.ends_with(".html") || path.ends_with(".htm") {
            if let Some(msg) = Self::lint_html_slop(content) {
                return ValidationResult {
                    result: false,
                    message: Some(msg),
                    error_code: Some(422),
                    meta: None,
                };
            }
        } else if path.ends_with(".css") && !path.ends_with("tokens.css") {
            if let Some(msg) = Self::lint_css_slop(content) {
                return ValidationResult {
                    result: false,
                    message: Some(msg),
                    error_code: Some(422),
                    meta: None,
                };
            }
        }

        ValidationResult::default()
    }

    /// Reject the most common AI-slop patterns in an entry HTML file so the
    /// agent is forced to build tokens-driven, editorial markup instead of
    /// re-inlining generic SaaS defaults. Returns `Some(reason)` if the
    /// content should be rejected.
    fn lint_html_slop(content: &str) -> Option<String> {
        // Count inline <style> blocks: entry HTML must not carry more than a
        // handful of critical-CSS lines; serious styling belongs in
        // styles/*.css.
        let lower = content.to_ascii_lowercase();
        if let Some(open) = lower.find("<style") {
            if let Some(close) = lower[open..].find("</style>") {
                let block = &content[open..open + close];
                let lines_in_block = block.lines().count();
                if lines_in_block > 40 {
                    return Some(format!(
                        "DesignArtifact.update_file rejected: entry HTML has an inline <style> block with {} lines. Move styles into styles/*.css and link them; keep any inline <style> under 40 lines for critical CSS only.",
                        lines_in_block
                    ));
                }
            }
        }

        // Reject entry HTML that rebuilds an entire token system inside a
        // :root selector — tokens belong in styles/tokens.css generated from
        // tokens.json.
        if lower.contains(":root") && lower.contains("--") && content.matches("--").count() >= 10 {
            return Some(
                "DesignArtifact.update_file rejected: entry HTML embeds its own token system in :root { --... }. Source tokens from styles/tokens.css (generated from tokens.json) instead of redefining them.".to_string(),
            );
        }

        None
    }

    /// Reject CSS files that hardcode aesthetic decisions that must come from
    /// the committed token system. The goal is soft: we allow a few stray hex
    /// values (e.g. `currentColor`-style tweaks) but refuse files that look
    /// like a parallel design system. `styles/tokens.css` itself is exempt
    /// from this check by the caller.
    fn lint_css_slop(content: &str) -> Option<String> {
        // Count hardcoded hex colors that are not part of a var() fallback.
        let hex_count = content
            .match_indices('#')
            .filter(|(idx, _)| {
                let rest = &content[*idx + 1..];
                let hex_len = rest.chars().take_while(|c| c.is_ascii_hexdigit()).count();
                hex_len == 3 || hex_len == 4 || hex_len == 6 || hex_len == 8
            })
            .count();
        if hex_count > 6 {
            return Some(format!(
                "DesignArtifact.update_file rejected: this CSS file hardcodes {} hex color literals. All colors must reference --dt-* custom properties declared in styles/tokens.css (sourced from tokens.json). Keep occasional hex values only for asset-specific overrides.",
                hex_count
            ));
        }

        let lower = content.to_ascii_lowercase();

        // `transition: all` is the signature of "animate everything" slop.
        if lower.contains("transition: all") || lower.contains("transition:all") {
            return Some(
                "DesignArtifact.update_file rejected: `transition: all` detected. Animate specific properties (opacity, transform, color) with reasoned durations; never blanket-animate everything.".to_string(),
            );
        }

        // Purple-to-blue / indigo-to-cyan hero gradients are the loudest slop
        // signature. Only reject multi-stop linear-gradients that are clearly
        // decorative; single-value linear-gradients used for narrow accents
        // are allowed.
        let gradient_count = lower.matches("linear-gradient(").count()
            + lower.matches("radial-gradient(").count()
            + lower.matches("conic-gradient(").count();
        if gradient_count > 3 {
            return Some(format!(
                "DesignArtifact.update_file rejected: this CSS file uses {} CSS gradients. Gradients are a blunt instrument — prefer solid tokens and let typography/space carry the hierarchy. Keep gradients to at most 3 per file and only where they serve a meaningful reason.",
                gradient_count
            ));
        }

        None
    }

    /// Render the committed tokens into a `tokens.css` file that exposes
    /// `--dt-*` custom properties consumed by the scaffold stylesheet.
    ///
    /// Only canonical keys are read (see `CanonicalTokens`). If a key is
    /// missing we fall back to a neutral value that still reads as a proper
    /// design decision (not as a placeholder) so a partial token system still
    /// produces a cohesive preview.
    fn render_tokens_css(committed: &Value) -> String {
        let t = CanonicalTokens::from_value(committed);

        fn or<'a>(v: &'a Option<String>, fallback: &'a str) -> &'a str {
            v.as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(fallback)
        }

        let font_family = or(
            &t.typography.font_family,
            "Inter, system-ui, -apple-system, sans-serif",
        );
        let font_family_mono = or(
            &t.typography.font_family_mono,
            "ui-monospace, SFMono-Regular, Menlo, monospace",
        );

        let background = or(&t.colors.background, "#ffffff");
        let surface = or(&t.colors.surface, "#fafafa");
        let surface_elevated = or(&t.colors.surface_elevated, "#ffffff");
        let border = or(&t.colors.border, "rgba(17, 17, 17, 0.08)");
        let text = or(&t.colors.text, "#0b0b0c");
        let text_secondary = or(&t.colors.text_secondary, "rgba(11, 11, 12, 0.72)");
        let text_muted = or(&t.colors.text_muted, "rgba(11, 11, 12, 0.52)");
        let primary = or(&t.colors.primary, "#0b0b0c");
        let primary_hover = or(&t.colors.primary_hover, primary);
        let accent = or(&t.colors.accent, primary);
        let success = or(&t.colors.success, "#16a34a");
        let warning = or(&t.colors.warning, "#d97706");
        let danger = or(&t.colors.danger, "#dc2626");

        let radius_sm = or(&t.radius.sm, "4px");
        let radius_md = or(&t.radius.md, "8px");
        let radius_lg = or(&t.radius.lg, "16px");
        let radius_full = or(&t.radius.full, "999px");

        let shadow_sm = or(&t.shadow.sm, "0 1px 2px rgba(0, 0, 0, 0.06)");
        let shadow_md = or(&t.shadow.md, "0 4px 14px rgba(0, 0, 0, 0.10)");
        let shadow_lg = or(&t.shadow.lg, "0 18px 40px rgba(0, 0, 0, 0.18)");

        let space_xs = or(&t.spacing.xs, "4px");
        let space_sm = or(&t.spacing.sm, "8px");
        let space_md = or(&t.spacing.md, "16px");
        let space_lg = or(&t.spacing.lg, "24px");
        let space_xl = or(&t.spacing.xl, "40px");

        let font_display = or(&t.typography.scale.display, "48px");
        let font_headline = or(&t.typography.scale.headline, "32px");
        let font_title = or(&t.typography.scale.title, "20px");
        let font_body = or(&t.typography.scale.body, "15px");
        let font_caption = or(&t.typography.scale.caption, "12px");

        format!(
            "/* Design tokens — generated from the committed token system.\n\
             * Do NOT edit by hand. Re-running DesignArtifact.create or the\n\
             * token studio overwrites this file. Every value in the artifact\n\
             * should reference one of these --dt-* properties. */\n\
:root {{\n\
  --dt-font-family: {font_family};\n\
  --dt-font-family-mono: {font_family_mono};\n\
  --dt-font-display: {font_display};\n\
  --dt-font-headline: {font_headline};\n\
  --dt-font-title: {font_title};\n\
  --dt-font-body: {font_body};\n\
  --dt-font-caption: {font_caption};\n\
\n\
  --dt-background: {background};\n\
  --dt-surface: {surface};\n\
  --dt-surface-elevated: {surface_elevated};\n\
  --dt-border: {border};\n\
  --dt-text: {text};\n\
  --dt-text-secondary: {text_secondary};\n\
  --dt-text-muted: {text_muted};\n\
  --dt-primary: {primary};\n\
  --dt-primary-hover: {primary_hover};\n\
  --dt-accent: {accent};\n\
  --dt-success: {success};\n\
  --dt-warning: {warning};\n\
  --dt-danger: {danger};\n\
\n\
  --dt-radius-sm: {radius_sm};\n\
  --dt-radius-md: {radius_md};\n\
  --dt-radius-lg: {radius_lg};\n\
  --dt-radius-full: {radius_full};\n\
\n\
  --dt-shadow-sm: {shadow_sm};\n\
  --dt-shadow-md: {shadow_md};\n\
  --dt-shadow-lg: {shadow_lg};\n\
\n\
  --dt-space-xs: {space_xs};\n\
  --dt-space-sm: {space_sm};\n\
  --dt-space-md: {space_md};\n\
  --dt-space-lg: {space_lg};\n\
  --dt-space-xl: {space_xl};\n\
}}\n",
        )
    }

    async fn resolve_committed_tokens(
        context: &ToolUseContext,
        artifact_id: Option<&str>,
    ) -> BitFunResult<Value> {
        let workspace_root = Self::workspace_root(context)?;
        let workspace_tokens_path =
            get_path_manager_arc().workspace_design_tokens_file(&workspace_root);
        let artifact_tokens_path = artifact_id.map(|id| {
            get_path_manager_arc()
                .workspace_design_artifact_dir(&workspace_root, id)
                .join("tokens.json")
        });

        for path in artifact_tokens_path
            .into_iter()
            .chain(std::iter::once(workspace_tokens_path))
        {
            if !path.exists() {
                continue;
            }
            let raw = fs::read_to_string(&path).await.map_err(|e| {
                BitFunError::tool(format!(
                    "DesignArtifact: failed to read tokens {}: {}",
                    path.display(),
                    e
                ))
            })?;
            let envelope: CommittedTokensEnvelope = serde_json::from_str(&raw).map_err(|e| {
                BitFunError::tool(format!(
                    "DesignArtifact: invalid tokens json at {}: {}",
                    path.display(),
                    e
                ))
            })?;
            let Some(committed_id) = envelope.committed_id.as_deref() else {
                continue;
            };
            if let Some(proposal) = envelope
                .proposals
                .into_iter()
                .find(|proposal| proposal.get("id").and_then(|v| v.as_str()) == Some(committed_id))
            {
                return Ok(proposal);
            }
        }

        Err(BitFunError::tool(
            "TOKENS_NOT_COMMITTED — commit a design token proposal with DesignTokens.commit before creating an artifact",
        ))
    }

    async fn write_file(
        artifact_dir: &Path,
        relative: &Path,
        content: &str,
    ) -> BitFunResult<DesignArtifactFileEntry> {
        let bytes = content.as_bytes();
        let current_path = Self::current_dir(artifact_dir).join(relative);
        if let Some(parent) = current_path.parent() {
            Self::ensure_dir(parent).await?;
        }
        fs::write(&current_path, bytes).await.map_err(|e| {
            BitFunError::tool(format!(
                "DesignArtifact: write file {}: {}",
                current_path.display(),
                e
            ))
        })?;

        Ok(DesignArtifactFileEntry {
            path: relative.to_string_lossy().replace('\\', "/"),
            size: bytes.len() as u64,
            sha256: Self::hash_bytes(bytes),
            updated_at: Self::now_iso(),
        })
    }

    async fn delete_file(artifact_dir: &Path, relative: &Path) -> BitFunResult<()> {
        let current_path = Self::current_dir(artifact_dir).join(relative);
        if current_path.exists() {
            fs::remove_file(&current_path).await.map_err(|e| {
                BitFunError::tool(format!(
                    "DesignArtifact: delete file {}: {}",
                    current_path.display(),
                    e
                ))
            })?;
        }
        Ok(())
    }

    async fn scan_current_files(artifact_dir: &Path) -> BitFunResult<Vec<DesignArtifactFileEntry>> {
        let current_dir = Self::current_dir(artifact_dir);
        let mut out = Vec::new();
        let mut stack = vec![current_dir.clone()];

        while let Some(dir) = stack.pop() {
            let mut entries = fs::read_dir(&dir).await.map_err(|e| {
                BitFunError::tool(format!(
                    "DesignArtifact.sync: read directory {}: {}",
                    dir.display(),
                    e
                ))
            })?;

            while let Some(entry) = entries.next_entry().await.map_err(|e| {
                BitFunError::tool(format!(
                    "DesignArtifact.sync: read directory entry {}: {}",
                    dir.display(),
                    e
                ))
            })? {
                let path = entry.path();
                let metadata = entry.metadata().await.map_err(|e| {
                    BitFunError::tool(format!(
                        "DesignArtifact.sync: stat {}: {}",
                        path.display(),
                        e
                    ))
                })?;

                if metadata.is_dir() {
                    stack.push(path);
                    continue;
                }

                if !metadata.is_file() {
                    continue;
                }

                let relative = path.strip_prefix(&current_dir).map_err(|e| {
                    BitFunError::tool(format!(
                        "DesignArtifact.sync: path {} is outside current dir {}: {}",
                        path.display(),
                        current_dir.display(),
                        e
                    ))
                })?;
                let bytes = fs::read(&path).await.map_err(|e| {
                    BitFunError::tool(format!(
                        "DesignArtifact.sync: read file {}: {}",
                        path.display(),
                        e
                    ))
                })?;

                out.push(DesignArtifactFileEntry {
                    path: relative.to_string_lossy().replace('\\', "/"),
                    size: bytes.len() as u64,
                    sha256: Self::hash_bytes(&bytes),
                    updated_at: Self::now_iso(),
                });
            }
        }

        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    async fn handle_create(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<DesignArtifactManifest> {
        let title = input
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BitFunError::tool("DesignArtifact.create: title is required"))?;
        let kind = input
            .get("kind")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("page")
            .to_string();
        let supplied_id = input
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let id = supplied_id.map(|s| s.to_string()).unwrap_or_else(|| {
            format!(
                "{}-{}",
                Self::slugify(title),
                &Self::hash_bytes(title.as_bytes())[..8]
            )
        });

        let artifact_dir = Self::artifact_dir(context, &id)?;
        let committed_tokens = Self::resolve_committed_tokens(context, Some(&id)).await?;

        // Refuse to overwrite an existing artifact through create.
        if Self::manifest_path(&artifact_dir).exists() {
            return Err(BitFunError::tool(format!(
                "DesignArtifact.create: artifact '{}' already exists; use update_file or snapshot instead",
                id
            )));
        }

        Self::ensure_dir(&artifact_dir).await?;
        Self::ensure_dir(&Self::current_dir(&artifact_dir)).await?;
        Self::ensure_dir(&Self::versions_dir(&artifact_dir)).await?;
        Self::ensure_dir(&Self::thumbnails_dir(&artifact_dir)).await?;

        let viewports = input
            .get("viewports")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec!["desktop".into(), "tablet".into(), "mobile".into()]);

        let entry = input
            .get("entry")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("index.html")
            .to_string();

        let mut files: Vec<DesignArtifactFileEntry> = Vec::new();
        if let Some(files_obj) = input.get("files").and_then(|v| v.as_object()) {
            for (rel, value) in files_obj {
                let content = value.as_str().unwrap_or("");
                let relative = Self::validate_relative_path(rel)?;
                let entry_file = Self::write_file(&artifact_dir, &relative, content).await?;
                files.push(entry_file);
            }
        } else {
            // Seed a minimal, intentionally undecorated multi-file scaffold.
            //
            // Design principle: the scaffold must NOT introduce any aesthetic
            // decisions. It only wires the committed design tokens into real
            // `--dt-*` CSS custom properties and establishes three empty files
            // (`index.html`, `styles/main.css`, `styles/tokens.css`) so later
            // `update_file` patches always have a stable home.
            //
            // The previous scaffold hard-coded a dark-blue palette + a radial
            // gradient "aura" + a big display headline — all of which are
            // exactly the AI-slop patterns the design prompt forbids, and all
            // of which silently contradicted the committed token system
            // whenever token json keys differed from the bespoke pointer paths
            // (`/colors/bg` did not exist; the actual schema uses
            // `/colors/background`). We now read from the correct schema keys
            // and fall back only as a last resort.
            let entry_relative = Self::validate_relative_path(&entry)?;
            let tokens_css = Self::render_tokens_css(&committed_tokens);

            let entry_seed = format!(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>{title}</title>\n  <link rel=\"stylesheet\" href=\"styles/tokens.css\">\n  <link rel=\"stylesheet\" href=\"styles/main.css\">\n</head>\n<body>\n  <main class=\"artifact-root\"></main>\n  <script src=\"scripts/main.js\"></script>\n</body>\n</html>\n",
                title = title
            );
            let entry_file = Self::write_file(&artifact_dir, &entry_relative, &entry_seed).await?;
            files.push(entry_file);

            let tokens_relative = Self::validate_relative_path("styles/tokens.css")?;
            let tokens_file =
                Self::write_file(&artifact_dir, &tokens_relative, &tokens_css).await?;
            files.push(tokens_file);

            let style_relative = Self::validate_relative_path("styles/main.css")?;
            let style_seed = "\
/* Artifact stylesheet. All values MUST reference the --dt-* custom properties\n\
   declared in styles/tokens.css (which mirrors the committed tokens.json).\n\
   Do not hardcode colors, font families, radii, shadows, or spacing here. */\n\n\
*, *::before, *::after { box-sizing: border-box; }\n\
html, body { margin: 0; }\n\
body {\n  background: var(--dt-background);\n  color: var(--dt-text);\n  font-family: var(--dt-font-family);\n  font-size: var(--dt-font-body, 15px);\n  line-height: var(--dt-lh-body, 1.55);\n  -webkit-font-smoothing: antialiased;\n}\n\n\
.artifact-root {\n  min-height: 100vh;\n}\n";
            let style_file = Self::write_file(&artifact_dir, &style_relative, style_seed).await?;
            files.push(style_file);

            let script_relative = Self::validate_relative_path("scripts/main.js")?;
            // Intentionally empty — avoids the previous "stub main.js + real
            // app.js" duplication. The agent owns this file; if no JS is
            // needed the file simply stays empty.
            let script_seed = "// Artifact entry script. Intentionally empty.\n";
            let script_file =
                Self::write_file(&artifact_dir, &script_relative, script_seed).await?;
            files.push(script_file);
        }

        let now = Self::now_iso();
        let manifest = DesignArtifactManifest {
            id: id.clone(),
            title: title.to_string(),
            kind,
            entry,
            viewports,
            files,
            root: artifact_dir.to_string_lossy().to_string(),
            current_version: None,
            versions: Vec::new(),
            editing_lock: None,
            thumbnail: None,
            archived_at: None,
            created_at: now.clone(),
            updated_at: now,
        };
        Self::save_manifest(&artifact_dir, &manifest).await?;
        Ok(manifest)
    }

    async fn handle_sync(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<DesignArtifactManifest> {
        let artifact_id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BitFunError::tool("DesignArtifact.sync: artifact_id is required"))?;

        let artifact_dir = Self::artifact_dir(context, artifact_id)?;
        let mut manifest = Self::load_manifest(&artifact_dir).await?;
        let files = Self::scan_current_files(&artifact_dir).await?;

        if files.is_empty() {
            return Err(BitFunError::tool(format!(
                "DesignArtifact.sync: no files found in {}",
                Self::current_dir(&artifact_dir).display()
            )));
        }

        if let Some(entry) = input
            .get("entry")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            let entry = entry.replace('\\', "/");
            if !files.iter().any(|file| file.path == entry) {
                return Err(BitFunError::tool(format!(
                    "DesignArtifact.sync: entry '{}' is not present in current files",
                    entry
                )));
            }
            manifest.entry = entry;
        } else if !files.iter().any(|file| file.path == manifest.entry) {
            if let Some(html_entry) = files.iter().find(|file| file.path.ends_with(".html")) {
                manifest.entry = html_entry.path.clone();
            } else if let Some(first) = files.first() {
                manifest.entry = first.path.clone();
            }
        }

        manifest.files = files;
        manifest.updated_at = Self::now_iso();
        Self::save_manifest(&artifact_dir, &manifest).await?;
        Ok(manifest)
    }

    /// Validate the optional `expected_version` against the manifest.
    /// The rebase protocol: caller passes the version they believed was current;
    /// if the server has since advanced (another writer committed), we reject
    /// so the UI can refresh and retry.
    fn check_expected_version(
        input: &Value,
        manifest: &DesignArtifactManifest,
    ) -> BitFunResult<()> {
        let Some(expected) = input.get("expected_version").and_then(|v| v.as_str()) else {
            return Ok(());
        };
        let expected = expected.trim();
        if expected.is_empty() {
            return Ok(());
        }
        let current = manifest.current_version.as_deref().unwrap_or("");
        if current != expected {
            return Err(BitFunError::tool(format!(
                "DesignArtifact: VERSION_CONFLICT — expected current_version '{}', actual '{}'. Refresh and retry.",
                expected, current
            )));
        }
        Ok(())
    }

    /// True if a lock record is older than `LOCK_STALE_SECONDS` (holder likely
    /// crashed or left). Stale locks do not block writes — this prevents
    /// "lock zombies" from forcing every user to hit `force: true`.
    fn lock_is_stale(lock: &DesignArtifactLock) -> bool {
        let Ok(since) = chrono::DateTime::parse_from_rfc3339(&lock.since) else {
            return true;
        };
        let elapsed = Utc::now().signed_duration_since(since.with_timezone(&Utc));
        elapsed.num_seconds() > LOCK_STALE_SECONDS
    }

    /// Enforce editing lock semantics. Writers must supply the lock holder via
    /// `author` (snapshot) or a new `as` field; other actors are rejected unless
    /// the lock is already stale.
    fn check_editing_lock(
        input: &Value,
        manifest: &DesignArtifactManifest,
        override_flag_key: &str,
    ) -> BitFunResult<()> {
        let Some(lock) = manifest.editing_lock.as_ref() else {
            return Ok(());
        };
        if input
            .get(override_flag_key)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Ok(());
        }
        if Self::lock_is_stale(lock) {
            warn!(
                "DesignArtifact: ignoring stale editing lock held by '{}' since {}",
                lock.holder, lock.since
            );
            return Ok(());
        }
        let actor = input
            .get("as")
            .and_then(|v| v.as_str())
            .or_else(|| input.get("author").and_then(|v| v.as_str()))
            .unwrap_or("agent");
        if actor != lock.holder {
            return Err(BitFunError::tool(format!(
                "DesignArtifact: EDIT_LOCKED — '{}' is currently editing this artifact (held since {}). Call release_lock or pass `force: true`.",
                lock.holder, lock.since
            )));
        }
        Ok(())
    }

    async fn handle_update_file(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<DesignArtifactManifest> {
        let artifact_id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool("DesignArtifact.update_file: artifact_id is required")
            })?;
        let file_path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("DesignArtifact.update_file: path is required"))?;
        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("DesignArtifact.update_file: content is required"))?;

        let artifact_dir = Self::artifact_dir(context, artifact_id)?;
        let mut manifest = Self::load_manifest(&artifact_dir).await?;
        Self::check_editing_lock(input, &manifest, "force")?;
        Self::check_expected_version(input, &manifest)?;
        let relative = Self::validate_relative_path(file_path)?;
        let entry_file = Self::write_file(&artifact_dir, &relative, content).await?;

        manifest.files.retain(|f| f.path != entry_file.path);
        manifest.files.push(entry_file);
        manifest.updated_at = Self::now_iso();
        Self::save_manifest(&artifact_dir, &manifest).await?;
        Ok(manifest)
    }

    async fn handle_delete_file(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<DesignArtifactManifest> {
        let artifact_id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool("DesignArtifact.delete_file: artifact_id is required")
            })?;
        let file_path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("DesignArtifact.delete_file: path is required"))?;
        let artifact_dir = Self::artifact_dir(context, artifact_id)?;
        let mut manifest = Self::load_manifest(&artifact_dir).await?;
        Self::check_editing_lock(input, &manifest, "force")?;
        Self::check_expected_version(input, &manifest)?;
        let relative = Self::validate_relative_path(file_path)?;
        Self::delete_file(&artifact_dir, &relative).await?;
        let normalized = relative.to_string_lossy().replace('\\', "/");
        manifest.files.retain(|f| f.path != normalized);
        manifest.updated_at = Self::now_iso();
        Self::save_manifest(&artifact_dir, &manifest).await?;
        Ok(manifest)
    }

    async fn handle_set_entry(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<DesignArtifactManifest> {
        let artifact_id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool("DesignArtifact.set_entry: artifact_id is required")
            })?;
        let new_entry = input
            .get("entry")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("DesignArtifact.set_entry: entry is required"))?;
        let artifact_dir = Self::artifact_dir(context, artifact_id)?;
        let mut manifest = Self::load_manifest(&artifact_dir).await?;
        Self::check_editing_lock(input, &manifest, "force")?;
        let normalized = Self::validate_relative_path(new_entry)?
            .to_string_lossy()
            .replace('\\', "/");
        if !manifest.files.iter().any(|f| f.path == normalized) {
            return Err(BitFunError::tool(format!(
                "DesignArtifact.set_entry: entry '{}' is not a known file; add it first",
                normalized
            )));
        }
        manifest.entry = normalized;
        manifest.updated_at = Self::now_iso();
        Self::save_manifest(&artifact_dir, &manifest).await?;
        Ok(manifest)
    }

    async fn handle_snapshot(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<DesignArtifactManifest> {
        let artifact_id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("DesignArtifact.snapshot: artifact_id is required"))?;
        let summary = input
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("snapshot")
            .to_string();
        let author = input
            .get("author")
            .and_then(|v| v.as_str())
            .unwrap_or("agent")
            .to_string();

        let artifact_dir = Self::artifact_dir(context, artifact_id)?;
        let mut manifest = Self::load_manifest(&artifact_dir).await?;

        // Pure content-addressable version id. Hash each file's bytes, not the
        // manifest metadata (which contains timestamps that change on every write
        // and would make two truly-identical snapshots appear distinct). We read
        // files in a stable sort order so the resulting id is deterministic.
        let mut sorted_files = manifest.files.clone();
        sorted_files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut content_hasher = Sha256::new();
        let current_dir = Self::current_dir(&artifact_dir);
        for file in &sorted_files {
            content_hasher.update(file.path.as_bytes());
            content_hasher.update(b"\0");
            let source = current_dir.join(&file.path);
            match fs::read(&source).await {
                Ok(bytes) => content_hasher.update(&bytes),
                Err(e) => {
                    return Err(BitFunError::tool(format!(
                        "DesignArtifact.snapshot: missing or unreadable file {}: {}",
                        source.display(),
                        e
                    )));
                }
            }
            content_hasher.update(b"\n");
        }
        let version_id = hex::encode(content_hasher.finalize())[..12].to_string();

        let version_dir = Self::versions_dir(&artifact_dir).join(&version_id);
        if !version_dir.exists() {
            // Stage under a temp sibling, then rename atomically — prevents a
            // half-populated `versions/<id>/` from looking like a real snapshot
            // if the process dies mid-copy.
            let staging_name = format!(".{}.{}.tmp", version_id, Utc::now().timestamp_millis());
            let staging_dir = Self::versions_dir(&artifact_dir).join(&staging_name);
            Self::ensure_dir(&staging_dir).await?;

            let copy_result: BitFunResult<()> = async {
                for file in &manifest.files {
                    let source = current_dir.join(&file.path);
                    if !source.exists() {
                        return Err(BitFunError::tool(format!(
                            "DesignArtifact.snapshot: file listed in manifest missing from disk: {}",
                            source.display()
                        )));
                    }
                    let dest = staging_dir.join(&file.path);
                    if let Some(parent) = dest.parent() {
                        Self::ensure_dir(parent).await?;
                    }
                    fs::copy(&source, &dest).await.map_err(|e| {
                        BitFunError::tool(format!(
                            "DesignArtifact.snapshot: copy {} -> {}: {}",
                            source.display(),
                            dest.display(),
                            e
                        ))
                    })?;
                }
                Ok(())
            }
            .await;

            if let Err(err) = copy_result {
                let _ = fs::remove_dir_all(&staging_dir).await;
                return Err(err);
            }

            // Atomic promote. If a concurrent writer already published the same
            // id, just drop our staging copy (idempotent snapshot).
            if version_dir.exists() {
                let _ = fs::remove_dir_all(&staging_dir).await;
            } else if let Err(e) = fs::rename(&staging_dir, &version_dir).await {
                let _ = fs::remove_dir_all(&staging_dir).await;
                return Err(BitFunError::tool(format!(
                    "DesignArtifact.snapshot: promote staging -> {}: {}",
                    version_dir.display(),
                    e
                )));
            }
        }

        let version = DesignArtifactVersion {
            id: version_id.clone(),
            parent: manifest.current_version.clone(),
            author,
            summary,
            created_at: Self::now_iso(),
        };
        manifest.versions.retain(|v| v.id != version_id);
        manifest.versions.push(version);
        manifest.current_version = Some(version_id);
        manifest.updated_at = Self::now_iso();
        Self::save_manifest(&artifact_dir, &manifest).await?;
        Ok(manifest)
    }

    async fn handle_acquire_lock(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<DesignArtifactManifest> {
        let artifact_id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool("DesignArtifact.acquire_lock: artifact_id is required")
            })?;
        let holder = input
            .get("holder")
            .and_then(|v| v.as_str())
            .unwrap_or("agent")
            .to_string();
        let note = input
            .get("note")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let artifact_dir = Self::artifact_dir(context, artifact_id)?;
        let mut manifest = Self::load_manifest(&artifact_dir).await?;
        if let Some(existing) = manifest.editing_lock.as_ref() {
            let force = input
                .get("force")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if existing.holder != holder && !force && !Self::lock_is_stale(existing) {
                return Err(BitFunError::tool(format!(
                    "DesignArtifact.acquire_lock: already locked by '{}' (since {}). Retry with force:true to take over.",
                    existing.holder, existing.since
                )));
            }
        }
        manifest.editing_lock = Some(DesignArtifactLock {
            holder,
            since: Self::now_iso(),
            note,
        });
        manifest.updated_at = Self::now_iso();
        Self::save_manifest(&artifact_dir, &manifest).await?;
        Ok(manifest)
    }

    async fn handle_release_lock(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<DesignArtifactManifest> {
        let artifact_id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool("DesignArtifact.release_lock: artifact_id is required")
            })?;
        let artifact_dir = Self::artifact_dir(context, artifact_id)?;
        let mut manifest = Self::load_manifest(&artifact_dir).await?;
        manifest.editing_lock = None;
        manifest.updated_at = Self::now_iso();
        Self::save_manifest(&artifact_dir, &manifest).await?;
        Ok(manifest)
    }

    async fn handle_set_thumbnail(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<DesignArtifactManifest> {
        let artifact_id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool("DesignArtifact.set_thumbnail: artifact_id is required")
            })?;
        let data_url = input
            .get("data_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool(
                    "DesignArtifact.set_thumbnail: data_url is required (data:image/...;base64,...)",
                )
            })?;
        let (mime, b64) = match data_url.split_once(',') {
            Some((header, body)) if header.starts_with("data:") && header.contains("base64") => {
                let mime = header
                    .trim_start_matches("data:")
                    .split(';')
                    .next()
                    .unwrap_or("image/png");
                (mime.to_string(), body.to_string())
            }
            _ => {
                return Err(BitFunError::tool(
                    "DesignArtifact.set_thumbnail: data_url must be a base64 data URL",
                ))
            }
        };
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .map_err(|e| {
                BitFunError::tool(format!(
                    "DesignArtifact.set_thumbnail: invalid base64: {}",
                    e
                ))
            })?;
        let ext = if mime.contains("jpeg") || mime.contains("jpg") {
            "jpg"
        } else if mime.contains("webp") {
            "webp"
        } else {
            "png"
        };
        let artifact_dir = Self::artifact_dir(context, artifact_id)?;
        let thumbs_dir = Self::thumbnails_dir(&artifact_dir);
        Self::ensure_dir(&thumbs_dir).await?;
        let file_name = format!("{}.{}", Utc::now().timestamp_millis(), ext);
        let dest = thumbs_dir.join(&file_name);
        fs::write(&dest, &bytes).await.map_err(|e| {
            BitFunError::tool(format!(
                "DesignArtifact.set_thumbnail: write {}: {}",
                dest.display(),
                e
            ))
        })?;
        let mut manifest = Self::load_manifest(&artifact_dir).await?;
        manifest.thumbnail = Some(dest.to_string_lossy().to_string());
        manifest.updated_at = Self::now_iso();
        Self::save_manifest(&artifact_dir, &manifest).await?;
        Ok(manifest)
    }

    async fn handle_zip_export(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<(DesignArtifactManifest, PathBuf)> {
        let artifact_id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool("DesignArtifact.zip_export: artifact_id is required")
            })?;
        let artifact_dir = Self::artifact_dir(context, artifact_id)?;
        let manifest = Self::load_manifest(&artifact_dir).await?;
        let mut buffer: Vec<u8> = Vec::new();
        {
            let cursor = Cursor::new(&mut buffer);
            let mut zip_writer = zip::ZipWriter::new(cursor);
            let opts: zip::write::FileOptions = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for file in &manifest.files {
                let path_on_disk = Self::current_dir(&artifact_dir).join(&file.path);
                let Ok(content) = fs::read(&path_on_disk).await else {
                    continue;
                };
                zip_writer
                    .start_file(&file.path, opts)
                    .map_err(|e| BitFunError::tool(format!("zip start_file: {}", e)))?;
                zip_writer
                    .write_all(&content)
                    .map_err(|e| BitFunError::tool(format!("zip write: {}", e)))?;
            }
            zip_writer
                .finish()
                .map_err(|e| BitFunError::tool(format!("zip finish: {}", e)))?;
        }
        Self::ensure_dir(&artifact_dir).await?;
        let zip_path = artifact_dir.join(format!("{}.zip", manifest.id));
        fs::write(&zip_path, &buffer).await.map_err(|e| {
            BitFunError::tool(format!(
                "DesignArtifact.zip_export: write {}: {}",
                zip_path.display(),
                e
            ))
        })?;
        Ok((manifest, zip_path))
    }

    async fn handle_archive(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<DesignArtifactManifest> {
        let artifact_id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("DesignArtifact.archive: artifact_id is required"))?;
        let artifact_dir = Self::artifact_dir(context, artifact_id)?;
        let mut manifest = Self::load_manifest(&artifact_dir).await?;
        let unarchive = input
            .get("unarchive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        manifest.archived_at = if unarchive {
            None
        } else {
            Some(Self::now_iso())
        };
        manifest.updated_at = Self::now_iso();
        Self::save_manifest(&artifact_dir, &manifest).await?;
        Ok(manifest)
    }

    async fn handle_get(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<DesignArtifactManifest> {
        let artifact_id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("DesignArtifact.get: artifact_id is required"))?;
        let artifact_dir = Self::artifact_dir(context, artifact_id)?;
        Self::load_manifest(&artifact_dir).await
    }

    async fn handle_list(
        &self,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<DesignArtifactManifest>> {
        let workspace_root = Self::workspace_root(context)?;
        let root = get_path_manager_arc().workspace_design_root(&workspace_root);
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        let mut entries = fs::read_dir(&root).await.map_err(|e| {
            BitFunError::tool(format!(
                "DesignArtifact.list: read {}: {}",
                root.display(),
                e
            ))
        })?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let entry_path = entry.path();
            if !entry_path.is_dir() {
                continue;
            }
            if let Ok(manifest) = Self::load_manifest(&entry_path).await {
                out.push(manifest);
            }
        }
        Ok(out)
    }
}

#[async_trait]
impl Tool for DesignArtifactTool {
    fn name(&self) -> &str {
        "DesignArtifact"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Create and evolve a Design Artifact — a multi-file, versioned HTML design project rendered in the right-side Design Canvas tab.

Use DesignArtifact instead of bare Write/Edit when producing a design deliverable that the user will view, edit, and iterate on as a living artifact. Do NOT use GenerativeUI for design work — that tool is for one-off in-chat widgets.

Actions (discriminated by `action`):
- `create`   { title, kind?, id?, entry?, viewports?, files? } — Create a new artifact. Returns its manifest. IMPORTANT: keep `files` tiny. Prefer a minimal scaffold only.
- `update_file` { artifact_id, path, content, expected_version?, as?, force? } — Write/overwrite one file. If expected_version is provided and does not match manifest.current_version, the write is rejected (VERSION_CONFLICT).
- `delete_file` { artifact_id, path, expected_version?, as?, force? } — Remove a file from the artifact.
- `set_entry` { artifact_id, entry, as?, force? } — Set the default preview entry (must be an existing file).
- `snapshot` { artifact_id, summary?, author? } — Freeze current files as an immutable version in the history.
- `acquire_lock` { artifact_id, holder?, note?, force? } — Mark the artifact as being edited so other writers are rejected.
- `release_lock` { artifact_id } — Clear the editing lock.
- `set_thumbnail` { artifact_id, data_url } — Store a PNG/JPG/WebP thumbnail (data URL). Typically called by the UI on snapshot.
- `zip_export` { artifact_id } — Pack the artifact files into `.design/<id>/<id>.zip`.
- `archive` { artifact_id, unarchive? } — Soft-archive (or unarchive) the artifact so the browser hides it by default.
- `get` { artifact_id } — Return the current manifest.
- `list` — Return manifests for all artifacts in this workspace.

Operational rules:
- ALWAYS prefer updating an existing artifact. Create new ones only for a genuinely new design.
- CRITICAL anti-truncation rule: do NOT send a full large prototype in one tool call. Create a lightweight scaffold first, then call `update_file` multiple times for `index.html`, CSS, JS, and assets.
- Recommended scaffold layout: create the artifact with `index.html`, `styles/main.css`, and `scripts/main.js` as the initial relative paths, then grow from there.
- Keep inline payloads small. As a working rule, keep every `update_file.content` comfortably below ~8k characters, and keep `create.files` to a tiny seed only.
- If a page is large, split it into multiple files (`index.html`, `styles/*.css`, `scripts/*.js`) instead of sending one huge HTML document.
- Use relative paths inside the artifact (e.g. `index.html`, `styles/main.css`). Never use `..` or absolute paths.
- Call `snapshot` at natural save points (finished an iteration, end of a Todo batch). Every snapshot records a short `summary` describing intent.
- Keep each file under ~1000 lines; split into subfiles when they grow.
- The artifact lives under `.design/<artifact_id>/` in the workspace. Create, sync, snapshot, and inspect through this tool; write substantial file contents with Write/Edit under `<manifest.root>/current/...`."#.to_string())
    }

    async fn description_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(r#"Create and evolve a Design Artifact: a multi-file HTML/CSS/JS project rendered in the right-side Design Canvas tab.

DesignArtifact owns the manifest, Canvas refresh, snapshots, and listing. File content is plain filesystem content:
1. Call `DesignArtifact.create` to create the artifact and get `manifest.root`.
2. Use normal Write/Edit tools to write files under `<manifest.root>/current/...`. `manifest.root` is normally `<workspace>/.design/<artifact_id>`; do not write DesignArtifact source files under `outputs/designs`.
3. Call `DesignArtifact.sync` with `artifact_id` and optional `entry` to scan current files, update manifest.files, and refresh the Canvas.
4. Call `DesignArtifact.snapshot` at meaningful save points.

For model-authored work, do not send substantial HTML/CSS/JS through DesignArtifact parameters. Use Write/Edit for file content, then sync."#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "create", "sync", "update_file", "delete_file", "set_entry", "snapshot",
                        "acquire_lock", "release_lock", "set_thumbnail", "zip_export",
                        "archive", "get", "list"
                    ],
                    "description": "Which operation to perform on the design artifact."
                },
                "expected_version": { "type": "string", "description": "Optional rebase guard: fail if current_version differs from this value (update_file/delete_file)." },
                "as": { "type": "string", "description": "Actor id for lock-aware writes: usually 'agent' or 'human'." },
                "force": { "type": "boolean", "description": "Override editing lock / existing lock (update_file/delete_file/acquire_lock)." },
                "holder": { "type": "string", "description": "Lock holder id for acquire_lock (default 'agent')." },
                "note": { "type": "string", "description": "Optional lock note (e.g. which tool call is writing)." },
                "data_url": { "type": "string", "description": "Base64 data URL for set_thumbnail (data:image/png;base64,...)." },
                "unarchive": { "type": "boolean", "description": "If true, clear archived_at instead of setting it (archive)." },
                "artifact_id": { "type": "string", "description": "Target artifact id (required for all actions except create/list)." },
                "id": { "type": "string", "description": "Optional deterministic id for create; otherwise derived from title." },
                "title": { "type": "string", "description": "Human-readable artifact title (create)." },
                "kind": { "type": "string", "description": "Artifact kind: page | component | flow | system (create, default page)." },
                "entry": { "type": "string", "description": "Entry HTML path relative to current/ (create/sync/set_entry)." },
                "viewports": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Preview viewports, e.g. [\"desktop\",\"tablet\",\"mobile\"] (create)."
                },
                "files": {
                    "type": "object",
                    "additionalProperties": { "type": "string" },
                    "description": "Initial files map (relative path -> content) for create. Keep this tiny: use a minimal scaffold only, with a small number of small files. Do not send a full prototype here."
                },
                "path": { "type": "string", "description": "File path relative to the artifact root (update_file/delete_file)." },
                "content": { "type": "string", "description": "File content (update_file). Frontend/editor compatibility path; agents should prefer normal Write/Edit to `<manifest.root>/current/...` followed by sync." },
                "summary": { "type": "string", "description": "Short intent summary for snapshot." },
                "author": { "type": "string", "description": "Who authored the change — agent|human (snapshot)." }
            }
        })
    }

    async fn input_schema_for_model(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "sync", "snapshot", "get", "list"],
                    "description": "Use create to make the artifact, sync after normal Write/Edit file writes, snapshot at save points, get/list to inspect."
                },
                "artifact_id": { "type": "string", "description": "Target artifact id (sync/snapshot/get)." },
                "id": { "type": "string", "description": "Optional deterministic id for create; otherwise derived from title." },
                "title": { "type": "string", "description": "Human-readable artifact title (create)." },
                "kind": { "type": "string", "description": "Artifact kind: page | component | flow | system (create, default page)." },
                "entry": { "type": "string", "description": "Entry HTML path relative to current/ (create/sync)." },
                "viewports": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Preview viewports, e.g. [\"desktop\",\"tablet\",\"mobile\"] (create)."
                },
                "summary": { "type": "string", "description": "Short intent summary for snapshot." },
                "author": { "type": "string", "description": "Who authored the change, usually agent (snapshot)." }
            }
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn user_facing_name(&self) -> String {
        "Design Artifact".to_string()
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("");
        if action.is_empty() {
            return ValidationResult {
                result: false,
                message: Some("action is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }
        match action {
            "create" => {
                let result = Self::validate_create_payload(input);
                if !result.result {
                    return result;
                }
            }
            "update_file" => {
                let result = Self::validate_update_payload(input);
                if !result.result {
                    return result;
                }
            }
            _ => {}
        }
        ValidationResult::default()
    }

    fn render_tool_use_message(
        &self,
        input: &Value,
        _options: &crate::agentic::tools::framework::ToolRenderOptions,
    ) -> String {
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("?");
        let id = input
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .or_else(|| input.get("id").and_then(|v| v.as_str()))
            .or_else(|| input.get("title").and_then(|v| v.as_str()))
            .unwrap_or("");
        if id.is_empty() {
            format!("DesignArtifact.{}", action)
        } else {
            format!("DesignArtifact.{} — {}", action, id)
        }
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        let event = output
            .get("artifact_event")
            .and_then(|v| v.as_str())
            .unwrap_or("ok");
        let id = output
            .get("manifest")
            .and_then(|m| m.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        format!(
            "Design artifact '{}' updated ({}). The Design Canvas tab has been opened in the right panel.",
            id, event
        )
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("DesignArtifact: action is required"))?
            .to_string();

        let (event, payload): (&str, Value) = match action.as_str() {
            "create" => {
                let manifest = self.handle_create(input, context).await?;
                (
                    "created",
                    json!({ "success": true, "artifact_event": "created", "manifest": manifest }),
                )
            }
            "sync" => {
                let manifest = self.handle_sync(input, context).await?;
                (
                    "file-changed",
                    json!({ "success": true, "artifact_event": "file-changed", "manifest": manifest }),
                )
            }
            "update_file" => {
                let manifest = self.handle_update_file(input, context).await?;
                (
                    "file-changed",
                    json!({ "success": true, "artifact_event": "file-changed", "manifest": manifest }),
                )
            }
            "delete_file" => {
                let manifest = self.handle_delete_file(input, context).await?;
                (
                    "file-removed",
                    json!({ "success": true, "artifact_event": "file-removed", "manifest": manifest }),
                )
            }
            "set_entry" => {
                let manifest = self.handle_set_entry(input, context).await?;
                (
                    "manifest-updated",
                    json!({ "success": true, "artifact_event": "manifest-updated", "manifest": manifest }),
                )
            }
            "snapshot" => {
                let manifest = self.handle_snapshot(input, context).await?;
                (
                    "snapshot-committed",
                    json!({ "success": true, "artifact_event": "snapshot-committed", "manifest": manifest }),
                )
            }
            "acquire_lock" => {
                let manifest = self.handle_acquire_lock(input, context).await?;
                (
                    "lock-acquired",
                    json!({ "success": true, "artifact_event": "lock-acquired", "manifest": manifest }),
                )
            }
            "release_lock" => {
                let manifest = self.handle_release_lock(input, context).await?;
                (
                    "lock-released",
                    json!({ "success": true, "artifact_event": "lock-released", "manifest": manifest }),
                )
            }
            "set_thumbnail" => {
                let manifest = self.handle_set_thumbnail(input, context).await?;
                (
                    "thumbnail-updated",
                    json!({ "success": true, "artifact_event": "thumbnail-updated", "manifest": manifest }),
                )
            }
            "zip_export" => {
                let (manifest, zip_path) = self.handle_zip_export(input, context).await?;
                (
                    "exported",
                    json!({
                        "success": true,
                        "artifact_event": "exported",
                        "manifest": manifest,
                        "export_path": zip_path.to_string_lossy(),
                    }),
                )
            }
            "archive" => {
                let manifest = self.handle_archive(input, context).await?;
                (
                    "archived",
                    json!({ "success": true, "artifact_event": "archived", "manifest": manifest }),
                )
            }
            "get" => {
                let manifest = self.handle_get(input, context).await?;
                (
                    "ok",
                    json!({ "success": true, "artifact_event": "ok", "manifest": manifest }),
                )
            }
            "list" => {
                let manifests = self.handle_list(context).await?;
                (
                    "listed",
                    json!({ "success": true, "artifact_event": "listed", "manifests": manifests }),
                )
            }
            other => {
                return Err(BitFunError::tool(format!(
                    "DesignArtifact: unknown action '{}'",
                    other
                )));
            }
        };

        let assistant_text = if action == "list" {
            let count = payload
                .get("manifests")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            format!("Listed {} design artifact(s).", count)
        } else {
            let id = payload
                .get("manifest")
                .and_then(|m| m.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!(
                "Design artifact '{}' {}. Users can view and edit it in the right-side Design Canvas tab.",
                id, event
            )
        };

        Ok(vec![ToolResult::Result {
            data: payload,
            result_for_assistant: Some(assistant_text),
            image_attachments: None,
        }])
    }
}
