//! ACP Server for taiji-quant.
//!
//! Implements the Agent Communication Protocol (JSON-RPC over stdio).
//! Supports `run_backtest`, `generate_signal`, `analyze`, `get_status` methods.

// 优先尝试使用 agent-client-protocol crate 的类型定义
// 若 crate 不可用，回退到自建 JSON-RPC 格式
use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::config::{require, GateResult, ResolvedConfig};

// ── JSON-RPC types ──

#[derive(Debug, Deserialize)]
struct AcpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Value,
    id: Value, // string or number
}

#[derive(Debug, Serialize)]
struct AcpResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<AcpError>,
    id: Value,
}

#[derive(Debug, Serialize)]
struct AcpError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl AcpResponse {
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
            error: Some(AcpError { code, message, data }),
            id,
        }
    }
}

// ── Request handlers ──

fn handle_run_backtest(config: &ResolvedConfig, params: Value) -> Result<Value, String> {
    match require("cli.backtest", config) {
        GateResult::UpgradeRequired { message, .. } => return Err(message),
        GateResult::Ok => {}
    }

    let config_yaml = params.get("config")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'config' parameter (YAML string)".to_string())?;
    let csv_data = params.get("csv_data")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'csv_data' parameter".to_string())?;

    let bt_config: taiji_backtest::BacktestConfig =
        serde_yaml::from_str(config_yaml)
            .map_err(|e| format!("Invalid config YAML: {}", e))?;

    let mut runner = taiji_backtest::BacktestRunner::new(bt_config);
    match runner.run_with_csv(csv_data) {
        Ok(result) => {
            serde_json::to_value(result)
                .map_err(|e| format!("Serialization error: {}", e))
        }
        Err(e) => Err(format!("Backtest failed: {}", e)),
    }
}

fn handle_generate_signal(config: &ResolvedConfig, _params: Value) -> Result<Value, String> {
    match require("cli.signal", config) {
        GateResult::UpgradeRequired { message, .. } => return Err(message),
        GateResult::Ok => {}
    }

    // Placeholder — full signal generation would parse pipeline YAML + CSV
    Ok(serde_json::json!({
        "status": "available",
        "message": "Signal generation via ACP — use the full CLI for production",
        "tools": ["taiji signal --config <pipeline.yaml> --csv <data.csv>"]
    }))
}

fn handle_analyze(config: &ResolvedConfig, params: Value) -> Result<Value, String> {
    match require("cli.analyze", config) {
        GateResult::UpgradeRequired { message, .. } => return Err(message),
        GateResult::Ok => {}
    }

    let csv_data = params.get("csv_data")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'csv_data' parameter".to_string())?;

    let lines: Vec<&str> = csv_data.lines().collect();
    let row_count = lines.len().saturating_sub(1);

    Ok(serde_json::json!({
        "total_rows": row_count,
        "note": "Full analysis available via `taiji analyze --csv <file>`",
    }))
}

fn handle_get_status(config: &ResolvedConfig) -> Result<Value, String> {
    Ok(serde_json::json!({
        "tier": format!("{:?}", config.tier),
        "features": config.features,
        "data_sources": config.data_sources,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ── Main server loop ──

/// Run the ACP JSON-RPC server over stdio.
///
/// Reads JSON-RPC requests line by line from stdin,
/// dispatches to the appropriate handler, and writes responses to stdout.
pub(crate) fn run_acp_server(config: ResolvedConfig) -> Result<(), anyhow::Error> {
    info!("Starting taiji ACP server over stdio (tier={:?})", config.tier);

    let stdin = io::stdin();
    let reader = stdin.lock();
    let mut line_buf = String::new();

    // Signal readiness
    let ready = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "server/ready",
        "params": {
            "tier": format!("{:?}", config.tier),
            "features": config.features.keys().cloned().collect::<Vec<_>>(),
        }
    });
    println!("{}", serde_json::to_string(&ready)?);
    io::stdout().flush()?;

    for line_result in reader.lines() {
        line_buf.clear();
        match line_result {
            Ok(line) => {
                line_buf = line;
            }
            Err(e) => {
                warn!("ACP stdin read error: {}", e);
                break;
            }
        }

        let trimmed = line_buf.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: AcpRequest = match serde_json::from_str(trimmed) {
            Ok(req) => req,
            Err(e) => {
                let resp = AcpResponse::failure(
                    Value::Null,
                    -32700,
                    format!("Parse error: {}", e),
                    None,
                );
                println!("{}", serde_json::to_string(&resp)?);
                io::stdout().flush()?;
                continue;
            }
        };

        debug!("ACP request: method={}, id={:?}", request.method, request.id);

        let response = match request.method.as_str() {
            "run_backtest" => {
                match handle_run_backtest(&config, request.params) {
                    Ok(result) => AcpResponse::success(request.id.clone(), result),
                    Err(msg) => AcpResponse::failure(request.id.clone(), -32000, msg, None),
                }
            }
            "generate_signal" => {
                match handle_generate_signal(&config, request.params) {
                    Ok(result) => AcpResponse::success(request.id.clone(), result),
                    Err(msg) => AcpResponse::failure(request.id.clone(), -32001, msg, None),
                }
            }
            "analyze" => {
                match handle_analyze(&config, request.params) {
                    Ok(result) => AcpResponse::success(request.id.clone(), result),
                    Err(msg) => AcpResponse::failure(request.id.clone(), -32002, msg, None),
                }
            }
            "get_status" => {
                match handle_get_status(&config) {
                    Ok(result) => AcpResponse::success(request.id.clone(), result),
                    Err(msg) => AcpResponse::failure(request.id.clone(), -32003, msg, None),
                }
            }
            _ => AcpResponse::failure(
                request.id.clone(),
                -32601,
                format!("Method '{}' not found", request.method),
                None,
            ),
        };

        let resp_json = serde_json::to_string(&response)?;
        println!("{}", resp_json);
        io::stdout().flush()?;

        if response.error.is_some() {
            warn!("ACP error: method={}, id={:?}", request.method, request.id);
        }
    }

    info!("ACP server shutting down.");
    Ok(())
}

// ── stdio EOF 处理策略 ──
//
// ACP server 使用 std::io::BufRead 逐行读取 stdin。
// EOF 处理模式：
//   1. stdin.lock().lines() 在 EOF 时返回 Ok(0) / None，
//      for 循环自然退出，server 正常关闭
//   2. 不会产生 panic — reader.lines() 的 Err 分支只记录 warn
//     并 break 出循环
//   3. 客户端断开连接后，不应再次写入 stdout（println! 静默失败）
//
// 生产环境建议启用 keepalive/ping:
//   - ACP 客户端定期发送空行或 ping 请求
//   - server 超时 N 秒无读取后主动 shutdown
//   当前实现使用最简单的"EOF 即退出"模式：
//
//   for line_result in reader.lines() {
//       match line_result {
//           Ok(line) => { /* process */ }
//           Err(e) => {
//               warn!("ACP stdin read error: {}, exiting", e);
//               break;  // ← 关键：Err 时退出循环
//           }
//       }
//   }
//   info!("ACP server shutting down.");  // ← 正常到达这里
//
// ## 重要注意事项
// - EOF 后不要再 println! 到 stdout — JSON-RPC 响应无法送达
// - 测试 MCP/ACP 时使用 `echo '{}' | taiji mcp` 模拟 EOF
// - 使用 `timeout` 命令防止 server hang:
//     timeout 10 taiji acp < /dev/null || true
