//! LogTool - log tool implementation
//!
//! Provides log viewing, filtering, and analysis functionality

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::agentic::tools::framework::{Tool, ToolExposure, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};

/// LogTool - log viewing and analysis tool
pub struct LogTool;

/// LogTool input parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogToolInput {
    pub action: String, // Operation type: "read", "tail", "search", "analyze"
    pub log_path: Option<String>, // Log file path
    pub lines: Option<usize>, // Number of lines to read (for tail operation)
    pub pattern: Option<String>, // Search pattern (for search operation)
    pub level: Option<String>, // Log level filter: "error", "warn", "info", "debug"
}

impl Default for LogTool {
    fn default() -> Self {
        Self::new()
    }
}

impl LogTool {
    pub fn new() -> Self {
        Self
    }

    /// Read log file
    async fn read_log(&self, log_path: &str, lines: Option<usize>) -> BitFunResult<String> {
        let path = PathBuf::from(log_path);

        if !path.exists() {
            return Err(BitFunError::validation(format!(
                "Log file does not exist: {}",
                log_path
            )));
        }

        let mut file = fs::File::open(&path)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to open log file: {}", e)))?;

        let mut content = String::new();
        file.read_to_string(&mut content)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read log file: {}", e)))?;

        // If number of lines is specified, only return last N lines
        if let Some(n) = lines {
            let lines_vec: Vec<&str> = content.lines().collect();
            let start = if lines_vec.len() > n {
                lines_vec.len() - n
            } else {
                0
            };
            Ok(lines_vec[start..].join("\n"))
        } else {
            Ok(content)
        }
    }

    /// Search log content
    async fn search_log(
        &self,
        log_path: &str,
        pattern: &str,
        level: Option<String>,
    ) -> BitFunResult<String> {
        let content = self.read_log(log_path, None).await?;

        let mut results = Vec::new();
        for (idx, line) in content.lines().enumerate() {
            // Level filter
            if let Some(ref level_filter) = level {
                let level_upper = level_filter.to_uppercase();
                if !line.contains(&level_upper) {
                    continue;
                }
            }

            // Pattern matching
            if line.contains(pattern) {
                results.push(format!("[{}] {}", idx + 1, line));
            }
        }

        if results.is_empty() {
            Ok("No matching log records found".to_string())
        } else {
            Ok(format!(
                "Found {} matching records:\n{}",
                results.len(),
                results.join("\n")
            ))
        }
    }

    /// Analyze log statistics
    async fn analyze_log(&self, log_path: &str) -> BitFunResult<Value> {
        let content = self.read_log(log_path, None).await?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let mut error_count = 0;
        let mut warn_count = 0;
        let mut info_count = 0;
        let mut debug_count = 0;

        for line in &lines {
            let line_upper = line.to_uppercase();
            if line_upper.contains("ERROR") {
                error_count += 1;
            } else if line_upper.contains("WARN") {
                warn_count += 1;
            } else if line_upper.contains("INFO") {
                info_count += 1;
            } else if line_upper.contains("DEBUG") {
                debug_count += 1;
            }
        }

        Ok(json!({
            "total_lines": total_lines,
            "error_count": error_count,
            "warn_count": warn_count,
            "info_count": info_count,
            "debug_count": debug_count,
            "file_path": log_path,
        }))
    }
}

