//! GenerativeUI tool — renders LLM-generated HTML/SVG widgets.

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::service::config::get_global_config_service;
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct GenerativeUITool;

const LARGE_WIDGET_CODE_SOFT_LINE_LIMIT: usize = 260;
const LARGE_WIDGET_CODE_SOFT_BYTE_LIMIT: usize = 28 * 1024;

struct ThemePromptSnapshot {
    id: &'static str,
    theme_type: &'static str,
    bg_primary: &'static str,
    bg_secondary: &'static str,
    bg_scene: &'static str,
    text_primary: &'static str,
    text_muted: &'static str,
    accent_500: &'static str,
    accent_600: &'static str,
    border_base: &'static str,
    element_base: &'static str,
    radius_base: &'static str,
    spacing_4: &'static str,
    shadow_base: &'static str,
    style_notes: &'static str,
}

impl GenerativeUITool {
    pub fn new() -> Self {
        Self
    }

    fn architecture_widget_reminder() -> &'static str {
        "Architecture/codebase widget reminder: if the widget is a repo map, README architecture view, or module diagram, clickable nodes must carry verified file metadata on the clickable element itself. Use `data-file-path` for a REAL existing file and `data-line` for the exact definition line when the node represents code. Do not attach file metadata to abstract grouping nodes, package containers, or directories. If a node is conceptual or cannot be verified, leave it non-clickable."
    }

    fn bitfun_design_system_reminder() -> &'static str {
        "BitFun design-system reminder: when the widget should feel native to the host BitFun app, style it with BitFun theme tokens instead of hard-coded design values. Prefer CSS variables such as `var(--color-bg-primary)`, `var(--color-bg-secondary)`, `var(--color-bg-scene)`, `var(--color-bg-elevated)`, `var(--color-text-primary)`, `var(--color-text-secondary)`, `var(--color-text-muted)`, `var(--color-accent-500)`, `var(--color-accent-600)`, `var(--border-subtle)`, `var(--border-base)`, `var(--border-medium)`, `var(--element-bg-subtle)`, `var(--element-bg-soft)`, `var(--element-bg-base)`, `var(--element-bg-medium)`, `var(--shadow-*)`, `var(--radius-*)`, `var(--spacing-*)`, `var(--motion-*)`, `var(--easing-*)`, `var(--font-sans)`, and `var(--font-mono)`. Support both `bitfun-dark` and `bitfun-light`; do not assume dark-only, purple-only, or landing-page styling. Favor compact desktop workbench layouts, panel/card surfaces, strong information hierarchy, and reusable BitFun component patterns. Avoid hard-coded colors, arbitrary spacing, giant hero sections, fake mobile chrome, and full marketing-page shells; prefer understated, premium UI with layered surfaces, restrained contrast, subtle borders, and do not use thick left-accent emphasis blocks."
    }

    fn bitfun_widget_scaffold_reminder() -> &'static str {
        "BitFun widget scaffold reminder: the host iframe already provides reusable utility classes. Prefer these host classes before inventing a new visual language: `bf-root`, `bf-stack`, `bf-row`, `bf-row-wrap`, `bf-toolbar`, `bf-section`, `bf-section-header`, `bf-title`, `bf-subtitle`, `bf-eyebrow`, `bf-card`, `bf-panel`, `bf-card-accent`, `bf-grid`, `bf-kpi`, `bf-kpi-label`, `bf-kpi-value`, `bf-kpi-meta`, `bf-badge`, `bf-badge-accent`, `bf-badge-success`, `bf-badge-warning`, `bf-badge-error`, `bf-button`, `bf-button-primary`, `bf-input`, `bf-textarea`, `bf-select`, `bf-list`, `bf-list-item`, `bf-table-wrap`, `bf-table`, `bf-empty`, `bf-divider`, `bf-code`, and `bf-mono`. Generate markup that composes these classes first, and only add small local CSS when the scaffold is insufficient."
    }

    fn combined_reminder() -> String {
        format!(
            "{} {} {}",
            Self::architecture_widget_reminder(),
            Self::bitfun_design_system_reminder(),
            Self::bitfun_widget_scaffold_reminder()
        )
    }

    fn builtin_theme_snapshot(theme_id: &str) -> Option<ThemePromptSnapshot> {
        match theme_id {
            "bitfun-dark" => Some(ThemePromptSnapshot {
                id: "bitfun-dark",
                theme_type: "dark",
                bg_primary: "#0e0e10",
                bg_secondary: "#1c1c1f",
                bg_scene: "#1c1c1f",
                text_primary: "#e8e8e8",
                text_muted: "#858585",
                accent_500: "#60a5fa",
                accent_600: "#3b82f6",
                border_base: "rgba(255, 255, 255, 0.18)",
                element_base: "rgba(255, 255, 255, 0.095)",
                radius_base: "8px",
                spacing_4: "16px",
                shadow_base: "0 4px 8px rgba(0, 0, 0, 0.7)",
                style_notes: "neutral dark workbench, low-chroma surfaces, blue accent used sparingly",
            }),
            "bitfun-light" => Some(ThemePromptSnapshot {
                id: "bitfun-light",
                theme_type: "light",
                bg_primary: "#f3f3f5",
                bg_secondary: "#ffffff",
                bg_scene: "#ffffff",
                text_primary: "#1e293b",
                text_muted: "#64748b",
                accent_500: "#64748b",
                accent_600: "#475569",
                border_base: "rgba(100, 116, 139, 0.22)",
                element_base: "rgba(15, 23, 42, 0.09)",
                radius_base: "8px",
                spacing_4: "16px",
                shadow_base: "0 4px 8px rgba(71, 85, 105, 0.10)",
                style_notes: "neutral light workbench, soft gray chrome, restrained contrast, no glossy marketing feel",
            }),
            "bitfun-slate" => Some(ThemePromptSnapshot {
                id: "bitfun-slate",
                theme_type: "dark",
                bg_primary: "#14161a",
                bg_secondary: "#22262c",
                bg_scene: "#22262c",
                text_primary: "#eef0f3",
                text_muted: "#9ea4ab",
                accent_500: "#94a3b8",
                accent_600: "#64748b",
                border_base: "rgba(255, 255, 255, 0.18)",
                element_base: "rgba(255, 255, 255, 0.095)",
                radius_base: "6px",
                spacing_4: "16px",
                shadow_base: "0 4px 8px rgba(0, 0, 0, 0.75)",
                style_notes: "cool gray geometric chrome, crisp edges, restrained accent, dense desktop mood",
            }),
            "bitfun-midnight" => Some(ThemePromptSnapshot {
                id: "bitfun-midnight",
                theme_type: "dark",
                bg_primary: "#2b2d30",
                bg_secondary: "#1e1f22",
                bg_scene: "#27292c",
                text_primary: "#bcbec4",
                text_muted: "#6f737a",
                accent_500: "#58a6ff",
                accent_600: "#3b82f6",
                border_base: "rgba(255, 255, 255, 0.14)",
                element_base: "rgba(255, 255, 255, 0.09)",
                radius_base: "8px",
                spacing_4: "16px",
                shadow_base: "0 4px 8px rgba(0, 0, 0, 0.7)",
                style_notes: "IDE-like dark gray theme, professional, sober, subtle blue focus accents",
            }),
            "bitfun-cyber" => Some(ThemePromptSnapshot {
                id: "bitfun-cyber",
                theme_type: "dark",
                bg_primary: "#101010",
                bg_secondary: "#151515",
                bg_scene: "#141414",
                text_primary: "#e0f2ff",
                text_muted: "#7fadcc",
                accent_500: "#00e6ff",
                accent_600: "#00ccff",
                border_base: "rgba(0, 230, 255, 0.20)",
                element_base: "rgba(0, 230, 255, 0.13)",
                radius_base: "6px",
                spacing_4: "16px",
                shadow_base: "0 4px 12px rgba(0, 0, 0, 0.8)",
                style_notes: "neon cyber tooling, black surfaces, glowing cyan accents, still compact and workbench-first",
            }),
            "bitfun-china-style" => Some(ThemePromptSnapshot {
                id: "bitfun-china-style",
                theme_type: "light",
                bg_primary: "#faf8f0",
                bg_secondary: "#f5f3e8",
                bg_scene: "#fdfcf6",
                text_primary: "#1a1a1a",
                text_muted: "#6a6a6a",
                accent_500: "#2e5e8a",
                accent_600: "#234a6d",
                border_base: "rgba(106, 92, 70, 0.20)",
                element_base: "rgba(46, 94, 138, 0.10)",
                radius_base: "6px",
                spacing_4: "16px",
                shadow_base: "0 4px 8px rgba(106, 92, 70, 0.1)",
                style_notes: "warm rice-paper surfaces, ink-and-blue accenting, elegant and restrained",
            }),
            "bitfun-china-night" => Some(ThemePromptSnapshot {
                id: "bitfun-china-night",
                theme_type: "dark",
                bg_primary: "#1a1814",
                bg_secondary: "#212019",
                bg_scene: "#1e1c17",
                text_primary: "#e8e6e1",
                text_muted: "#928f89",
                accent_500: "#73a5cc",
                accent_600: "#5a8bb3",
                border_base: "rgba(232, 230, 225, 0.16)",
                element_base: "rgba(115, 165, 204, 0.12)",
                radius_base: "6px",
                spacing_4: "16px",
                shadow_base: "0 4px 8px rgba(0, 0, 0, 0.65)",
                style_notes: "warm ink-night dark palette, calm contrast, blue-green highlights, elegant not flashy",
            }),
            _ => None,
        }
    }

    fn format_theme_snapshot(snapshot: &ThemePromptSnapshot) -> String {
        format!(
            "{} ({}) => bg.primary={}, bg.secondary={}, bg.scene={}, text.primary={}, text.muted={}, accent.500={}, accent.600={}, border.base={}, element.base={}, radius.base={}, spacing.4={}, shadow.base={}, style={}",
            snapshot.id,
            snapshot.theme_type,
            snapshot.bg_primary,
            snapshot.bg_secondary,
            snapshot.bg_scene,
            snapshot.text_primary,
            snapshot.text_muted,
            snapshot.accent_500,
            snapshot.accent_600,
            snapshot.border_base,
            snapshot.element_base,
            snapshot.radius_base,
            snapshot.spacing_4,
            snapshot.shadow_base,
            snapshot.style_notes
        )
    }

    fn baseline_theme_context() -> String {
        let dark = Self::builtin_theme_snapshot("bitfun-dark")
            .map(|snapshot| Self::format_theme_snapshot(&snapshot))
            .unwrap_or_default();
        let light = Self::builtin_theme_snapshot("bitfun-light")
            .map(|snapshot| Self::format_theme_snapshot(&snapshot))
            .unwrap_or_default();
        format!(
            "Cross-theme baseline: {}. {}. Widgets must remain correct in both themes by default.",
            dark, light
        )
    }

    async fn build_theme_prompt_context(&self) -> Option<String> {
        let config_service = get_global_config_service().await.ok()?;
        let selected_theme_id = config_service
            .get_config::<String>(Some("themes.current"))
            .await
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "bitfun-light".to_string());

        if selected_theme_id == "system" {
            return Some(format!(
                "BitFun active theme selection: system. Exact runtime resolution is host-dependent, so do not assume one palette. {}",
                Self::baseline_theme_context()
            ));
        }

        if let Some(snapshot) = Self::builtin_theme_snapshot(&selected_theme_id) {
            return Some(format!(
                "BitFun active theme snapshot: {}. {}",
                Self::format_theme_snapshot(&snapshot),
                Self::baseline_theme_context()
            ));
        }

        Some(format!(
            "BitFun active theme selection: {}. Backend does not have an exact built-in snapshot for this theme, so use BitFun CSS variables strictly and avoid hard-coded fallback palettes. {}",
            selected_theme_id,
            Self::baseline_theme_context()
        ))
    }
}

