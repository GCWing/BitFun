//! Mermaid interactive diagram tool
//!
//! Allows Agent to generate Mermaid diagrams with interactive features, supports node click navigation and highlight states

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::infrastructure::events::event_system::{get_global_event_system, BackendEvent};
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use chrono::Utc;
use log::debug;
use serde_json::{json, Value};

/// Mermaid interactive diagram tool
pub struct MermaidInteractiveTool;

impl MermaidInteractiveTool {
    pub fn new() -> Self {
        Self
    }

    /// Validate if Mermaid code is valid, returns validation result and error message
    fn validate_mermaid_code(&self, code: &str) -> (bool, Option<String>) {
        let trimmed = code.trim();

        // Check if empty
        if trimmed.is_empty() {
            return (false, Some("Mermaid code cannot be empty".to_string()));
        }

        // Check if starts with valid diagram type
        let valid_starters = vec![
            "graph ",
            "flowchart ",
            "sequenceDiagram",
            "classDiagram",
            "stateDiagram",
            "erDiagram",
            "gantt",
            "pie",
            "journey",
            "timeline",
            "mindmap",
            "gitgraph",
            "C4Context",
            "C4Container",
        ];

        let starts_with_valid = valid_starters
            .iter()
            .any(|starter| trimmed.starts_with(starter));

        if !starts_with_valid {
            return (false, Some(format!(
                "Mermaid code must start with a valid diagram type. Supported diagram types: graph, flowchart, sequenceDiagram, classDiagram, stateDiagram, erDiagram, gantt, pie, journey, timeline, mindmap, etc.\nCurrent code start: {}",
                if trimmed.len() > 50 { 
                    format!("{}...", &trimmed[..50]) 
                } else { 
                    trimmed.to_string() 
                }
            )));
        }

        // Check basic syntax structure
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() < 2 {
            return (false, Some("Mermaid code needs at least 2 lines (diagram type declaration and at least one node/relationship)".to_string()));
        }

        // Check if graph/flowchart has node definitions
        if trimmed.starts_with("graph ") || trimmed.starts_with("flowchart ") {
            // Check if there are arrows or node definitions
            let has_arrow =
                trimmed.contains("-->") || trimmed.contains("---") || trimmed.contains("==>");
            let has_node = trimmed.contains('[') || trimmed.contains('(') || trimmed.contains('{');

            if !has_arrow && !has_node {
                return (false, Some("Flowchart (graph/flowchart) must contain node definitions and connections. Example: A[Node] --> B[Node]".to_string()));
            }
        }

        // Check if sequenceDiagram has participants
        if trimmed.starts_with("sequenceDiagram") {
            if !trimmed.contains("participant")
                && !trimmed.contains("->>")
                && !trimmed.contains("-->>")
            {
                return (false, Some("Sequence diagram (sequenceDiagram) must contain participant definitions and interaction arrows. Example: participant A\nA->>B: Message".to_string()));
            }
        }

        // Check if classDiagram has class definitions
        if trimmed.starts_with("classDiagram") {
            if !trimmed.contains("class ") && !trimmed.contains("<|--") && !trimmed.contains("..>")
            {
                return (false, Some("Class diagram (classDiagram) must contain class definitions and relationships. Example: class A\nclass B\nA <|-- B".to_string()));
            }
        }

        // Check if stateDiagram has state definitions
        if trimmed.starts_with("stateDiagram") {
            if !trimmed.contains("state ") && !trimmed.contains("[*]") && !trimmed.contains("-->") {
                return (false, Some("State diagram (stateDiagram) must contain state definitions and transitions. Example: state A\n[*] --> A".to_string()));
            }
        }

        // Check for unclosed brackets
        let open_brackets = trimmed.matches('[').count();
        let close_brackets = trimmed.matches(']').count();
        if open_brackets != close_brackets {
            return (false, Some(format!(
                "Unclosed square brackets: found {} '[' but only {} ']'. Please check if node definitions are properly closed.",
                open_brackets, close_brackets
            )));
        }

        let open_parens = trimmed.matches('(').count();
        let close_parens = trimmed.matches(')').count();
        if open_parens != close_parens {
            return (false, Some(format!(
                "Unclosed parentheses: found {} '(' but only {} ')'. Please check if node definitions are properly closed.",
                open_parens, close_parens
            )));
        }

        let open_braces = trimmed.matches('{').count();
        let close_braces = trimmed.matches('}').count();
        if open_braces != close_braces {
            return (false, Some(format!(
                "Unclosed braces: found {} '{{' but only {} '}}'. Please check if node definitions are properly closed.",
                open_braces, close_braces
            )));
        }

        // Check for obvious syntax errors (like isolated arrows)
        let lines_with_arrows: Vec<&str> = lines
            .iter()
            .filter(|line| {
                let trimmed_line = line.trim();
                trimmed_line.contains("-->")
                    || trimmed_line.contains("---")
                    || trimmed_line.contains("==>")
            })
            .copied()
            .collect();

        for line in &lines_with_arrows {
            let trimmed_line = line.trim();
            // Check if there are node identifiers before and after arrows
            if trimmed_line.contains("-->") {
                let parts: Vec<&str> = trimmed_line.split("-->").collect();
                if parts.len() == 2 {
                    let left = parts[0].trim();
                    let right = parts[1].trim();
                    if left.is_empty() || right.is_empty() {
                        return (false, Some(format!(
                            "Arrow '-->' must have node identifiers before and after. Error line: {}",
                            trimmed_line
                        )));
                    }
                }
            }
        }

        (true, None)
    }

