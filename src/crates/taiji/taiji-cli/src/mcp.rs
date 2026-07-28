//! MCP Server for taiji-quant.
//!
//! Implements a minimal MCP-over-stdio server using JSON-RPC.
//! Exposes tools: taiji_run_backtest, taiji_generate_signal, taiji_analyze, taiji_status.
//!
//! NOTE: This is a simplified implementation. The `rmcp` crate provides a more
//! feature-complete MCP implementation. When the rmcp macro API stabilises,
//! consider migrating to rmcp's `#[tool_router]` / `#[tool_handler]` pattern.

use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info};

use crate::config::{require, GateResult, ResolvedConfig};

// ── JSON-RPC types ──

#[derive(Debug, Deserialize)]
struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Value,
    id: Value,
}

#[derive(Debug, Serialize)]
struct McpResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpErrorObj>,
    id: Value,
}

#[derive(Debug, Serialize)]
struct McpErrorObj {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl McpResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    fn failure(id: Value, code: i64, message: String, data: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(McpErrorObj { code, message, data }),
            id,
        }
    }
}

// ── Tool parameter types ──

#[derive(Deserialize)]
struct BacktestParams {
    config_yaml: String,
    csv_data: String,
}

#[derive(Deserialize)]
struct SignalParams {
    #[allow(dead_code)]
    config_yaml: String,
    #[allow(dead_code)]
    csv_data: String,
}

#[derive(Deserialize)]
struct AnalyzeParams {
    csv_data: String,
}

// ── Tool implementations ──

fn handle_run_backtest(config: &ResolvedConfig, params: Value) -> Result<Value, String> {
    match require("cli.backtest", config) {
        GateResult::UpgradeRequired { message, .. } => return Err(message),
        GateResult::Ok => {}
    }

    let p: BacktestParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid params: {}", e))?;

    let bt_config: taiji_backtest::BacktestConfig = serde_yaml::from_str(&p.config_yaml)
        .map_err(|e| format!("Invalid config YAML: {}", e))?;

    let mut runner = taiji_backtest::BacktestRunner::new(bt_config);
    match runner.run_with_csv(&p.csv_data) {
        Ok(result) => Ok(serde_json::to_value(result).unwrap_or_default()),
        Err(e) => Err(format!("Backtest failed: {}", e)),
    }
}

fn handle_generate_signal(config: &ResolvedConfig, params: Value) -> Result<Value, String> {
    match require("cli.signal", config) {
        GateResult::UpgradeRequired { message, .. } => return Err(message),
        GateResult::Ok => {}
    }

    let _p: SignalParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid params: {}", e))?;

    // TODO(R1.2): full signal generation via pipeline
    let placeholder = serde_json::json!([{
        "message": "Signal generation via MCP — use `taiji signal` CLI for production use",
        "status": "available"
    }]);
    Ok(placeholder)
}

fn handle_analyze(config: &ResolvedConfig, params: Value) -> Result<Value, String> {
    match require("cli.analyze", config) {
        GateResult::UpgradeRequired { message, .. } => return Err(message),
        GateResult::Ok => {}
    }

    let p: AnalyzeParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid params: {}", e))?;

    let lines: Vec<&str> = p.csv_data.lines().collect();
    if lines.len() < 2 {
        return Err("CSV data must have at least a header and one data row".to_string());
    }

    let stats = serde_json::json!({
        "total_rows": lines.len() - 1,
        "note": "Full analysis via `taiji analyze --csv <file>`",
        "preview": lines.iter().take(5).map(|l| l.to_string()).collect::<Vec<_>>(),
    });
    Ok(stats)
}

fn handle_get_status(config: &ResolvedConfig) -> Value {
    serde_json::json!({
        "tier": format!("{:?}", config.tier),
        "features": config.features,
        "data_sources": config.data_sources,
    })
}

// ── Main server loop ──

/// Run the MCP server over stdio.
///
/// Reads JSON-RPC requests from stdin, dispatches to tool handlers,
/// and writes JSON-RPC responses to stdout.
pub(crate) fn run_mcp_server_sync(config: ResolvedConfig) -> Result<(), anyhow::Error> {
    info!("Starting taiji MCP server over stdio (tier={:?})", config.tier);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    // Print server startup message (non-JSON, for human readers)
    eprintln!("[mcp] Server starting (tier={:?})", config.tier);
    eprintln!("[mcp] Listening for JSON-RPC requests on stdin...");

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to read stdin: {}", e);
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: McpRequest = match serde_json::from_str(trimmed) {
            Ok(req) => req,
            Err(e) => {
                let err_resp = McpResponse::failure(
                    Value::Null,
                    -32700,
                    format!("Parse error: {}", e),
                    None,
                );
                let resp_line = serde_json::to_string(&err_resp).unwrap_or_default();
                let _ = writeln!(stdout_lock, "{}", resp_line);
                continue;
            }
        };

        let response = dispatch_tool(&config, &request);

        let resp_line = serde_json::to_string(&response).unwrap_or_default();
        if let Err(e) = writeln!(stdout_lock, "{}", resp_line) {
            error!("Failed to write response: {}", e);
            break;
        }
        let _ = stdout_lock.flush();
    }

    info!("MCP server shutting down (stdin closed).");
    Ok(())
}

fn dispatch_tool(config: &ResolvedConfig, request: &McpRequest) -> McpResponse {
    let id = request.id.clone();

    let result = match request.method.as_str() {
        "taiji_run_backtest" => handle_run_backtest(config, request.params.clone()),
        "taiji_generate_signal" => handle_generate_signal(config, request.params.clone()),
        "taiji_analyze" => handle_analyze(config, request.params.clone()),
        "taiji_status" => Ok(handle_get_status(config)),
        "ping" => Ok(Value::String("pong".to_string())),
        "initialize" => {
            // MCP initialize: return server capabilities
            Ok(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "taiji-quant",
                    "version": "0.1.0"
                }
            }))
        }
        _ => Err(format!("Method not found: {}", request.method)),
    };

    match result {
        Ok(value) => McpResponse::success(id, value),
        Err(msg) => McpResponse::failure(id, -32000, msg, None),
    }
}
