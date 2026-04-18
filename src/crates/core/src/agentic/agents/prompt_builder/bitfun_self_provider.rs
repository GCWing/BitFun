//! BitFun self-introspection prompt section.
//!
//! Builds the markdown block injected into the system prompt at
//! the `{BITFUN_SELF}` placeholder. The goal is to make BitFun's own
//! capabilities (scenes, settings tabs, installed mini-apps) discoverable
//! to the model with **zero tool calls**, so it never falls back to
//! `Bash ls` against the user workspace when asked "what mini-apps do I
//! have / what scenes are there / what can BitFun do".
//!
//! Refresh strategy: regenerated every time a system prompt is built. The
//! mini-app manager's `list()` is a cheap in-memory + metadata read, so
//! there is no caching layer to invalidate. Anything newly installed is
//! visible to the model on the next prompt rebuild without bookkeeping.

use std::fmt::Write as _;

/// Build the BitFun self-introspection prompt block. Returns an empty
/// string if there is nothing useful to say (e.g. mini-app subsystem not
/// initialized AND no extra context to surface) — callers should treat
/// `""` as "skip the section".
pub async fn build_bitfun_self_prompt() -> String {
    let mut out = String::new();
    out.push_str("# BitFun Self Capabilities (you are running INSIDE this app)\n");
    out.push_str(
        "When the user asks \"what mini-apps are installed / what scenes are there / how do I use BitFun\", \
use ControlHub `domain: \"app\"` actions FIRST. Do NOT answer those questions by listing the workspace directory \
— workspace folders belong to the user, not to BitFun's own catalog.\n\n",
    );

    push_scene_catalog(&mut out);
    push_settings_tab_catalog(&mut out);
    push_miniapp_section(&mut out).await;

    out.push_str(
        "\n## Quick recipes\n\
- \"列一下小应用 / what mini-apps do I have\" → `ControlHub { domain: \"app\", action: \"list_miniapps\" }`.\n\
- \"打开小应用 X\" → `ControlHub { domain: \"app\", action: \"execute_task\", params: { task: \"open_miniapp\", params: { miniAppId: \"<id>\" } } }`.\n\
- \"打开小应用市场 / show the gallery\" → `ControlHub { domain: \"app\", action: \"execute_task\", params: { task: \"open_miniapp_gallery\" } }`.\n\
- \"BitFun 都能干啥 / 一次列出所有能力\" → `ControlHub { domain: \"app\", action: \"app_self_describe\" }`.\n",
    );

    out
}

fn push_scene_catalog(out: &mut String) {
    out.push_str("## Available scenes (pass `id` to `open_scene`)\n");
    for (id, label_en, label_zh) in scene_catalog() {
        let _ = writeln!(out, "- `{id}` — {label_en} / {label_zh}");
    }
    out.push_str("- `miniapp:<appId>` — opens a specific installed mini-app (use list_miniapps to find ids).\n\n");
}

fn push_settings_tab_catalog(out: &mut String) {
    out.push_str("## Settings tabs (pass `tabId` to `open_settings_tab`)\n");
    for (id, desc) in settings_tab_catalog() {
        let _ = writeln!(out, "- `{id}` — {desc}");
    }
    out.push('\n');
}

async fn push_miniapp_section(out: &mut String) {
    out.push_str("## Installed mini-apps\n");
    let manager = match crate::miniapp::try_get_global_miniapp_manager() {
        Some(m) => m,
        None => {
            out.push_str("(Mini-app subsystem is not initialized in this build.)\n");
            return;
        }
    };

    let metas = match manager.list().await {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(out, "(Failed to enumerate mini-apps: {e})");
            return;
        }
    };

    if metas.is_empty() {
        out.push_str("(No mini-apps installed yet. The user can install some from the gallery scene `miniapps`.)\n");
        return;
    }

    let _ = writeln!(out, "({} installed)", metas.len());
    for meta in metas.iter().take(40) {
        let desc = if meta.description.is_empty() {
            "(no description)"
        } else {
            meta.description.as_str()
        };
        let _ = writeln!(
            out,
            "- `{}` — {} — {} (open via `execute_task open_miniapp miniAppId=\"{}\"`)",
            meta.id, meta.name, desc, meta.id
        );
    }
    if metas.len() > 40 {
        let _ = writeln!(
            out,
            "- … {} more (call `list_miniapps` to enumerate the rest).",
            metas.len() - 40
        );
    }
}

// NOTE: these two catalogs MUST stay aligned with the Rust copies in
// `control_hub_tool.rs::scene_catalog` / `settings_tab_catalog` and the
// frontend registries (`scenes/registry.ts`, settings store). The e2e
// suite already validates the `list_tasks` catalog; extend it to cover
// these as well when adding new entries.
fn scene_catalog() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("welcome", "Welcome", "欢迎使用"),
        ("session", "Session (chat)", "会话"),
        ("terminal", "Terminal", "终端"),
        ("git", "Git", "Git"),
        ("settings", "Settings", "设置"),
        ("file-viewer", "File Viewer", "文件查看"),
        ("profile", "Profile", "个人资料"),
        ("agents", "Agents", "智能体"),
        ("skills", "Skills", "技能"),
        ("miniapps", "Mini App Gallery", "小应用"),
        ("browser", "Browser", "浏览器"),
        ("mermaid", "Mermaid Editor", "Mermaid 图表"),
        ("assistant", "Assistant", "助理"),
        ("insights", "Insights", "洞察"),
        ("shell", "Shell", "Shell"),
        ("panel-view", "Panel View", "面板视图"),
    ]
}

fn settings_tab_catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("basics", "Basic preferences (language, theme, etc.)"),
        ("models", "AI models (add / edit / set defaults / delete)"),
        ("session-config", "Default session behavior"),
        ("agents", "Agent management"),
        ("skills", "Skill packages"),
        ("tools", "Built-in tools and MCP servers"),
        ("about", "About BitFun"),
    ]
}