    /// Validate node metadata format
    fn validate_node_metadata(&self, metadata: &Value) -> bool {
        if !metadata.is_object() {
            return false;
        }

        // Check metadata for each node
        if let Some(obj) = metadata.as_object() {
            for (node_id, node_data) in obj.iter() {
                if node_id.is_empty() {
                    return false;
                }

                if !node_data.is_object() {
                    return false;
                }

                // Check required field: file_path is required
                let has_file_path = node_data
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);

                if !has_file_path {
                    return false;
                }

                // Get node type (defaults to file)
                let node_type = node_data
                    .get("node_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("file");

                // For file type, line_number is required
                if node_type == "file" {
                    let has_line_number = node_data
                        .get("line_number")
                        .and_then(|v| v.as_u64())
                        .is_some();

                    if !has_line_number {
                        return false;
                    }
                }
                // For directory type, line_number is optional
            }
        }

        true
    }
}

#[async_trait]
impl Tool for MermaidInteractiveTool {
    fn name(&self) -> &str {
        "MermaidInteractive"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Use the MermaidInteractive tool to create interactive diagrams that visualize code execution, architecture, or workflows.

CRITICAL - FILE PATH ACCURACY REQUIREMENTS:
1. NEVER GUESS OR INVENT file paths. Every file_path in node_metadata MUST be a REAL file/directory that EXISTS in the workspace.
2. NEVER GUESS line numbers. Every line_number MUST point to the ACTUAL line where the code is located (e.g., function definition, struct declaration).
3. BEFORE using this tool with node_metadata, you MUST first use Glob/LS to verify files exist, and Read to find exact line numbers.
4. If you cannot confirm a file exists or find the exact line, DO NOT include it in node_metadata. Better to have fewer accurate nodes than many wrong ones.
5. Users click these nodes to navigate - wrong paths destroy trust and usability.

WORKFLOW for creating accurate diagrams:
1. Use Glob/LS to list files in relevant directories
2. Use Read tool with include_line_numbers=true to get line numbers
3. Find the exact line where function/struct/class is defined
4. Only then create the diagram with verified paths and line numbers

HOW TO GET EXACT LINE NUMBERS:
1. Call Read tool with include_line_numbers=true parameter
2. Output format is "LINE_NUMBER|LINE_CONTENT" (e.g., "    42|pub fn main() {")
3. The number before "|" is your line_number value (e.g., 42)
4. For definitions, use the line where "fn name", "struct Name", "class Name", or "impl Name" appears
5. Line numbers start from 1, NOT 0

FILE PATH FORMAT:
- Must be ABSOLUTE path, not relative
- Windows: Use forward slashes preferred (D:/WorkSpace/project/src/main.rs)
- Linux/Mac: Standard path (/home/user/project/src/main.rs)
- Both "D:/path" and "D:\\path" work on Windows, but "/" is recommended
- WRONG: "./src/main.rs" or "src/main.rs" (relative paths)
- RIGHT: "D:/WorkSpace/BitFun/crates/core/src/main.rs" (absolute path)

Example with REAL format (replace paths/lines with your verified values):
{
  "mermaid_code": "graph TD\n    A[main entry] --> B[initialize config]\n    B --> C[start service]",
  "title": "Startup flow",
  "node_metadata": {
    "A": {"node_type": "file", "file_path": "D:/WorkSpace/project/src/main.rs", "line_number": 15, "label": "main"},
    "B": {"node_type": "file", "file_path": "D:/WorkSpace/project/src/config.rs", "line_number": 42, "label": "init_config"},
    "C": {"node_type": "directory", "file_path": "D:/WorkSpace/project/src/server", "label": "server module"}
  }
}

Node Types:
- "file": Opens file and jumps to line_number. Clicking navigates to that exact line. line_number is REQUIRED and must be accurate.
- "directory": Expands folder in file explorer. No line_number needed.

CLICK BEHAVIOR:
- File nodes: Click opens the file in editor and cursor jumps to the specified line_number
- Directory nodes: Click expands/reveals the folder in the file explorer panel
- Nodes without metadata: Not clickable, purely visual

Key Rules:
- Node IDs in mermaid_code must match keys in node_metadata exactly (case-sensitive)
- file_path must be ABSOLUTE path that exists in the workspace
- line_number must be a positive integer (1, 2, 3, ...), pointing to meaningful code location
- For abstract/conceptual nodes (like "Database", "External API"), omit from node_metadata entirely - they will be non-clickable
- Use style statements for colors: style NodeID fill:#color,stroke:#border,color:#text
- Use highlights for execution state: {"executed": ["A"], "current": "B", "failed": ["E"]}

Mermaid Syntax:
- Diagram types: graph, flowchart, sequenceDiagram, classDiagram, stateDiagram, etc.
- Arrows: --> (solid), --- (line), ==> (thick)
- Node shapes: [rect], (round), {diamond}, ((circle))
- Ensure all brackets are properly closed"#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "mermaid_code": {
                    "type": "string",
                    "description": "Mermaid diagram code. Use standard Mermaid syntax. Node IDs should match the keys in node_metadata for interactive features. Add style statements for custom colors."
                },
                "title": {
                    "type": "string",
                    "description": "Title for the diagram panel",
                    "default": "Interactive Mermaid Diagram"
                },
                "node_metadata": {
                    "type": "object",
                    "description": "Metadata for clickable nodes. Keys must match node IDs in mermaid_code. Only include nodes with VERIFIED paths. Workflow: 1) Use Glob/LS to confirm file exists, 2) Use Read with include_line_numbers=true to find exact line, 3) Add node with absolute path and line number. Nodes without metadata are non-clickable.",
                    "additionalProperties": {
                        "type": "object",
                        "properties": {
                            "node_type": {
                                "type": "string",
                                "enum": ["file", "directory"],
                                "description": "Type of node: 'file' (opens file at line_number) or 'directory' (expands in file explorer). Defaults to 'file'."
                            },
                            "file_path": {
                                "type": "string",
                                "description": "ABSOLUTE path that MUST exist. Use forward slashes. Example: 'D:/WorkSpace/project/src/main.rs' or '/home/user/project/src/main.rs'. Verify with Glob/LS first. NEVER use relative paths like './src/main.rs'."
                            },
                            "line_number": {
                                "type": "integer",
                                "description": "Line number (starting from 1) where the code is defined. REQUIRED for 'file' type. Use Read tool with include_line_numbers=true, then extract the number before '|'. Example: if Read shows '  42|pub fn main()', use line_number: 42. NEVER guess."
                            },
                            "label": {
                                "type": "string",
                                "description": "Display label for the node"
                            },
                            "description": {
                                "type": "string",
                                "description": "Detailed description shown in tooltip"
                            },
                            "tooltip": {
                                "type": "string",
                                "description": "Quick tooltip text on hover"
                            },
                            "category": {
                                "type": "string",
                                "enum": ["entry", "process", "decision", "error", "exit"],
                                "description": "Node category for semantic understanding"
                            },
                            "trace_id": {
                                "type": "string",
                                "description": "Trace/log ID for correlation"
                            },
                            "log_data": {
                                "type": "object",
                                "description": "Additional log/trace data as key-value pairs"
                            }
                        },
                        "required": ["file_path"]
                    }
                },
                "highlights": {
                    "type": "object",
                    "description": "Node IDs to highlight with different states",
                    "properties": {
                        "executed": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Nodes that have been executed (green)"
                        },
                        "failed": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Nodes that failed (red)"
                        },
                        "current": {
                            "type": "string",
                            "description": "Current execution node (yellow, animated)"
                        },
                        "warnings": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Nodes with warnings (orange)"
                        }
                    }
                },
                "mode": {
                    "type": "string",
                    "enum": ["interactive", "editor"],
                    "description": "Display mode: 'interactive' for read-only interactive view, 'editor' for editable mode",
                    "default": "interactive"
                },
                "allow_mode_switch": {
                    "type": "boolean",
                    "description": "Whether to allow switching between interactive and editor modes",
                    "default": true
                },
                "enable_navigation": {
                    "type": "boolean",
                    "description": "Enable click-to-navigate functionality",
                    "default": true
                },
                "enable_tooltips": {
                    "type": "boolean",
                    "description": "Enable hover tooltips",
                    "default": true
                }
            },
            "required": ["mermaid_code"]
        })
    }

    fn user_facing_name(&self) -> String {
        "Interactive Mermaid Diagram".to_string()
    }

    async fn is_enabled(&self) -> bool {
        true
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
        // Validate mermaid_code
        let mermaid_code = match input.get("mermaid_code").and_then(|v| v.as_str()) {
            Some(code) if !code.trim().is_empty() => code,
            _ => {
                return ValidationResult {
                    result: false,
                    message: Some("Missing or empty mermaid_code field. Please provide valid Mermaid diagram code.".to_string()),
                    error_code: Some(400),
                    meta: Some(json!({
                        "error_type": "missing_field",
                        "field": "mermaid_code",
                        "suggestion": "Provide Mermaid diagram code starting with a valid diagram type (graph, flowchart, sequenceDiagram, etc.)"
                    })),
                };
            }
        };

        // Validate Mermaid code format (returns detailed error message)
        let (is_valid, error_msg) = self.validate_mermaid_code(mermaid_code);
        if !is_valid {
            let error_message =
                error_msg.unwrap_or_else(|| "Invalid Mermaid diagram syntax".to_string());
            return ValidationResult {
                result: false,
                message: Some(format!(
                    "Mermaid code validation failed: {}\n\nPlease check and fix the following issues:\n1. Ensure code starts with a valid diagram type (graph, flowchart, sequenceDiagram, etc.)\n2. Ensure node definitions and connection syntax are correct\n3. Ensure all parentheses, square brackets, and braces are properly closed\n4. Ensure arrows have node identifiers before and after\n\nPlease regenerate Mermaid code after fixing.",
                    error_message
                )),
                error_code: Some(400),
                meta: Some(json!({
                    "error_type": "syntax_error",
                    "field": "mermaid_code",
                    "error_detail": error_message,
                    "suggestion": "Please fix the syntax errors and regenerate the Mermaid code. Common issues: missing diagram type, unclosed brackets, invalid node definitions, or malformed arrows."
                })),
            };
        }

        // Validate node_metadata (if provided)
        if let Some(node_metadata) = input.get("node_metadata") {
            if !self.validate_node_metadata(node_metadata) {
                return ValidationResult {
                    result: false,
                    message: Some("Invalid node_metadata format. Each node must have file_path (string) and line_number (integer)".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        if let Some(success) = output.get("success").and_then(|v| v.as_bool()) {
            if success {
                let title = output
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Mermaid diagram");

                let node_count = output
                    .get("metadata")
                    .and_then(|m| m.get("node_count"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                let interactive_nodes = output
                    .get("metadata")
                    .and_then(|m| m.get("interactive_nodes"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                if interactive_nodes > 0 {
                    return format!(
                        "Created interactive diagram '{}' with {} nodes ({} clickable). Users can click nodes to navigate to code and see tooltips on hover.",
                        title, node_count, interactive_nodes
                    );
                } else {
                    return format!(
                        "Created diagram '{}' with {} nodes. Add node_metadata to enable interactive features.",
                        title, node_count
                    );
                }
            }
        }

        if let Some(error) = output.get("error").and_then(|v| v.as_str()) {
            return format!("Failed to create Mermaid diagram: {}", error);
        }

        "Mermaid diagram creation result unknown".to_string()
    }

    fn render_tool_use_message(
        &self,
        input: &Value,
        _options: &crate::agentic::tools::framework::ToolRenderOptions,
    ) -> String {
        let title = input
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Interactive Mermaid Diagram");

        let has_metadata = input
            .get("node_metadata")
            .and_then(|v| v.as_object())
            .map(|obj| obj.len())
            .unwrap_or(0)
            > 0;

        if has_metadata {
            format!("Creating interactive diagram: {}", title)
        } else {
            format!("Creating diagram: {}", title)
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let mermaid_code = input
            .get("mermaid_code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing mermaid_code field"))?;

        // Validate Mermaid code
        let (is_valid, error_msg) = self.validate_mermaid_code(mermaid_code);
        if !is_valid {
            let error_message = error_msg.unwrap_or_else(|| "Invalid Mermaid syntax".to_string());
            return Ok(vec![ToolResult::Result {
                data: json!({
                    "success": false,
                    "error": format!(
                        "Mermaid code validation failed, cannot create diagram card. Error: {}\n\nPlease fix Mermaid code syntax errors and regenerate. Common issues:\n1. Diagram type declaration error\n2. Node definition syntax error\n3. Unclosed brackets\n4. Arrow syntax error",
                        error_message
                    ),
                    "error_code": 400,
                    "error_type": "mermaid_validation_failed",
                    "error_detail": error_message,
                    "suggestion": "Please fix the Mermaid syntax errors and regenerate the code. The diagram card will only be created after validation passes."
                }),
                result_for_assistant: Some(format!(
                    "Mermaid code validation failed: {}. Please fix syntax errors and regenerate Mermaid code. Only validated code will display the diagram card.",
                    error_message
                ))
            }]);
        }

        let title = input
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Interactive Mermaid Diagram");

        let mode = input
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("interactive");

        let session_id = context
            .session_id
            .clone()
            .unwrap_or_else(|| format!("mermaid-{}", Utc::now().timestamp_millis()));

        // Build interactive configuration
        let mut interactive_config = json!({
            "enable_navigation": input.get("enable_navigation").and_then(|v| v.as_bool()).unwrap_or(true),
            "enable_tooltips": input.get("enable_tooltips").and_then(|v| v.as_bool()).unwrap_or(true)
        });

        // Add node metadata
        if let Some(node_metadata) = input.get("node_metadata") {
            interactive_config["node_metadata"] = node_metadata.clone();
        }

        // Add highlight states
        if let Some(highlights) = input.get("highlights") {
            interactive_config["highlights"] = highlights.clone();
        }

        // Calculate statistics
        let node_count = mermaid_code
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty()
                    && !trimmed.starts_with("%%")
                    && !trimmed.starts_with("style")
                    && !trimmed.starts_with("classDef")
            })
            .count();

        let interactive_nodes = input
            .get("node_metadata")
            .and_then(|v| v.as_object())
            .map(|obj| obj.len())
            .unwrap_or(0);

        // Build panel data
        let panel_data = json!({
            "mermaid_code": mermaid_code,
            "title": title,
            "session_id": session_id,
            "mode": mode,
            "allow_mode_switch": input.get("allow_mode_switch").and_then(|v| v.as_bool()).unwrap_or(true),
            "interactive_config": interactive_config
        });

        // Send IDE control event to open Mermaid panel
        let event = BackendEvent::Custom {
            event_name: "ide-control-event".to_string(),
            payload: json!({
                "operation": "open_panel",
                "target": {
                    "type": "mermaid-editor",
                    "id": format!("mermaid_{}", session_id),
                    "config": panel_data
                },
                "position": "right",
                "options": {
                    "auto_focus": true,
                    "replace_existing": false,
                    "check_duplicate": true,
                    "expand_panel": true,
                    "mode": "agent"
                },
                "metadata": {
                    "source": "mermaid_interactive_tool",
                    "timestamp": Utc::now().timestamp_millis(),
                    "session_id": session_id.clone()
                }
            }),
        };

        debug!("MermaidInteractive tool creating diagram, mode: {}, title: {}, node_count: {}, interactive_nodes: {}", 
            mode, title, node_count, interactive_nodes);

        let event_system = get_global_event_system();
        event_system.emit(event).await?;

        // Return result
        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "title": title,
                "session_id": session_id,
                "mode": mode,
                "metadata": {
                    "node_count": node_count,
                    "interactive_nodes": interactive_nodes,
                    "has_highlights": input.get("highlights").is_some(),
                    "timestamp": Utc::now().to_rfc3339()
                }
            }),
            result_for_assistant: Some(format!(
                "Interactive Mermaid diagram '{}' created with {} nodes ({} clickable). The diagram is now visible in the right panel. Users can click nodes to navigate to code locations and hover for detailed tooltips.",
                title, node_count, interactive_nodes
            ))
        }])
    }
}
