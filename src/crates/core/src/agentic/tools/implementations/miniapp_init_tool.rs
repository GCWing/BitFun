//! InitMiniApp tool — create a new MiniApp skeleton; AI then uses generic file tools to edit.

use crate::agentic::tools::framework::{Tool, ToolExposure, ToolResult, ToolUseContext};
use crate::infrastructure::events::{emit_global_event, BackendEvent};
use crate::miniapp::try_get_global_miniapp_manager;
use crate::miniapp::types::{
    FsPermissions, MiniAppPermissions, MiniAppSource, NetPermissions, ShellPermissions,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

const SKELETON_HTML: &str = r#"<!DOCTYPE html>
<html data-theme-type="dark">
<head><meta charset="utf-8"></head>
<body>
  <div id="app"></div>
</body>
</html>"#;

const SKELETON_UI_JS: &str = r#"// ESM module — use import, not require. Example:
// import React from 'react';
// const files = await app.fs.readdir('.');
// document.getElementById('app').textContent = JSON.stringify(files, null, 2);
"#;

const SKELETON_WORKER_JS: &str = r#"// Node.js Worker — export methods callable via app.call('methodName', params).
// module.exports = {
//   async 'myMethod'(params) { return { result: 'ok' }; },
// };
"#;

const SKELETON_CSS: &str = r#"/* MiniApp skeleton — uses host theme via --bitfun-* variables */
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  font-family: var(--bitfun-font-sans, -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Hiragino Sans GB', 'Segoe UI', 'Microsoft YaHei UI', 'Microsoft YaHei', 'Helvetica Neue', Helvetica, Arial, sans-serif);
  font-size: 13px;
  color: var(--bitfun-text, #e8e8e8);
  background: var(--bitfun-bg, #121214);
  min-height: 100vh;
}
#app { min-height: 100vh; }
"#;

pub struct InitMiniAppTool;

impl InitMiniAppTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InitMiniAppTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for InitMiniAppTool {
    fn name(&self) -> &str {
        "InitMiniApp"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Create a new MiniApp skeleton in the Toolbox. After creation, use Read/Write/Edit file tools to modify the source files directly.

Input: name, description, icon, category. The tool creates the app directory and skeleton files:
- manifest (meta.json), source/index.html, source/style.css, source/ui.js, source/worker.js,
  package.json, storage.json.

Returns app_id and the app root directory. Use the root directory and file names above with Read/Write/Edit to implement the app. The MiniApp uses window.app (app.fs, app.call, app.dialog, etc.) — see miniapp-dev skill for API reference."#
            .to_string())
    }

    fn short_description(&self) -> String {
        "Create a new MiniApp skeleton in the Toolbox.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Collapsed
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["name"],
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Short app name (e.g. 'Image Compressor', 'Markdown Viewer')"
                },
                "description": {
                    "type": "string",
                    "description": "One-sentence description. Default empty."
                },
                "icon": {
                    "type": "string",
                    "description": "Emoji or icon identifier. Default '📦'."
                },
                "category": {
                    "type": "string",
                    "description": "Category: utility, media, dev, productivity. Default 'utility'."
                }
            }
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let manager = try_get_global_miniapp_manager()
            .ok_or_else(|| BitFunError::tool("MiniAppManager not initialized".to_string()))?;

        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::validation("Missing required field: name"))?
            .to_string();
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let icon = input
            .get("icon")
            .and_then(|v| v.as_str())
            .unwrap_or("📦")
            .to_string();
        let category = input
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("utility")
            .to_string();

        let source = MiniAppSource {
            html: SKELETON_HTML.to_string(),
            css: SKELETON_CSS.to_string(),
            ui_js: SKELETON_UI_JS.to_string(),
            esm_dependencies: Vec::new(),
            worker_js: SKELETON_WORKER_JS.to_string(),
            npm_dependencies: Vec::new(),
        };

        let permissions = MiniAppPermissions {
            fs: Some(FsPermissions {
                read: Some(vec!["{appdata}".to_string(), "{workspace}".to_string()]),
                write: Some(vec!["{appdata}".to_string()]),
            }),
            shell: Some(ShellPermissions {
                allow: Some(Vec::new()),
            }),
            net: Some(NetPermissions {
                allow: Some(vec!["*".to_string()]),
            }),
            node: None,
            ai: None,
        };

        let app = manager
            .create(
                name.clone(),
                description,
                icon,
                category,
                Vec::new(),
                source,
                permissions,
                None,
                context.workspace_root(),
            )
            .await
            .map_err(|e| BitFunError::tool(format!("Failed to create MiniApp: {}", e)))?;

        let path_manager = manager.path_manager();
        let app_dir = path_manager.miniapp_dir(&app.id);
        let app_dir_str = app_dir.to_string_lossy().to_string();
        let source_dir = app_dir.join("source");

        let files = json!({
            "manifest": app_dir.join("meta.json").to_string_lossy(),
            "ui": source_dir.join("ui.js").to_string_lossy(),
            "worker": source_dir.join("worker.js").to_string_lossy(),
            "style": source_dir.join("style.css").to_string_lossy(),
            "html": source_dir.join("index.html").to_string_lossy(),
            "package": app_dir.join("package.json").to_string_lossy(),
        });

        let _ = emit_global_event(BackendEvent::Custom {
            event_name: "miniapp-created".to_string(),
            payload: json!({ "id": app.id, "name": app.name }),
        })
        .await;

        let result_text = format!(
            "MiniApp '{}' skeleton created. app_id: {}. Root directory: {}. Use Read/Write/Edit tools with files under this root, then open in Toolbox to run.",
            app.name, app.id, app_dir_str
        );

        Ok(vec![ToolResult::Result {
            data: json!({
                "app_id": app.id,
                "path": app_dir_str,
                "files": files,
            }),
            result_for_assistant: Some(result_text),
            image_attachments: None,
        }])
    }
}
