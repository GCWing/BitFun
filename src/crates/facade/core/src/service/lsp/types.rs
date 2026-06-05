//! LSP type definitions
//!
//! Data structures related to the LSP protocol.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// LSP plugin manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspPlugin {
    /// Plugin unique identifier.
    pub id: String,
    /// Plugin display name.
    pub name: String,
    /// Version.
    pub version: String,
    /// Author.
    pub author: String,
    /// Description.
    pub description: String,
    /// Server configuration.
    pub server: ServerConfig,
    /// Supported languages.
    pub languages: Vec<String>,
    /// File extensions.
    pub file_extensions: Vec<String>,
    /// Capability configuration.
    pub capabilities: CapabilitiesConfig,
    /// Default settings.
    #[serde(default)]
    pub settings: HashMap<String, serde_json::Value>,
    /// Checksum.
    #[serde(default)]
    pub checksum: String,
    /// Minimum BitFun version.
    #[serde(default)]
    pub min_bitfun_version: String,
}

/// Server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Command path (relative to the plugin directory).
    pub command: String,
    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Runtime type (optional: "exe", "bash", "node").
    /// Defaults to "exe"; Windows is handled automatically.
    #[serde(default)]
    pub runtime: Option<String>,
}

/// Runtime type enum.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeType {
    /// Native executable (e.g. `.exe`).
    Executable,
    /// Bash script.
    Bash,
    /// Node.js program.
    Node,
}

/// Capability configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesConfig {
    #[serde(default)]
    pub completion: bool,
    #[serde(default)]
    pub hover: bool,
    #[serde(default)]
    pub definition: bool,
    #[serde(default)]
    pub references: bool,
    #[serde(default)]
    pub rename: bool,
    #[serde(default)]
    pub formatting: bool,
    #[serde(default)]
    pub diagnostics: bool,
    #[serde(default)]
    pub inlay_hints: bool,
}

/// JSON-RPC message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

/// JSON-RPC request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// Initialize parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    /// Process ID.
    pub process_id: Option<u32>,
    /// Workspace root path (deprecated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
    /// Workspace URI.
    pub root_uri: Option<String>,
    /// Client capabilities.
    pub capabilities: ClientCapabilities,
    /// Initialization options.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initialization_options: Option<serde_json::Value>,
    /// Workspace folders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_folders: Option<Vec<WorkspaceFolder>>,
}

/// Client capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_document: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
}

/// Workspace folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFolder {
    pub uri: String,
    pub name: String,
}

/// Initialize result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_info: Option<ServerInfo>,
}

/// Server capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_provider: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rename_provider: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_formatting_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_document_sync: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inlay_hint_provider: Option<serde_json::Value>,
}

/// Server information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Completion item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    /// Label (display text).
    pub label: String,
    /// Kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<u32>,
    /// Detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Documentation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<serde_json::Value>,
    /// Sort text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_text: Option<String>,
    /// Filter text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_text: Option<String>,
    /// Insert text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
    /// Insert text format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text_format: Option<u32>,
}

/// Completion list (LSP response format).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionList {
    /// Whether the list is incomplete (more completion items exist).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_incomplete: Option<bool>,
    /// Completion items.
    pub items: Vec<CompletionItem>,
}

/// Position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// Range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// Text document identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentIdentifier {
    pub uri: String,
}

/// Text document position parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentPositionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// Diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Range,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub message: String,
}

/// Plugin installation source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginSource {
    Marketplace,
    File(PathBuf),
    Url(String),
}

/// Inlay hint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlayHint {
    /// Position.
    pub position: Position,
    /// Label (display text).
    pub label: InlayHintLabel,
    /// Kind (1 = Type, 2 = Parameter).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<u32>,
    /// Tooltip.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<serde_json::Value>,
    /// Whether to render padding before the position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_left: Option<bool>,
    /// Whether to render padding after the position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_right: Option<bool>,
    /// Text edits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_edits: Option<Vec<TextEdit>>,
}

/// Inlay hint label.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InlayHintLabel {
    String(String),
    Parts(Vec<InlayHintLabelPart>),
}

/// Inlay hint label part.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlayHintLabelPart {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
}

/// Location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

/// Text edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: Range,
    #[serde(rename = "newText")]
    pub new_text: String,
}
