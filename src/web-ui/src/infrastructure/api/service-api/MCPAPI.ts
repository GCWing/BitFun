 

import { api } from './ApiClient';

/** MCP Apps protocol version (aligned with VSCode modelContextProtocolApps.ts). */
export const MCP_APPS_PROTOCOL_VERSION = '2026-01-26';

export type MCPServerStatus = 
  | 'Uninitialized'
  | 'Starting'
  | 'Connected'
  | 'Healthy'
  | 'Reconnecting'
  | 'Failed'
  | 'Stopping'
  | 'Stopped';

 
export interface MCPServerInfo {
  id: string;
  name: string;
  status: string;
  serverType: string;
  enabled: boolean;
  autoStart: boolean;
  command?: string;
  commandAvailable?: boolean;
  commandSource?: 'system' | 'managed';
  commandResolvedPath?: string;
}

export interface RuntimeCommandCapability {
  command: string;
  available: boolean;
  source?: 'system' | 'managed';
  resolvedPath?: string;
}

 
export interface MCPResource {
  uri: string;
  name: string;
  description?: string;
  mimeType?: string;
  metadata?: Record<string, any>;
}

 
export interface MCPPrompt {
  name: string;
  description?: string;
  arguments?: Array<{
    name: string;
    description?: string;
    required: boolean;
  }>;
}

 
export interface MCPTool {
  name: string;
  description?: string;
  inputSchema: any;
}

 
/** Content Security Policy configuration for MCP App UI (aligned with VSCode). */
export interface McpUiResourceCsp {
  /** Origins for network requests (fetch/XHR/WebSocket). */
  connectDomains?: string[];
  /** Origins for static resources (scripts, images, styles, fonts). */
  resourceDomains?: string[];
  /** Origins for nested iframes (frame-src directive). */
  frameDomains?: string[];
  /** Allowed base URIs for the document (base-uri directive). */
  baseUriDomains?: string[];
}

/** Sandbox permissions requested by the UI resource (aligned with VSCode). */
export interface McpUiResourcePermissions {
  /** Request camera access. */
  camera?: {};
  /** Request microphone access. */
  microphone?: {};
  /** Request geolocation access. */
  geolocation?: {};
  /** Request clipboard write access. */
  clipboardWrite?: {};
}

// ==================== MCP App ui/message types ====================

/** Content block types for ui/message (aligned with MCP Apps spec). */
export type McpUiMessageContentBlock =
  | { type: 'text'; text: string }
  | { type: 'image'; data: string; mimeType?: string }
  | { type: 'resource_link'; uri: string; name?: string; mimeType?: string }
  | { type: 'resource'; resource: { uri: string; mimeType?: string; text?: string; blob?: string } };

/** ui/message request params from MCP App. */
export interface McpUiMessageParams {
  /** Message role, currently only "user" is supported. */
  role: 'user';
  /** Message content blocks (text, image, etc.). */
  content: McpUiMessageContentBlock[];
}

/** ui/message result returned to MCP App. */
export interface McpUiMessageResult {
  /** True if the host rejected or failed to deliver the message. */
  isError?: boolean;
  [key: string]: unknown;
}

/** ui/update-model-context request params from MCP App. */
export interface McpUiUpdateModelContextParams {
  /** Context content blocks (text, image, etc.). */
  content?: McpUiMessageContentBlock[];
  /** Structured content for machine-readable context data. */
  structuredContent?: Record<string, unknown>;
}

// ==================== MCP App message event types ====================

/** Event payload for mcp-app:message event with requestId for response. */
export interface McpAppMessageEvent {
  /** Unique request ID for correlating response. */
  requestId: string;
  /** Message params from MCP App. */
  params: McpUiMessageParams;
}

/** Event payload for mcp-app:message-response event. */
export interface McpAppMessageResponseEvent {
  /** Request ID to correlate with original request. */
  requestId: string;
  /** Result of handling the message. */
  result: McpUiMessageResult;
}

/** MCP App resource content (ui:// scheme and others like videos://). */
export interface MCPAppResourceContent {
  uri: string;
  /** Text content (for HTML, etc.). Omitted when resource has blob only. */
  content?: string;
  /** Base64-encoded binary content (MCP spec). Used for video, images, etc. */
  blob?: string;
  mimeType?: string;
  /** Content Security Policy configuration. */
  csp?: McpUiResourceCsp;
  /** Sandbox permissions requested by the UI. */
  permissions?: McpUiResourcePermissions;
}

/** Fetch MCP App UI resource for rendering in sandboxed iframe. */
export interface FetchMCPAppResourceRequest {
  serverId: string;
  resourceUri: string;
}

export interface FetchMCPAppResourceResponse {
  contents: MCPAppResourceContent[];
}

export class MCPAPI {

  static async initializeServers(): Promise<void> {
    return api.invoke('initialize_mcp_servers');
  }

   
  static async initializeServersNonDestructive(): Promise<void> {
    return api.invoke('initialize_mcp_servers_non_destructive');
  }

   
  static async getServers(): Promise<MCPServerInfo[]> {
    return api.invoke('get_mcp_servers');
  }

   
  static async getRuntimeCapabilities(): Promise<RuntimeCommandCapability[]> {
    return api.invoke('get_runtime_capabilities');
  }

   
  static async startServer(serverId: string): Promise<void> {
    return api.invoke('start_mcp_server', { serverId });
  }

   
  static async stopServer(serverId: string): Promise<void> {
    return api.invoke('stop_mcp_server', { serverId });
  }

   
  static async restartServer(serverId: string): Promise<void> {
    return api.invoke('restart_mcp_server', { serverId });
  }

   
  static async getServerStatus(serverId: string): Promise<string> {
    return api.invoke('get_mcp_server_status', { serverId });
  }

   
  static async loadMCPJsonConfig(): Promise<string> {
    return api.invoke('load_mcp_json_config');
  }

   
  static async saveMCPJsonConfig(jsonConfig: string): Promise<void> {
    return api.invoke('save_mcp_json_config', { jsonConfig });
  }

  /**
   * Get MCP App UI resource URI from tool metadata (_meta.ui.resourceUri).
   * Returns null if tool has no UI or is not an MCP tool.
   */
  static async getMCPToolUiUri(toolName: string): Promise<string | null> {
    const result = await api.invoke<string | null>('get_mcp_tool_ui_uri', { toolName });
    return result ?? null;
  }

  /**
   * Fetch MCP App UI resource (ui:// scheme) for rendering in sandboxed iframe.
   * Used by MCP Apps extension.
   */
  static async fetchMCPAppResource(
    request: FetchMCPAppResourceRequest
  ): Promise<FetchMCPAppResourceResponse> {
    return api.invoke('fetch_mcp_app_resource', { request });
  }

  /**
   * Forward JSON-RPC message from MCP App iframe to the MCP server (tools/call, resources/read, ping).
   * Request must include serverId plus JSON-RPC fields (method, params, id) - backend expects flattened shape.
   * Returns the JSON-RPC response to postMessage back to the iframe.
   * Note: Backend uses #[serde(flatten)], so response fields are at top level.
   */
  static async sendMCPAppMessage(request: {
    serverId: string;
    [key: string]: unknown;
  }): Promise<Record<string, unknown>> {
    return api.invoke('send_mcp_app_message', { request });
  }
}

export default MCPAPI;