impl Default for GenerativeUITool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GenerativeUITool {
    fn name(&self) -> &str {
        "GenerativeUI"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(format!(
            r#"Use GenerativeUI to render visual HTML or SVG content.

Use this when the user asks for visual or interactive output such as:
- charts, dashboards, tables
- explainers with sliders or controls
- diagrams, mockups, or small simulations
- SVG illustrations

Input rules:
1. Put the widget code in `widget_code`.
2. For HTML, provide a raw fragment only. Do NOT include Markdown fences, <!DOCTYPE>, <html>, <head>, or <body>.
3. For SVG, provide raw SVG starting with <svg>.
4. Put CSS first, then HTML, then scripts last so the preview can stream progressively.
5. Keep the first useful content visible early. Avoid giant style blocks.
6. Prefer self-contained widgets. CDN scripts are allowed when needed, but keep them minimal.
6a. Keep `widget_code` compact. The 260-line / 28KB guideline is a soft reliability threshold, not a hard cap. If the widget is larger, reduce repeated static DOM with data-driven loops, shared CSS classes, and simpler markup rather than truncating required behavior.
7. If the user only needs text, do not use this tool.
8. Prefer compact, scroll-light layouts. Avoid large CSS resets, fixed overlays, oversized app chrome, and nested scrolling.
9. IMPORTANT sizing rule: the default target is an inline FlowChat card, not a full browser page. Build responsive widgets that fit a narrow card without horizontal scrolling.
10. Use fluid sizing: `width: 100%`, `max-width: 100%`, responsive grids, wrapped controls, and charts that shrink to their container. Do not rely on fixed pixel widths, `min-width` hacks, wide tables, or page-sized canvases.
11. Keep the widget focused. Prefer one clear visual or one small interactive tool.
12. If the widget needs follow-up reasoning, use `sendPrompt('...')` from inside the widget.
13. Do not invent custom desktop bridge APIs such as `window.app.call(...)` for file opening inside widgets.
14. Do not use `parent.postMessage(...)` or custom `onclick` protocols for file opening when `data-file-path` can be attached directly to the clickable element.
15. CRITICAL for codebase maps, repo overviews, and architecture diagrams: NEVER guess or invent paths. Every clickable `data-file-path` MUST point to a REAL file that exists in the workspace.
16. For clickable file navigation, add `data-file-path` on the clickable element itself, and add `data-line` for the exact definition or anchor line whenever the node represents code.
17. `data-file-path` may be workspace-relative such as `src/crates/core/src/lib.rs`, or absolute when already verified, but it MUST resolve to a file, not a directory.
18. Do NOT attach `data-file-path` to abstract grouping nodes such as "Core", "Frontend", "Agent System", or module containers unless that node intentionally opens one specific real file.
19. For codebase architecture diagrams, prefer one clickable node per concrete file. If a node represents a broader concept, package, or directory, leave it non-clickable instead of pointing it at a folder.
20. Workflow for architecture widgets: first verify candidate files with Glob or LS, then use Read with line numbers when needed, and only then emit clickable nodes with verified file paths and lines.
21. If you cannot verify the exact file path and line number, do not make that node clickable. Better to have fewer accurate links than many broken ones.
22. If the user asks for click-to-open files, do not build a details-only interaction with `data-key` and `onclick="showDetail(...)"` unless the clickable node also carries its own `data-file-path`.
23. Do not put one `data-file-path` on a large wrapper that contains multiple visual nodes. The actual clickable node must own the path metadata.
24. Make clickable nodes look clickable with visible grouping, spacing, and hover feedback instead of producing a static poster.
25. For charts, give charts a fixed-height wrapper and keep legends or summary numbers outside the canvas when possible.
26. For mockups, use compact spacing and clear hierarchy. Avoid building full app chrome unless the chrome itself is the point.
27. For lightweight generative art, prefer SVG and keep the output deterministic and performant.
28. If the widget is meant to match BitFun's product UI, apply these reminders strictly: {} {}"#,
            Self::bitfun_design_system_reminder(),
            Self::bitfun_widget_scaffold_reminder()
        ))
    }