#[async_trait]
impl Tool for LogTool {
    fn name(&self) -> &str {
        "Log"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Read and analyze log files to debug issues, monitor application behavior, and understand system events.

Available actions:
- read: Read the entire log file content
- tail: Read the last N lines from the log file (specify 'lines' parameter)
- search: Search for a specific pattern in the log file (specify 'pattern' parameter)
- analyze: Get statistical analysis of the log file (error count, warning count, etc.)

You can filter logs by level using the 'level' parameter: error, warn, info, debug.

When to use this tool:
- When you need to check application logs to understand errors or warnings
- When debugging issues and need to see recent log entries
- When searching for specific error messages or patterns in logs
- When analyzing log patterns to understand system behavior
- When the user asks to check logs, view errors, or investigate issues

When NOT to use this tool:
- When you need to read regular source code files (use FileRead instead)
- When you need to search for code patterns (use Grep instead)
- When the file is not a log file

Usage examples:
1. Read last 100 lines of a log file:
   Log(action="tail", log_path="/var/log/app.log", lines=100)

2. Search for error messages:
   Log(action="search", log_path="/var/log/app.log", pattern="Exception", level="error")

3. Analyze log statistics:
   Log(action="analyze", log_path="/var/log/app.log")

4. Read entire log file:
   Log(action="read", log_path="/var/log/app.log")

The tool will return the log content or analysis results that you can use to diagnose issues."#.to_string())
    }

    fn short_description(&self) -> String {
        "Read and analyze log files for debugging and monitoring.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Collapsed
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Action to perform: 'read' (read entire log), 'tail' (read last N lines), 'search' (search for pattern), 'analyze' (get statistics)",
                    "enum": ["read", "tail", "search", "analyze"]
                },
                "log_path": {
                    "type": "string",
                    "description": "Path to the log file"
                },
                "lines": {
                    "type": "integer",
                    "description": "Number of lines to read from the end (for tail action)"
                },
                "pattern": {
                    "type": "string",
                    "description": "Search pattern (for search action)"
                },
                "level": {
                    "type": "string",
                    "description": "Filter by log level: 'error', 'warn', 'info', 'debug'",
                    "enum": ["error", "warn", "info", "debug"]
                }
            },
            "required": ["action", "log_path"]
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // Validate input
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::validation("Missing required field: action"))?;

        let valid_actions = ["read", "tail", "search", "analyze"];
        if !valid_actions.contains(&action) {
            return Err(BitFunError::validation(format!(
                "Invalid action '{}'. Must be one of: {}",
                action,
                valid_actions.join(", ")
            )));
        }

        // Validate tail operation requires lines parameter
        if action == "tail" && input.get("lines").is_none() {
            return Err(BitFunError::validation(
                "tail action requires 'lines' parameter",
            ));
        }

        // Validate search operation requires pattern parameter
        if action == "search" && input.get("pattern").is_none() {
            return Err(BitFunError::validation(
                "search action requires 'pattern' parameter",
            ));
        }

        // Parse input
        let log_input: LogToolInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::validation(format!("Invalid input: {}", e)))?;

        let log_path = log_input
            .log_path
            .as_ref()
            .ok_or_else(|| BitFunError::validation("log_path is required"))?;

        let result = match log_input.action.as_str() {
            "read" => {
                let content = self.read_log(log_path, None).await?;
                json!({
                    "action": "read",
                    "file_path": log_path,
                    "content": content,
                    "lines": content.lines().count()
                })
            }
            "tail" => {
                let lines = log_input.lines.unwrap_or(100);
                let content = self.read_log(log_path, Some(lines)).await?;
                json!({
                    "action": "tail",
                    "file_path": log_path,
                    "content": content,
                    "lines": content.lines().count(),
                    "requested_lines": lines
                })
            }
            "search" => {
                let pattern = log_input.pattern.as_ref().ok_or_else(|| {
                    BitFunError::validation("pattern is required for search action")
                })?;
                let level_filter = log_input.level.clone();
                let results = self
                    .search_log(log_path, pattern, level_filter.clone())
                    .await?;
                json!({
                    "action": "search",
                    "file_path": log_path,
                    "pattern": pattern,
                    "level": level_filter,
                    "results": results
                })
            }
            "analyze" => {
                let stats = self.analyze_log(log_path).await?;
                stats
            }
            _ => {
                return Err(BitFunError::validation(format!(
                    "Unknown action: {}",
                    log_input.action
                )));
            }
        };

        let result_for_assistant =
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
        Ok(vec![ToolResult::Result {
            data: result,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}