    async fn description_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        let mut description = self.description().await?;
        if let Some(theme_context) = self.build_theme_prompt_context().await {
            description.push_str("\n\n");
            description.push_str(&theme_context);
        }
        Ok(description)
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "description": format!(
                "Render a compact HTML/SVG widget. {}",
                Self::combined_reminder()
            ),
            "required": ["title", "widget_code"],
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short widget title, for example 'compound interest simulator' or 'latency dashboard'."
                },
                "widget_code": {
                    "type": "string",
                    "description": format!(
                        "Raw HTML fragment or raw SVG. No Markdown code fences. For HTML: no <!DOCTYPE>, <html>, <head>, or <body>. The 260-line / 28KB guideline is a soft reliability threshold. For larger widgets, use data-driven loops, shared CSS classes, and simpler markup rather than truncating required behavior. {} If the widget should match BitFun, rely on the host CSS variables instead of hard-coded colors or spacing. If the user asked for file navigation, do not finish this field until each clickable node has verified file metadata or is intentionally non-clickable.",
                        Self::combined_reminder()
                    )
                },
                "width": {
                    "type": "integer",
                    "minimum": 240,
                    "maximum": 1600,
                    "description": "Preferred width in pixels for enlarged panel view. Optional. Do not rely on this for inline card layout; the widget itself must remain responsive and fit narrow containers without horizontal scrolling."
                },
                "height": {
                    "type": "integer",
                    "minimum": 160,
                    "maximum": 1600,
                    "description": "Preferred height in pixels for enlarged panel view. Optional."
                },
                "modules": {
                    "type": "array",
                    "description": "Optional guidance tags such as interactive, chart, mockup, art, diagram, architecture, or repo-map. If this includes architecture/repo-map/diagram, apply the architecture widget reminder strictly.",
                    "items": {
                        "type": "string"
                    }
                }
            }
        })
    }

    async fn input_schema_for_model_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> Value {
        let mut schema = self.input_schema();
        let theme_context = self.build_theme_prompt_context().await;
        if let Some(obj) = schema.as_object_mut() {
            obj.insert(
                "x-bitfun-reminder".to_string(),
                Value::String(Self::combined_reminder()),
            );
            obj.insert(
                "x-bitfun-design-system".to_string(),
                Value::String(Self::bitfun_design_system_reminder().to_string()),
            );
            if let Some(theme_context) = theme_context {
                obj.insert(
                    "x-bitfun-theme-context".to_string(),
                    Value::String(theme_context.clone()),
                );
                if let Some(description) = obj
                    .get_mut("description")
                    .and_then(|value| value.as_str().map(str::to_string))
                {
                    obj.insert(
                        "description".to_string(),
                        Value::String(format!("{} {}", description, theme_context)),
                    );
                }
            }
        }
        schema
    }

    fn user_facing_name(&self) -> String {
        "Generative UI".to_string()
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let title = match input.get("title").and_then(|v| v.as_str()) {
            Some(value) if !value.trim().is_empty() => value.trim(),
            _ => {
                return ValidationResult {
                    result: false,
                    message: Some("Missing or empty title".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        let widget_code = match input.get("widget_code").and_then(|v| v.as_str()) {
            Some(value) if !value.trim().is_empty() => value.trim(),
            _ => {
                return ValidationResult {
                    result: false,
                    message: Some("Missing or empty widget_code".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if title.len() > 120 {
            return ValidationResult {
                result: false,
                message: Some("title is too long; keep it under 120 characters".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if widget_code.starts_with("```") {
            return ValidationResult {
                result: false,
                message: Some(
                    "widget_code must be raw HTML or SVG, not Markdown code fences".to_string(),
                ),
                error_code: Some(400),
                meta: None,
            };
        }

        let line_count = widget_code.lines().count();
        let byte_count = widget_code.len();
        if line_count > LARGE_WIDGET_CODE_SOFT_LINE_LIMIT
            || byte_count > LARGE_WIDGET_CODE_SOFT_BYTE_LIMIT
        {
            return ValidationResult {
                result: true,
                message: Some(format!(
                    "Large GenerativeUI widget_code: {} lines, {} bytes. This is allowed when necessary, but prefer a staged design approach: keep the first version compact, use data-driven loops/shared classes, and iterate rather than emitting a huge static widget payload.",
                    line_count, byte_count
                )),
                error_code: None,
                meta: Some(json!({
                    "large_widget_code": true,
                    "line_count": line_count,
                    "byte_count": byte_count,
                    "soft_line_limit": LARGE_WIDGET_CODE_SOFT_LINE_LIMIT,
                    "soft_byte_limit": LARGE_WIDGET_CODE_SOFT_BYTE_LIMIT
                })),
            };
        }

        ValidationResult::default()
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        let title = output
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("widget");

        format!("Rendered widget preview '{}'.", title)
    }

    fn render_tool_use_message(
        &self,
        input: &Value,
        _options: &crate::agentic::tools::framework::ToolRenderOptions,
    ) -> String {
        let title = input
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("widget");
        format!("Rendering widget: {}", title)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let title = input
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Widget");
        let widget_code = input
            .get("widget_code")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let width = input.get("width").and_then(|v| v.as_i64()).unwrap_or(960);
        let height = input.get("height").and_then(|v| v.as_i64()).unwrap_or(640);
        let modules = input
            .get("modules")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let is_svg = widget_code.trim_start().starts_with("<svg");

        let widget_id = context
            .tool_call_id
            .clone()
            .unwrap_or_else(|| format!("widget_{}", chrono::Utc::now().timestamp_millis()));

        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "widget_id": widget_id,
                "title": title,
                "widget_code": widget_code,
                "width": width,
                "height": height,
                "is_svg": is_svg,
                "modules": modules,
            }),
            result_for_assistant: Some(format!(
                "Rendered widget '{}' inline in the FlowChat tool card.",
                title
            )),
            image_attachments: None,
        }])
    }
}
