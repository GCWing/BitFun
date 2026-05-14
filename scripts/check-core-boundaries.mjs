#!/usr/bin/env node

import { readdirSync, readFileSync, statSync } from 'fs';
import { join, relative } from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

const noCoreDependencyCrates = [
  'core-types',
  'events',
  'ai-adapters',
  'agent-stream',
  'runtime-ports',
  'services-core',
  'services-integrations',
  'agent-tools',
  'tool-packs',
  'product-domains',
  'terminal',
  'tool-runtime',
  'transport',
  'api-layer',
  'webdriver',
];

const lightweightBoundaryRules = [
  {
    crateName: 'core-types',
    reason: 'core-types must stay low-level DTO-only',
    forbiddenDeps: [
      'bitfun-core',
      'bitfun-events',
      'bitfun-ai-adapters',
      'bitfun-agent-stream',
      'bitfun-runtime-ports',
      'bitfun-services-core',
      'bitfun-services-integrations',
      'bitfun-agent-tools',
      'bitfun-tool-packs',
      'bitfun-product-domains',
      'bitfun-transport',
      'terminal-core',
      'tool-runtime',
      'tauri',
      'reqwest',
      'git2',
      'rmcp',
      'image',
      'tokio-tungstenite',
      'bitfun-cli',
      'ratatui',
      'crossterm',
      'arboard',
      'syntect-tui',
    ],
  },
  {
    crateName: 'runtime-ports',
    reason: 'runtime-ports must stay DTO/trait-only',
    forbiddenDeps: [
      'bitfun-core',
      'bitfun-agent-stream',
      'bitfun-services-core',
      'bitfun-services-integrations',
      'bitfun-agent-tools',
      'bitfun-tool-packs',
      'bitfun-product-domains',
      'bitfun-transport',
      'terminal-core',
      'tool-runtime',
      'tauri',
      'reqwest',
      'git2',
      'rmcp',
      'image',
      'tokio-tungstenite',
      'bitfun-cli',
      'ratatui',
      'crossterm',
      'arboard',
      'syntect-tui',
    ],
  },
  {
    crateName: 'agent-tools',
    reason: 'agent-tools must not depend on concrete service or product runtime implementations',
    forbiddenDeps: [
      'bitfun-core',
      'bitfun-ai-adapters',
      'bitfun-services-core',
      'bitfun-services-integrations',
      'bitfun-tool-packs',
      'bitfun-product-domains',
      'bitfun-transport',
      'terminal-core',
      'tool-runtime',
      'tauri',
      'reqwest',
      'git2',
      'rmcp',
      'tokio-tungstenite',
      'bitfun-cli',
      'ratatui',
      'crossterm',
      'arboard',
      'syntect-tui',
    ],
  },
];

const dependencyProfileRules = [
  {
    crateName: 'core-types',
    profileName: 'default DTO profile',
    reason: 'core-types default profile must stay DTO-only',
    forbiddenNonOptionalDeps: [
      'bitfun-core',
      'bitfun-events',
      'bitfun-ai-adapters',
      'bitfun-agent-stream',
      'bitfun-runtime-ports',
      'bitfun-services-core',
      'bitfun-services-integrations',
      'bitfun-agent-tools',
      'bitfun-tool-packs',
      'bitfun-product-domains',
      'bitfun-transport',
      'terminal-core',
      'tool-runtime',
      'tauri',
      'reqwest',
      'git2',
      'rmcp',
      'image',
      'tokio-tungstenite',
      'bitfun-cli',
      'ratatui',
      'crossterm',
      'arboard',
      'syntect-tui',
    ],
  },
  {
    crateName: 'runtime-ports',
    profileName: 'default ports profile',
    reason: 'runtime-ports default profile must stay trait/DTO-only',
    forbiddenNonOptionalDeps: [
      'bitfun-core',
      'bitfun-ai-adapters',
      'bitfun-agent-stream',
      'bitfun-services-core',
      'bitfun-services-integrations',
      'bitfun-agent-tools',
      'bitfun-tool-packs',
      'bitfun-product-domains',
      'bitfun-transport',
      'terminal-core',
      'tool-runtime',
      'tauri',
      'reqwest',
      'git2',
      'rmcp',
      'image',
      'tokio-tungstenite',
      'bitfun-cli',
      'ratatui',
      'crossterm',
      'arboard',
      'syntect-tui',
    ],
  },
  {
    crateName: 'agent-tools',
    profileName: 'tool contract-only profile',
    reason: 'agent-tools must stay a lightweight tool contract crate',
    forbiddenNonOptionalDeps: [
      'bitfun-ai-adapters',
      'reqwest',
      'git2',
      'rmcp',
      'image',
      'tokio-tungstenite',
      'tauri',
      'bitfun-cli',
      'ratatui',
      'crossterm',
      'arboard',
      'syntect-tui',
    ],
  },
  {
    crateName: 'product-domains',
    profileName: 'default product domain profile',
    reason: 'product-domains default profile must not compile runtime/platform helpers',
    forbiddenNonOptionalDeps: [
      'dirs',
      'reqwest',
      'git2',
      'rmcp',
      'image',
      'tokio-tungstenite',
      'tauri',
      'bitfun-cli',
      'ratatui',
      'crossterm',
      'arboard',
      'syntect-tui',
    ],
  },
  {
    crateName: 'services-integrations',
    profileName: 'default integrations profile',
    reason: 'services-integrations default profile must not compile feature-gated integrations',
    forbiddenNonOptionalDeps: [
      'aes-gcm',
      'anyhow',
      'async-trait',
      'base64',
      'bitfun-runtime-ports',
      'bitfun-services-core',
      'chrono',
      'futures',
      'git2',
      'notify',
      'rand',
      'reqwest',
      'rmcp',
      'sha2',
      'sse-stream',
      'thiserror',
      'tokio-util',
      'tokio-tungstenite',
      'bitfun-relay-server',
    ],
  },
];

const facadeOnlyFiles = [
  {
    path: 'src/crates/core/src/service/git/git_service.rs',
    importPrefix: 'bitfun_services_integrations::git',
    reason: 'core git service facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/git/git_types.rs',
    importPrefix: 'bitfun_services_integrations::git',
    reason: 'core git types facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/git/git_utils.rs',
    importPrefix: 'bitfun_services_integrations::git',
    reason: 'core git utils facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/git/graph.rs',
    importPrefix: 'bitfun_services_integrations::git',
    reason: 'core git graph facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/remote_ssh/types.rs',
    importPrefix: 'bitfun_services_integrations::remote_ssh',
    reason: 'core remote SSH types facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/mcp/tool_info.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP tool info facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/mcp/tool_name.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP tool name facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/mcp/protocol/types.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP protocol types facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/mcp/protocol/transport.rs',
    importPrefix: 'bitfun_services_integrations::mcp::protocol',
    reason: 'core MCP stdio transport facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/mcp/protocol/transport_remote.rs',
    importPrefix: 'bitfun_services_integrations::mcp::protocol',
    reason: 'core MCP remote transport facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/mcp/server/connection.rs',
    importPrefix: 'bitfun_services_integrations::mcp::server',
    reason: 'core MCP connection facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/mcp/config/location.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP config location facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/mcp/adapter/resource.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP resource adapter facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/mcp/adapter/prompt.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP prompt adapter facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/announcement/types.rs',
    importPrefix: 'bitfun_services_integrations::announcement',
    reason: 'core announcement types facade must only re-export the integrations owner crate',
  },
];

const forbiddenContentRules = [
  {
    path: 'src/crates/core/src/agentic/tools/framework.rs',
    patterns: [
      {
        regex: /\bpub struct DynamicMcpToolInfo\b/,
        message: 'core tool framework must not redefine DynamicMcpToolInfo; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct DynamicToolInfo\b/,
        message: 'core tool framework must not redefine DynamicToolInfo; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ToolRenderOptions\b/,
        message: 'core tool framework must not redefine ToolRenderOptions; use bitfun-agent-tools',
      },
      {
        regex: /\bpub enum ToolPathBackend\b/,
        message: 'core tool framework must not redefine ToolPathBackend; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ToolPathResolution\b/,
        message: 'core tool framework must not redefine ToolPathResolution; use bitfun-agent-tools',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/restrictions.rs',
    patterns: [
      {
        regex: /\bpub enum ToolPathOperation\b/,
        message: 'core tool restrictions must not redefine ToolPathOperation; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ToolPathPolicy\b/,
        message: 'core tool restrictions must not redefine ToolPathPolicy; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ToolRuntimeRestrictions\b/,
        message:
          'core tool restrictions must not redefine ToolRuntimeRestrictions; use bitfun-agent-tools',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/registry.rs',
    patterns: [
      {
        regex: /\bstruct DynamicToolMetadata\b/,
        message:
          'core tool registry must not own dynamic tool metadata storage; use bitfun-agent-tools ToolRegistry',
      },
      {
        regex: /\btools\s*:\s*IndexMap\b/,
        message:
          'core tool registry must not own the generic tool map; use bitfun-agent-tools ToolRegistry',
      },
      {
        regex: /\bdynamic_tools\s*:\s*IndexMap\b/,
        message:
          'core tool registry must not own the dynamic tool map; use bitfun-agent-tools ToolRegistry',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/server/process.rs',
    patterns: [
      {
        regex: /\bpub enum MCPServerType\b/,
        message: 'core MCP server process runtime must not redefine MCPServerType; use the integrations contract',
      },
      {
        regex: /\bpub enum MCPServerStatus\b/,
        message: 'core MCP server process runtime must not redefine MCPServerStatus; use the integrations contract',
      },
      {
        regex: /\bfn is_auth_error\b/,
        message: 'core MCP server process runtime must not own auth error classification; use the integrations helper',
      },
      {
        regex: /\bconst AUTHORIZATION_KEYS\b/,
        message: 'core MCP server process runtime must not own remote authorization key constants; use the integrations helper',
      },
      {
        regex: /\bcontains_key\("Authorization"\)/,
        message: 'core MCP server process runtime must not inline legacy authorization header fallback; use the integrations helper',
      },
      {
        regex: /\bprocess_manager::create_tokio_command\b/,
        message: 'core MCP server process facade must not spawn MCP child processes; use the integrations owner crate',
      },
      {
        regex: /\bMCPTransport::start_receive_loop\b/,
        message: 'core MCP server process facade must not own stdio receive lifecycle; use the integrations owner crate',
      },
      {
        regex: /\bMCPConnection::new_remote\b/,
        message: 'core MCP server process facade must not own remote transport lifecycle; use the integrations owner crate',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/server/manager/mod.rs',
    patterns: [
      {
        regex: /\benum ListChangedKind\b/,
        message: 'core MCP server manager must not own list-changed classification; use the integrations helper',
      },
      {
        regex: /\bresource_catalog_cache\b/,
        message: 'core MCP server manager must not own resource catalog cache state; use the integrations owner crate',
      },
      {
        regex: /\bprompt_catalog_cache\b/,
        message: 'core MCP server manager must not own prompt catalog cache state; use the integrations owner crate',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/server/manager/reconnect.rs',
    patterns: [
      {
        regex: /\bfn compute_backoff_delay\b/,
        message: 'core MCP reconnect runtime must not own backoff policy math; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/server/manager/interaction.rs',
    patterns: [
      {
        regex: /\bfn detect_list_changed_kind\b/,
        message: 'core MCP interaction runtime must not own list-changed classification; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/adapter/tool.rs',
    patterns: [
      {
        regex: /\bfn behavior_hints\b/,
        message: 'core MCP tool adapter must not own dynamic tool behavior hint rendering; use the integrations helper',
      },
      {
        regex: /\bfn truncate_for_assistant\b/,
        message: 'core MCP tool adapter must not own result truncation rendering; use the integrations helper',
      },
      {
        regex: /\bMCPToolResultContent\b/,
        message: 'core MCP tool adapter must not own MCP result content rendering; use the integrations helper',
      },
      {
        regex: /Tool '\{\}' from MCP server/,
        message: 'core MCP tool adapter must not own dynamic descriptor text; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/adapter/context.rs',
    patterns: [
      {
        regex: /\bpub struct ContextEnhancerConfig\b/,
        message: 'core MCP context provider must not own enhancer config; use the integrations helper',
      },
      {
        regex: /\bpub struct ContextEnhancer\b/,
        message: 'core MCP context provider must not own resource selection logic; use the integrations helper',
      },
      {
        regex: /\bpartial_cmp\b/,
        message: 'core MCP context provider must not own resource ranking logic; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/server/config.rs',
    patterns: [
      {
        regex: /\bpub enum MCPServerTransport\b/,
        message: 'core MCP server config facade must not redefine MCPServerTransport; use the integrations contract',
      },
      {
        regex: /\bpub struct MCPServerOAuthConfig\b/,
        message: 'core MCP server config facade must not redefine OAuth config; use the integrations contract',
      },
      {
        regex: /\bpub struct MCPServerXaaConfig\b/,
        message: 'core MCP server config facade must not redefine XAA config; use the integrations contract',
      },
      {
        regex: /\bpub struct MCPServerConfig\b/,
        message: 'core MCP server config facade must not redefine server config; use the integrations contract',
      },
      {
        regex: /\bfn default_true\b/,
        message: 'core MCP server config facade must not redefine config serde defaults; use the integrations contract',
      },
      {
        regex: /\bpub fn resolved_transport\b/,
        message: 'core MCP server config facade must not redefine transport defaults; use the integrations contract',
      },
      {
        regex: /\bpub fn validate\b/,
        message: 'core MCP server config facade must not redefine config validation; use the integrations contract',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/config/cursor_format.rs',
    patterns: [
      {
        regex: /\bfn parse_source\b/,
        message: 'core MCP cursor facade must not redefine source parsing; use the integrations contract',
      },
      {
        regex: /\bfn parse_transport\b/,
        message: 'core MCP cursor facade must not redefine transport parsing; use the integrations contract',
      },
      {
        regex: /\bfn parse_legacy_type\b/,
        message: 'core MCP cursor facade must not redefine legacy type parsing; use the integrations contract',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/config/json_config.rs',
    patterns: [
      {
        regex: /\bfn normalize_source\b/,
        message: 'core MCP JSON config facade must not redefine source normalization; use the integrations helper',
      },
      {
        regex: /\bfn normalize_transport\b/,
        message: 'core MCP JSON config facade must not redefine transport normalization; use the integrations helper',
      },
      {
        regex: /\bfn normalize_legacy_type\b/,
        message: 'core MCP JSON config facade must not redefine legacy type normalization; use the integrations helper',
      },
      {
        regex: /\bconfig_value\.get\("mcpServers"\)\.is_none\(\)/,
        message: 'core MCP JSON config facade must not inline save validation; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/config/service.rs',
    patterns: [
      {
        regex: /\bconst AUTHORIZATION_KEYS\b/,
        message: 'core MCP config service facade must not own authorization key constants; use the integrations helper',
      },
      {
        regex: /\bfn config_signature\b/,
        message: 'core MCP config service facade must not own merge signatures; use the integrations helper',
      },
      {
        regex: /\bfn precedence\b/,
        message: 'core MCP config service facade must not own merge precedence; use the integrations helper',
      },
      {
        regex: /\bfn config_authorization_from_map\b/,
        message: 'core MCP config service facade must not own authorization extraction; use the integrations helper',
      },
      {
        regex: /\bBTreeMap\b/,
        message: 'core MCP config service facade must not rebuild stable merge signatures; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/auth.rs',
    patterns: [
      {
        regex: /\bstruct VaultFile\b/,
        message: 'core MCP auth facade must not own OAuth vault storage; use the integrations owner crate',
      },
      {
        regex: /\bconst NONCE_LEN\b/,
        message: 'core MCP auth facade must not own OAuth vault encryption; use the integrations owner crate',
      },
      {
        regex: /\bfn encrypt_value\b/,
        message: 'core MCP auth facade must not own OAuth vault encryption; use the integrations owner crate',
      },
      {
        regex: /\bfn decrypt_value\b/,
        message: 'core MCP auth facade must not own OAuth vault encryption; use the integrations owner crate',
      },
      {
        regex: /\bAuthorizationManager::new\b/,
        message: 'core MCP auth facade must not assemble OAuth authorization manager internals; use the integrations owner crate',
      },
      {
        regex: /\bOAuthState::new\b/,
        message: 'core MCP auth facade must not assemble OAuth authorization state internals; use the integrations owner crate',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/protocol/transport_remote.rs',
    patterns: [
      {
        regex: /\bfn normalize_authorization_value\b/,
        message: 'core MCP remote transport must not redefine authorization normalization; use the integrations helper',
      },
      {
        regex: /starts_with\("bearer "\)/,
        message: 'core MCP remote transport must not inline bearer normalization; use the integrations helper',
      },
      {
        regex: /\bfn build_client_info\b/,
        message: 'core MCP remote transport must not own client capability construction; use the integrations helper',
      },
      {
        regex: /\bClientCapabilities::builder\b/,
        message: 'core MCP remote transport must not inline client capability construction; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?initialize_result\b/,
        message: 'core MCP remote transport must not own rmcp initialize mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?tool\b/,
        message: 'core MCP remote transport must not own rmcp tool mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?resource\b/,
        message: 'core MCP remote transport must not own rmcp resource mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?resource_content\b/,
        message: 'core MCP remote transport must not own rmcp resource content mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?prompt\b/,
        message: 'core MCP remote transport must not own rmcp prompt mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?prompt_message\b/,
        message: 'core MCP remote transport must not own rmcp prompt message mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?tool_result\b/,
        message: 'core MCP remote transport must not own rmcp tool result mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?content_block\b/,
        message: 'core MCP remote transport must not own rmcp content block mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?icons\b/,
        message: 'core MCP remote transport must not own rmcp icon mapping; use the integrations helper',
      },
      {
        regex: /\bfn map_(?:rmcp_)?annotations\b/,
        message: 'core MCP remote transport must not own rmcp annotation mapping; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mcp/protocol/jsonrpc.rs',
    patterns: [
      {
        regex: /\bfn serialize_params\b/,
        message: 'core MCP jsonrpc facade must not redefine request parameter serialization; use the integrations contract',
      },
      {
        regex: /\bpub fn create_initialize_request\b/,
        message: 'core MCP jsonrpc facade must not redefine initialize request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_resources_list_request\b/,
        message: 'core MCP jsonrpc facade must not redefine resources/list request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_resources_read_request\b/,
        message: 'core MCP jsonrpc facade must not redefine resources/read request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_prompts_list_request\b/,
        message: 'core MCP jsonrpc facade must not redefine prompts/list request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_prompts_get_request\b/,
        message: 'core MCP jsonrpc facade must not redefine prompts/get request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_tools_list_request\b/,
        message: 'core MCP jsonrpc facade must not redefine tools/list request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_tools_call_request\b/,
        message: 'core MCP jsonrpc facade must not redefine tools/call request builders; use the integrations contract',
      },
      {
        regex: /\bpub fn create_ping_request\b/,
        message: 'core MCP jsonrpc facade must not redefine ping request builders; use the integrations contract',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/remote_ssh/workspace_state.rs',
    patterns: [
      {
        regex: /\bpub const LOCAL_WORKSPACE_SSH_HOST\b/,
        message: 'core remote SSH workspace runtime must not redefine LOCAL_WORKSPACE_SSH_HOST; use the integrations contract',
      },
      {
        regex: /\bpub fn normalize_remote_workspace_path\b/,
        message: 'core remote SSH workspace runtime must not redefine remote path normalization; use the integrations contract',
      },
      {
        regex: /\bpub fn sanitize_ssh_connection_id_for_local_dir\b/,
        message: 'core remote SSH workspace runtime must not redefine SSH connection id sanitization; use the integrations contract',
      },
      {
        regex: /\bpub fn sanitize_remote_mirror_path_component\b/,
        message: 'core remote SSH workspace runtime must not redefine remote mirror path sanitization; use the integrations contract',
      },
      {
        regex: /\bpub fn sanitize_ssh_hostname_for_mirror\b/,
        message: 'core remote SSH workspace runtime must not redefine SSH hostname mirror sanitization; use the integrations contract',
      },
      {
        regex: /\bpub fn remote_root_to_mirror_subpath\b/,
        message: 'core remote SSH workspace runtime must not redefine remote mirror subpath mapping; use the integrations contract',
      },
      {
        regex: /\bpub fn workspace_logical_key\b/,
        message: 'core remote SSH workspace runtime must not redefine workspace logical keys; use the integrations contract',
      },
      {
        regex: /\bpub fn local_workspace_stable_storage_id\b/,
        message: 'core remote SSH workspace runtime must not redefine local workspace stable ids; use the integrations contract',
      },
      {
        regex: /\bpub fn remote_workspace_stable_id\b/,
        message: 'core remote SSH workspace runtime must not redefine remote workspace stable ids; use the integrations contract',
      },
      {
        regex: /\bpub fn unresolved_remote_session_storage_key\b/,
        message: 'core remote SSH workspace runtime must not redefine unresolved session keys; use the integrations contract',
      },
      {
        regex: /\bstruct RegisteredRemoteWorkspace\b/,
        message: 'core remote SSH workspace runtime must not own workspace registrations; use the integrations registry',
      },
      {
        regex: /\bpub struct RemoteWorkspaceEntry\b/,
        message: 'core remote SSH workspace runtime must not redefine workspace entries; use the integrations registry',
      },
      {
        regex: /\bpub struct RemoteWorkspaceState\b/,
        message: 'core remote SSH workspace runtime must not redefine legacy workspace state; use the integrations registry',
      },
      {
        regex: /\bregistration_matches_path\b/,
        message: 'core remote SSH workspace runtime must not own path-to-registration matching; use the integrations registry',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/remote_connect/remote_server.rs',
    patterns: [
      {
        regex: /\bpub struct ImageAttachment\b/,
        message: 'core remote-connect server must not redefine image attachment wire DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct ChatImageAttachment\b/,
        message: 'core remote-connect server must not redefine chat image wire DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct ChatMessage\b/,
        message: 'core remote-connect server must not redefine chat message wire DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct ChatMessageItem\b/,
        message: 'core remote-connect server must not redefine chat message item DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct RemoteToolStatus\b/,
        message: 'core remote-connect server must not redefine remote tool status DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct ActiveTurnSnapshot\b/,
        message: 'core remote-connect server must not redefine active turn snapshot DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct SessionInfo\b/,
        message: 'core remote-connect server must not redefine session info DTOs; use the integrations contract',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/announcement/state_store.rs',
    patterns: [
      {
        regex: /\btokio::fs\b/,
        message: 'core announcement state store facade must not own filesystem persistence; use the integrations state store',
      },
      {
        regex: /\bserde_json::to_string_pretty\b/,
        message: 'core announcement state store facade must not own state serialization; use the integrations state store',
      },
      {
        regex: /\bserde_json::from_str\b/,
        message: 'core announcement state store facade must not own state deserialization; use the integrations state store',
      },
    ],
  },
];

const forbiddenContentUnderRules = [
  {
    path: 'src/crates/product-domains/src',
    reason:
      'product-domains must not own IO/process/Git/AI/platform runtime behavior without an approved port/provider migration',
    patterns: [
      {
        regex: /\bCommand::new\(/,
        message: 'product-domains must not spawn processes; keep process execution in core/adapters',
      },
      {
        regex: /\bprocess_manager::/,
        message:
          'product-domains must not use the core process manager; keep process execution in core/adapters',
      },
      {
        regex: /\btokio::process::/,
        message: 'product-domains must not own async process execution',
      },
      {
        regex: /\btokio::fs::/,
        message:
          'product-domains must not own async storage IO; storage runtime remains in core/adapters',
      },
      {
        regex: /\bGitService::/,
        message: 'product-domains must not call concrete Git services; use reviewed ports/adapters',
      },
      {
        regex: /\b(?:AIService|AiService)::/,
        message: 'product-domains must not call concrete AI services; use reviewed ports/adapters',
      },
      {
        regex: /\breqwest::/,
        message: 'product-domains must not own network clients',
      },
      {
        regex: /\bgit2::/,
        message: 'product-domains must not own libgit2 runtime integration',
      },
      {
        regex: /\brmcp::/,
        message: 'product-domains must not own MCP runtime integration',
      },
      {
        regex: /\btauri::/,
        message: 'product-domains must not depend on Tauri platform APIs',
      },
      {
        regex: /\bAppHandle\b/,
        message: 'product-domains must not own desktop platform handles',
      },
      {
        regex: /\bstd::net::/,
        message: 'product-domains must not own network sockets',
      },
      {
        regex: /\b(?:TcpStream|UdpSocket)\b/,
        message: 'product-domains must not own network sockets',
      },
    ],
  },
  {
    path: 'src/crates/agent-tools/src',
    reason:
      'agent-tools must not own product tool manifest/exposure or GetToolSpec runtime without an approved provider migration',
    patterns: [
      {
        regex: /\bGetToolSpecTool\b/,
        message: 'GetToolSpec implementation stays in core product tool runtime',
      },
      {
        regex: /\bGET_TOOL_SPEC_TOOL_NAME\b/,
        message: 'GetToolSpec manifest insertion stays in core product tool runtime',
      },
      {
        regex: /\bmanifest_resolver\b/,
        message: 'tool manifest resolution stays in core product tool runtime',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'collapsed-tool unlock state stays in core ToolUseContext/runtime',
      },
      {
        regex: /\bToolExposure\b/,
        message: 'expanded/collapsed exposure policy stays in core until provider migration',
      },
      {
        regex: /\bToolUseContext\b/,
        message: 'ToolUseContext stays in core until a portable context port is reviewed',
      },
    ],
  },
  {
    path: 'src/crates/tool-packs/src',
    reason:
      'tool-packs must not own product tool manifest/exposure or GetToolSpec runtime without an approved provider migration',
    patterns: [
      {
        regex: /\bGetToolSpecTool\b/,
        message: 'GetToolSpec implementation stays in core product tool runtime',
      },
      {
        regex: /\bGET_TOOL_SPEC_TOOL_NAME\b/,
        message: 'GetToolSpec manifest insertion stays in core product tool runtime',
      },
      {
        regex: /\bmanifest_resolver\b/,
        message: 'tool manifest resolution stays in core product tool runtime',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'collapsed-tool unlock state stays in core ToolUseContext/runtime',
      },
      {
        regex: /\bToolExposure\b/,
        message: 'expanded/collapsed exposure policy stays in core until provider migration',
      },
    ],
  },
];

const requiredContentRules = [
  {
    path: 'src/crates/core/src/agentic/tools/registry.rs',
    reason:
      'core must continue owning product tool registry assembly until an approved product-provider migration exists',
    patterns: [
      {
        regex: /\bfn register_all_tools\b/,
        message: 'missing product tool registration owner',
      },
      {
        regex: /\bGetToolSpecTool::new\(\)/,
        message: 'missing GetToolSpec registration anchor',
      },
      {
        regex: /\bget_collapsed_tool_names\b/,
        message: 'missing collapsed-tool catalog owner',
      },
      {
        regex: /\bToolExposure::Collapsed\b/,
        message: 'missing collapsed exposure lookup',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/manifest_resolver.rs',
    reason:
      'core must continue owning prompt-visible tool manifest assembly until an approved provider migration exists',
    patterns: [
      {
        regex: /\bpub async fn resolve_tool_manifest\b/,
        message: 'missing tool manifest resolver owner',
      },
      {
        regex: /\bGET_TOOL_SPEC_TOOL_NAME\b/,
        message: 'missing GetToolSpec manifest insertion anchor',
      },
      {
        regex: /\bToolExposure::Collapsed\b/,
        message: 'missing collapsed exposure branch',
      },
      {
        regex: /\bcollapsed_tool_names\b/,
        message: 'missing collapsed-tool name tracking',
      },
      {
        regex: /Call `GetToolSpec` first/,
        message: 'missing collapsed-tool prompt stub',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/implementations/get_tool_spec_tool.rs',
    reason:
      'core must continue owning GetToolSpec runtime until an approved provider migration exists',
    patterns: [
      {
        regex: /\bpub struct GetToolSpecTool\b/,
        message: 'missing GetToolSpec owner type',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'missing collapsed-tool duplicate-load guard',
      },
      {
        regex: /\balready_loaded\b/,
        message: 'missing duplicate-load assistant result contract',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/framework.rs',
    reason:
      'core must continue owning ToolUseContext and exposure policy until a portable context port is reviewed',
    patterns: [
      {
        regex: /\bpub enum ToolExposure\b/,
        message: 'missing ToolExposure owner type',
      },
      {
        regex: /\bpub struct ToolUseContext\b/,
        message: 'missing ToolUseContext owner type',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'missing collapsed-tool unlock state',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs',
    reason:
      'core must continue owning collapsed-tool execution gating until manifest/runtime migration is reviewed',
    patterns: [
      {
        regex: /\bfn validate_collapsed_tool_usage\b/,
        message: 'missing collapsed-tool execution gate',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'missing collapsed-tool unlock state propagation',
      },
      {
        regex: /\bGetToolSpec\b/,
        message: 'missing GetToolSpec gating contract',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/execution/execution_engine.rs',
    reason:
      'core execution must continue carrying collapsed-tool unlock state and DeepResearch post-turn hooks until approved runtime migrations exist',
    patterns: [
      {
        regex: /\bfn collect_unlocked_collapsed_tools\b/,
        message: 'missing GetToolSpec result unlock collector',
      },
      {
        regex: /\bcollapsed_tool_names\b/,
        message: 'missing manifest collapsed-tool handoff',
      },
      {
        regex: /\bGetToolSpec\b/,
        message: 'missing GetToolSpec execution contract',
      },
      {
        regex: /\bcitation_renumber\b/,
        message: 'missing DeepResearch citation renumber hook',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/agents/registry/availability.rs',
    reason:
      'core agent registry must continue owning mode-scoped subagent availability until an approved agent-runtime migration exists',
    patterns: [
      {
        regex: /\bpub fn resolve_availability\b/,
        message: 'missing mode-scoped subagent availability resolver',
      },
      {
        regex: /\bpub fn resolve_override_layers\b/,
        message: 'missing project/user override layering contract',
      },
      {
        regex: /\bAgentSubagentOverrideState\b/,
        message: 'missing subagent override state contract',
      },
      {
        regex: /\bSubagentStateReason\b/,
        message: 'missing frontend-visible availability reason contract',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/agents/registry/types.rs',
    reason:
      'core agent registry must continue exposing subagent query and availability DTOs until registry ownership migrates with API equivalence tests',
    patterns: [
      {
        regex: /\bpub struct SubagentQueryContext\b/,
        message: 'missing subagent query context',
      },
      {
        regex: /\bpub enum SubagentListScope\b/,
        message: 'missing subagent list scope contract',
      },
      {
        regex: /\bdefault_enabled\b/,
        message: 'missing default availability field',
      },
      {
        regex: /\beffective_enabled\b/,
        message: 'missing effective availability field',
      },
      {
        regex: /\bpub enum SubagentStateReason\b/,
        message: 'missing availability reason wire contract',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/agents/citation_renumber.rs',
    reason:
      'core DeepResearch runtime must continue owning citation renumber post-processing until agent-runtime migration is reviewed',
    patterns: [
      {
        regex: /\bpub async fn run_for_session_workspace\b/,
        message: 'missing DeepResearch citation hook entry point',
      },
      {
        regex: /\bpub async fn try_renumber_research_report\b/,
        message: 'missing deterministic citation renumber implementation',
      },
      {
        regex: /display_map\.json/,
        message: 'missing citation display map sidecar contract',
      },
      {
        regex: /\bREJECTED\b/,
        message: 'missing rejected-citation filtering contract',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/workspace/service.rs',
    reason:
      'core workspace runtime must continue owning startup remote-workspace guards until workspace service migration is reviewed',
    patterns: [
      {
        regex: /\bprepare_startup_restored_workspaces\b/,
        message: 'missing restored-workspace startup guard',
      },
      {
        regex: /\bWorkspaceKind::Remote\b/,
        message: 'missing remote workspace branch',
      },
      {
        regex: /\bensure_remote_workspace_runtime\b/,
        message: 'missing remote workspace runtime ensure call',
      },
      {
        regex: /\bsshHost\b/,
        message: 'missing remote workspace host metadata contract',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/search/service.rs',
    reason:
      'core search runtime must continue owning local flashgrep fallback and preview mapping until search migration is reviewed',
    patterns: [
      {
        regex: /\bwith_scan_fallback\b/,
        message: 'missing flashgrep scan fallback request flag',
      },
      {
        regex: /\bconvert_hits_to_file_search_results\b/,
        message: 'missing hit-to-file-result conversion owner',
      },
      {
        regex: /\bsplit_preview\b/,
        message: 'missing preview split contract',
      },
      {
        regex: /\bpreview_inside\b/,
        message: 'missing preview-inside rendering contract',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/search/remote.rs',
    reason:
      'core remote search runtime must continue owning remote flashgrep fallback/session behavior until search migration is reviewed',
    patterns: [
      {
        regex: /\bremote_workspace_search_service_for_path\b/,
        message: 'missing remote workspace search resolver',
      },
      {
        regex: /\blookup_remote_connection_with_hint\b/,
        message: 'missing preferred remote connection lookup',
      },
      {
        regex: /\ballow_scan_fallback\b/,
        message: 'missing remote scan fallback contract',
      },
      {
        regex: /\bfallback_query\b/,
        message: 'missing FilesWithMatches fallback query',
      },
    ],
  },
  {
    path: 'src/crates/acp/src/client/manager.rs',
    reason:
      'ACP surface runtime must continue owning startup timeout diagnostics until ACP migration is reviewed',
    patterns: [
      {
        regex: /\bCLIENT_STARTUP_TIMEOUT_SECS\b/,
        message: 'missing ACP startup timeout duration contract',
      },
      {
        regex: /\bstartup_timeout_error_message\b/,
        message: 'missing ACP startup timeout diagnostic formatter',
      },
      {
        regex: /\bformats_startup_timeout_error_message\b/,
        message: 'missing ACP startup timeout regression',
      },
    ],
  },
  {
    path: 'src/web-ui/src/flow_chat/tool-cards/FileOperationToolCard.tsx',
    reason:
      'web-ui file operation surface must continue owning snapshot-to-local diff fallback until product surface migration is reviewed',
    patterns: [
      {
        regex: /\bopenLocalDiff\b/,
        message: 'missing local tool diff fallback',
      },
      {
        regex: /snapshotAPI\.getOperationDiff/,
        message: 'missing snapshot operation diff path',
      },
      {
        regex: /Snapshot diff unavailable/,
        message: 'missing snapshot-unavailable fallback diagnostic',
      },
      {
        regex: /\blocalDiffContent\b/,
        message: 'missing local diff content fallback state',
      },
    ],
  },
  {
    path: 'src/crates/core/src/miniapp/storage.rs',
    reason:
      'core must continue owning MiniApp storage runtime adapter until storage IO migration is reviewed',
    patterns: [
      {
        regex: /\bimpl MiniAppStoragePort for MiniAppStorage\b/,
        message: 'missing MiniApp storage port adapter owner',
      },
    ],
  },
  {
    path: 'src/crates/core/src/miniapp/js_worker_pool.rs',
    reason:
      'core must continue owning MiniApp worker runtime adapter until process/runtime migration is reviewed',
    patterns: [
      {
        regex: /\bimpl MiniAppRuntimePort for JsWorkerPool\b/,
        message: 'missing MiniApp runtime port adapter owner',
      },
    ],
  },
  {
    path: 'src/crates/core/src/function_agents/port_adapters.rs',
    reason:
      'core must continue owning function-agent Git runtime adapter until Git/AI service migration is reviewed',
    patterns: [
      {
        regex: /\bpub struct CoreFunctionAgentGitAdapter\b/,
        message: 'missing core function-agent Git adapter type',
      },
      {
        regex: /\bimpl FunctionAgentGitPort for CoreFunctionAgentGitAdapter\b/,
        message: 'missing function-agent Git port adapter owner',
      },
    ],
  },
];

const failures = [];

function toRepoPath(path) {
  return relative(ROOT, path).replace(/\\/g, '/');
}

function readText(path) {
  return readFileSync(path, 'utf8');
}

function walkFiles(dir, visit) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      walkFiles(path, visit);
      continue;
    }
    visit(path);
  }
}

function rustImportName(depName) {
  return depName.replace(/-/g, '_');
}

function escapeRegex(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function manifestDependencyHeaderPattern(depName) {
  const depPattern = `(?:${escapeRegex(depName)}|"${escapeRegex(depName)}")`;
  return new RegExp(
    `^\\[(?:target\\.[^\\]]+\\.)?(?:dependencies|dev-dependencies|build-dependencies)\\.${depPattern}\\]$`,
  );
}

function isManifestDependencyDeclaration(trimmedLine, depName) {
  const isInlineDependency = new RegExp(`^${escapeRegex(depName)}\\s*=`).test(trimmedLine);
  const isDependencyTable = manifestDependencyHeaderPattern(depName).test(trimmedLine);
  return isInlineDependency || isDependencyTable;
}

function isDependencyListHeader(trimmedLine) {
  return /^\[(?:target\.[^\]]+\.)?(?:dependencies|dev-dependencies|build-dependencies)\]$/.test(
    trimmedLine,
  );
}

function parseManifestDependencies(lines) {
  const deps = [];
  let inDependencyList = false;
  let currentTable = null;
  let currentInline = null;

  lines.forEach((line, index) => {
    const trimmed = line.trim();
    if (trimmed.startsWith('#') || trimmed === '') {
      return;
    }

    if (currentInline) {
      if (/\boptional\s*=\s*true\b/.test(trimmed)) {
        currentInline.optional = true;
      }
      if (trimmed.includes('}')) {
        currentInline = null;
      }
      return;
    }

    const headerMatch = trimmed.match(/^\[(.+)]$/);
    if (headerMatch) {
      inDependencyList = isDependencyListHeader(trimmed);
      currentTable = null;
      for (const depName of collectKnownDependencyNames()) {
        if (manifestDependencyHeaderPattern(depName).test(trimmed)) {
          currentTable = {
            name: depName,
            line: index + 1,
            optional: false,
          };
          deps.push(currentTable);
          break;
        }
      }
      return;
    }

    if (currentTable && /\boptional\s*=\s*true\b/.test(trimmed)) {
      currentTable.optional = true;
      return;
    }

    if (!inDependencyList) {
      return;
    }

    const inlineMatch = trimmed.match(/^([A-Za-z0-9_-]+|"[A-Za-z0-9_-]+")\s*=/);
    if (inlineMatch) {
      const name = inlineMatch[1].replace(/^"|"$/g, '');
      deps.push({
        name,
        line: index + 1,
        optional: /\boptional\s*=\s*true\b/.test(trimmed),
      });
      if (trimmed.includes('{') && !trimmed.includes('}')) {
        currentInline = deps[deps.length - 1];
      }
      return;
    }

  });

  return deps;
}

function collectKnownDependencyNames() {
  return Array.from(
    new Set([
      'bitfun-core',
      ...lightweightBoundaryRules.flatMap((rule) => rule.forbiddenDeps),
      ...dependencyProfileRules.flatMap((rule) => rule.forbiddenNonOptionalDeps),
    ]),
  );
}

function runManifestParserSelfTest() {
  const positiveCases = [
    'bitfun-core = { path = "../core" }',
    '[dependencies.bitfun-core]',
    '[dev-dependencies."bitfun-core"]',
    "[target.'cfg(windows)'.dependencies.bitfun-core]",
    "[target.'cfg(unix)'.build-dependencies.\"bitfun-core\"]",
  ];
  const negativeCases = [
    '# bitfun-core = { path = "../core" }',
    '[dependencies]',
    '[workspace.dependencies.bitfun-core]',
    '[dependencies.bitfun-core-extra]',
  ];

  for (const line of positiveCases) {
    if (!isManifestDependencyDeclaration(line, 'bitfun-core')) {
      throw new Error(`manifest parser missed dependency declaration: ${line}`);
    }
  }
  for (const line of negativeCases) {
    if (isManifestDependencyDeclaration(line, 'bitfun-core')) {
      throw new Error(`manifest parser matched non-dependency declaration: ${line}`);
    }
  }

  const parsedDeps = parseManifestDependencies([
    '[dependencies]',
    'reqwest = { workspace = true, optional = true }',
    'dirs = { workspace = true }',
    'rmcp = { version = "0.12.0", default-features = false, features = [',
    '    "auth",',
    '], optional = true }',
    '[dependencies.git2]',
    'workspace = true',
    'optional = true',
    '[target.\'cfg(windows)\'.dependencies."bitfun-cli"]',
    'path = "../../apps/cli"',
    '[features]',
    'image = []',
  ]);
  const parsedByName = new Map(parsedDeps.map((dep) => [dep.name, dep]));
  if (parsedByName.get('reqwest')?.optional !== true) {
    throw new Error('dependency profile parser must detect inline optional dependencies');
  }
  if (parsedByName.get('dirs')?.optional !== false) {
    throw new Error('dependency profile parser must detect non-optional inline dependencies');
  }
  if (parsedByName.get('rmcp')?.optional !== true) {
    throw new Error('dependency profile parser must detect multiline optional inline dependencies');
  }
  if (parsedByName.get('git2')?.optional !== true) {
    throw new Error('dependency profile parser must detect optional dependency tables');
  }
  if (parsedByName.get('bitfun-cli')?.optional !== false) {
    throw new Error('dependency profile parser must detect non-optional target dependency tables');
  }
  if (parsedByName.has('image')) {
    throw new Error('dependency profile parser must ignore feature entries named like dependencies');
  }

  const acceptsGitFacadeLine = createFacadeLineChecker('bitfun_services_integrations::git');
  const facadePositiveCases = [
    '',
    '//! Compatibility facade.',
    'pub use bitfun_services_integrations::git::GitService;',
    'pub use bitfun_services_integrations::git::types::*;',
    'pub use bitfun_services_integrations::git::{',
    '    build_git_graph, build_git_graph_for_branch,',
    '};',
    'pub use bitfun_services_integrations::git::{build_git_graph, build_git_graph_for_branch};',
  ];
  for (const line of facadePositiveCases) {
    if (!acceptsGitFacadeLine(line)) {
      throw new Error(`facade parser rejected allowed line: ${line}`);
    }
  }

  const rejectsGitImplementationLine = createFacadeLineChecker('bitfun_services_integrations::git');
  const facadeNegativeCases = [
    'pub mod service;',
    'use bitfun_services_integrations::git::GitService;',
    'fn parse_git_status() {}',
  ];
  for (const line of facadeNegativeCases) {
    if (rejectsGitImplementationLine(line)) {
      throw new Error(`facade parser accepted implementation line: ${line}`);
    }
  }

  const cliBoundaryDeps = ['bitfun-cli', 'ratatui', 'crossterm', 'arboard', 'syntect-tui'];
  for (const rule of lightweightBoundaryRules) {
    for (const dep of cliBoundaryDeps) {
      if (!rule.forbiddenDeps.includes(dep)) {
        throw new Error(
          `lightweight boundary rule for ${rule.crateName} must forbid CLI-only dependency: ${dep}`,
        );
      }
    }
  }

  const agentToolsRule = lightweightBoundaryRules.find((rule) => rule.crateName === 'agent-tools');
  if (!agentToolsRule?.forbiddenDeps.includes('bitfun-ai-adapters')) {
    throw new Error('agent-tools lightweight boundary must forbid bitfun-ai-adapters');
  }
  const coreToolFrameworkRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/agentic/tools/framework.rs',
  );
  if (!coreToolFrameworkRule) {
    throw new Error('missing core tool framework boundary rule');
  }
  const coreToolFrameworkContracts = [
    'DynamicMcpToolInfo',
    'DynamicToolInfo',
    'ToolRenderOptions',
    'ToolPathBackend',
    'ToolPathResolution',
  ];
  const coreToolFrameworkRuleText = coreToolFrameworkRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of coreToolFrameworkContracts) {
    if (!coreToolFrameworkRuleText.includes(contract)) {
      throw new Error(`core tool framework boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreToolRestrictionRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/agentic/tools/restrictions.rs',
  );
  if (!coreToolRestrictionRule) {
    throw new Error('missing core tool restrictions boundary rule');
  }
  const coreToolRestrictionContracts = [
    'ToolPathOperation',
    'ToolPathPolicy',
    'ToolRuntimeRestrictions',
  ];
  const coreToolRestrictionRuleText = coreToolRestrictionRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of coreToolRestrictionContracts) {
    if (!coreToolRestrictionRuleText.includes(contract)) {
      throw new Error(`core tool restrictions boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreToolRegistryRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/agentic/tools/registry.rs',
  );
  if (!coreToolRegistryRule) {
    throw new Error('missing core tool registry boundary rule');
  }
  const coreToolRegistryRuleText = coreToolRegistryRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  const coreToolRegistryContracts = [
    'DynamicToolMetadata',
    'tools\\s*:\\s*IndexMap',
    'dynamic_tools\\s*:\\s*IndexMap',
  ];
  for (const contract of coreToolRegistryContracts) {
    if (!coreToolRegistryRuleText.includes(contract)) {
      throw new Error(`core tool registry boundary rule must forbid contract: ${contract}`);
    }
  }

  const productDomainProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'product-domains',
  );
  if (!productDomainProfile?.forbiddenNonOptionalDeps.includes('dirs')) {
    throw new Error('product-domains default profile must forbid non-optional dirs');
  }
  const productDomainRuntimeRule = forbiddenContentUnderRules.find(
    (rule) => rule.path === 'src/crates/product-domains/src',
  );
  if (!productDomainRuntimeRule) {
    throw new Error('missing product-domains runtime-owner boundary rule');
  }
  const productDomainRuntimeContracts = [
    'Command::new\\(',
    'process_manager::',
    'GitService::',
    'reqwest::',
    'tauri::',
  ];
  const productDomainRuntimeRuleText = productDomainRuntimeRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of productDomainRuntimeContracts) {
    if (!productDomainRuntimeRuleText.includes(contract)) {
      throw new Error(`product-domains runtime boundary rule must forbid: ${contract}`);
    }
  }
  const coreTypesProfile = dependencyProfileRules.find((rule) => rule.crateName === 'core-types');
  if (!coreTypesProfile?.forbiddenNonOptionalDeps.includes('bitfun-ai-adapters')) {
    throw new Error('core-types dependency profile must forbid ai-adapter dependencies');
  }
  const runtimePortsProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'runtime-ports',
  );
  if (!runtimePortsProfile?.forbiddenNonOptionalDeps.includes('bitfun-services-core')) {
    throw new Error('runtime-ports dependency profile must forbid service implementations');
  }
  const agentToolsManifestRule = forbiddenContentUnderRules.find(
    (rule) => rule.path === 'src/crates/agent-tools/src',
  );
  if (!agentToolsManifestRule) {
    throw new Error('missing agent-tools manifest-owner boundary rule');
  }
  const toolManifestContracts = [
    'GetToolSpecTool',
    'GET_TOOL_SPEC_TOOL_NAME',
    'manifest_resolver',
    'unlocked_collapsed_tools',
  ];
  const agentToolsManifestRuleText = agentToolsManifestRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of toolManifestContracts) {
    if (!agentToolsManifestRuleText.includes(contract)) {
      throw new Error(`agent-tools manifest boundary rule must forbid: ${contract}`);
    }
  }
  const toolPacksManifestRule = forbiddenContentUnderRules.find(
    (rule) => rule.path === 'src/crates/tool-packs/src',
  );
  if (!toolPacksManifestRule) {
    throw new Error('missing tool-packs manifest-owner boundary rule');
  }
  const toolPacksManifestRuleText = toolPacksManifestRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of toolManifestContracts) {
    if (!toolPacksManifestRuleText.includes(contract)) {
      throw new Error(`tool-packs manifest boundary rule must forbid: ${contract}`);
    }
  }

  const requiredOwnerContracts = [
    {
      path: 'src/crates/core/src/agentic/tools/registry.rs',
      contracts: ['register_all_tools', 'GetToolSpecTool', 'get_collapsed_tool_names'],
    },
    {
      path: 'src/crates/core/src/agentic/tools/manifest_resolver.rs',
      contracts: ['resolve_tool_manifest', 'GET_TOOL_SPEC_TOOL_NAME', 'ToolExposure'],
    },
    {
      path: 'src/crates/core/src/agentic/tools/implementations/get_tool_spec_tool.rs',
      contracts: ['GetToolSpecTool', 'unlocked_collapsed_tools', 'already_loaded'],
    },
    {
      path: 'src/crates/core/src/agentic/tools/framework.rs',
      contracts: ['ToolExposure', 'ToolUseContext', 'unlocked_collapsed_tools'],
    },
    {
      path: 'src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs',
      contracts: ['validate_collapsed_tool_usage', 'unlocked_collapsed_tools', 'GetToolSpec'],
    },
    {
      path: 'src/crates/core/src/agentic/execution/execution_engine.rs',
      contracts: ['collect_unlocked_collapsed_tools', 'collapsed_tool_names', 'GetToolSpec', 'citation_renumber'],
    },
    {
      path: 'src/crates/core/src/agentic/agents/registry/availability.rs',
      contracts: ['resolve_availability', 'resolve_override_layers', 'AgentSubagentOverrideState', 'SubagentStateReason'],
    },
    {
      path: 'src/crates/core/src/agentic/agents/registry/types.rs',
      contracts: ['SubagentQueryContext', 'SubagentListScope', 'default_enabled', 'effective_enabled', 'SubagentStateReason'],
    },
    {
      path: 'src/crates/core/src/agentic/agents/citation_renumber.rs',
      contracts: ['run_for_session_workspace', 'try_renumber_research_report', 'display_map', 'REJECTED'],
    },
    {
      path: 'src/crates/core/src/service/workspace/service.rs',
      contracts: ['prepare_startup_restored_workspaces', 'WorkspaceKind::Remote', 'ensure_remote_workspace_runtime', 'sshHost'],
    },
    {
      path: 'src/crates/core/src/service/search/service.rs',
      contracts: ['with_scan_fallback', 'convert_hits_to_file_search_results', 'split_preview', 'preview_inside'],
    },
    {
      path: 'src/crates/core/src/service/search/remote.rs',
      contracts: ['remote_workspace_search_service_for_path', 'lookup_remote_connection_with_hint', 'allow_scan_fallback', 'fallback_query'],
    },
    {
      path: 'src/crates/acp/src/client/manager.rs',
      contracts: ['CLIENT_STARTUP_TIMEOUT_SECS', 'startup_timeout_error_message', 'formats_startup_timeout_error_message'],
    },
    {
      path: 'src/web-ui/src/flow_chat/tool-cards/FileOperationToolCard.tsx',
      contracts: ['openLocalDiff', 'snapshotAPI\\.getOperationDiff', 'Snapshot diff unavailable', 'localDiffContent'],
    },
    {
      path: 'src/crates/core/src/miniapp/storage.rs',
      contracts: ['MiniAppStoragePort'],
    },
    {
      path: 'src/crates/core/src/miniapp/js_worker_pool.rs',
      contracts: ['MiniAppRuntimePort'],
    },
    {
      path: 'src/crates/core/src/function_agents/port_adapters.rs',
      contracts: ['CoreFunctionAgentGitAdapter', 'FunctionAgentGitPort'],
    },
  ];
  for (const { path, contracts } of requiredOwnerContracts) {
    const rule = requiredContentRules.find((rule) => rule.path === path);
    if (!rule) {
      throw new Error(`missing core owner anchor rule for ${path}`);
    }
    const ruleText = rule.patterns.map((pattern) => pattern.regex.source).join('\n');
    for (const contract of contracts) {
      if (!ruleText.includes(contract)) {
        throw new Error(`core owner anchor rule for ${path} must require: ${contract}`);
      }
    }
  }

  const remoteWorkspaceRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/remote_ssh/workspace_state.rs',
  );
  if (!remoteWorkspaceRule) {
    throw new Error('missing remote SSH workspace_state boundary rule');
  }
  const remoteWorkspaceHelpers = [
    'LOCAL_WORKSPACE_SSH_HOST',
    'normalize_remote_workspace_path',
    'sanitize_ssh_connection_id_for_local_dir',
    'sanitize_remote_mirror_path_component',
    'sanitize_ssh_hostname_for_mirror',
    'remote_root_to_mirror_subpath',
    'workspace_logical_key',
    'local_workspace_stable_storage_id',
    'remote_workspace_stable_id',
    'unresolved_remote_session_storage_key',
    'RegisteredRemoteWorkspace',
    'RemoteWorkspaceEntry',
    'RemoteWorkspaceState',
    'registration_matches_path',
  ];
  const ruleText = remoteWorkspaceRule.patterns.map((pattern) => pattern.regex.source).join('\n');
  for (const helper of remoteWorkspaceHelpers) {
    if (!ruleText.includes(helper)) {
      throw new Error(`remote SSH workspace boundary rule must forbid helper: ${helper}`);
    }
  }

  const announcementStateStoreRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/announcement/state_store.rs',
  );
  if (!announcementStateStoreRule) {
    throw new Error('missing announcement state store boundary rule');
  }

  const mcpProcessRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/server/process.rs',
  );
  if (!mcpProcessRule) {
    throw new Error('missing MCP server process boundary rule');
  }
  const mcpProcessHelpers = [
    'MCPServerType',
    'MCPServerStatus',
    'is_auth_error',
    'AUTHORIZATION_KEYS',
    'contains_key\\("Authorization"\\)',
    'process_manager::create_tokio_command',
    'MCPTransport::start_receive_loop',
    'MCPConnection::new_remote',
  ];
  const mcpProcessRuleText = mcpProcessRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const helper of mcpProcessHelpers) {
    if (!mcpProcessRuleText.includes(helper)) {
      throw new Error(`MCP server process boundary rule must forbid helper: ${helper}`);
    }
  }

  const mcpManagerRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/server/manager/mod.rs',
  );
  if (!mcpManagerRule) {
    throw new Error('missing MCP server manager boundary rule');
  }
  const mcpManagerHelpers = [
    'ListChangedKind',
    'resource_catalog_cache',
    'prompt_catalog_cache',
  ];
  const mcpManagerRuleText = mcpManagerRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const helper of mcpManagerHelpers) {
    if (!mcpManagerRuleText.includes(helper)) {
      throw new Error(`MCP server manager boundary rule must forbid helper: ${helper}`);
    }
  }

  const mcpReconnectRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/server/manager/reconnect.rs',
  );
  if (!mcpReconnectRule) {
    throw new Error('missing MCP reconnect boundary rule');
  }
  if (
    !mcpReconnectRule.patterns
      .map((pattern) => pattern.regex.source)
      .join('\n')
      .includes('compute_backoff_delay')
  ) {
    throw new Error('MCP reconnect boundary rule must forbid compute_backoff_delay');
  }

  const mcpInteractionRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/server/manager/interaction.rs',
  );
  if (!mcpInteractionRule) {
    throw new Error('missing MCP interaction boundary rule');
  }
  if (
    !mcpInteractionRule.patterns
      .map((pattern) => pattern.regex.source)
      .join('\n')
      .includes('detect_list_changed_kind')
  ) {
    throw new Error('MCP interaction boundary rule must forbid detect_list_changed_kind');
  }

  const mcpToolAdapterRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/adapter/tool.rs',
  );
  if (!mcpToolAdapterRule) {
    throw new Error('missing MCP tool adapter boundary rule');
  }
  const mcpToolAdapterHelpers = [
    'behavior_hints',
    'truncate_for_assistant',
    'MCPToolResultContent',
    'Tool',
  ];
  const mcpToolAdapterRuleText = mcpToolAdapterRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const helper of mcpToolAdapterHelpers) {
    if (!mcpToolAdapterRuleText.includes(helper)) {
      throw new Error(`MCP tool adapter boundary rule must forbid helper: ${helper}`);
    }
  }

  const mcpContextAdapterRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/adapter/context.rs',
  );
  if (!mcpContextAdapterRule) {
    throw new Error('missing MCP context adapter boundary rule');
  }
  const mcpContextAdapterHelpers = [
    'ContextEnhancerConfig',
    'ContextEnhancer',
    'partial_cmp',
  ];
  const mcpContextAdapterRuleText = mcpContextAdapterRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const helper of mcpContextAdapterHelpers) {
    if (!mcpContextAdapterRuleText.includes(helper)) {
      throw new Error(`MCP context adapter boundary rule must forbid helper: ${helper}`);
    }
  }

  const mcpJsonConfigRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/config/json_config.rs',
  );
  if (!mcpJsonConfigRule) {
    throw new Error('missing MCP JSON config boundary rule');
  }
  const mcpJsonConfigHelpers = [
    'normalize_source',
    'normalize_transport',
    'normalize_legacy_type',
    'mcpServers',
  ];
  const mcpJsonConfigRuleText = mcpJsonConfigRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const helper of mcpJsonConfigHelpers) {
    if (!mcpJsonConfigRuleText.includes(helper)) {
      throw new Error(`MCP JSON config boundary rule must forbid helper: ${helper}`);
    }
  }

  const mcpConfigServiceRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/config/service.rs',
  );
  if (!mcpConfigServiceRule) {
    throw new Error('missing MCP config service boundary rule');
  }
  const mcpConfigServiceHelpers = [
    'AUTHORIZATION_KEYS',
    'config_signature',
    'precedence',
    'config_authorization_from_map',
    'BTreeMap',
  ];
  const mcpConfigServiceRuleText = mcpConfigServiceRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const helper of mcpConfigServiceHelpers) {
    if (!mcpConfigServiceRuleText.includes(helper)) {
      throw new Error(`MCP config service boundary rule must forbid helper: ${helper}`);
    }
  }

  const mcpAuthRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/auth.rs',
  );
  if (!mcpAuthRule) {
    throw new Error('missing MCP auth boundary rule');
  }
  const mcpAuthHelpers = [
    'VaultFile',
    'NONCE_LEN',
    'encrypt_value',
    'decrypt_value',
    'AuthorizationManager::new',
    'OAuthState::new',
  ];
  const mcpAuthRuleText = mcpAuthRule.patterns.map((pattern) => pattern.regex.source).join('\n');
  for (const helper of mcpAuthHelpers) {
    if (!mcpAuthRuleText.includes(escapeRegex(helper))) {
      throw new Error(`MCP auth boundary rule must forbid helper: ${helper}`);
    }
  }

  const mcpRemoteTransportRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/protocol/transport_remote.rs',
  );
  if (!mcpRemoteTransportRule) {
    throw new Error('missing MCP remote transport boundary rule');
  }
  const mcpRemoteTransportHelpers = [
    'normalize_authorization_value',
    'starts_with\\("bearer "\\)',
    'build_client_info',
    'ClientCapabilities::builder',
    'map_(?:rmcp_)?initialize_result',
    'map_(?:rmcp_)?tool',
    'map_(?:rmcp_)?resource',
    'map_(?:rmcp_)?resource_content',
    'map_(?:rmcp_)?prompt',
    'map_(?:rmcp_)?prompt_message',
    'map_(?:rmcp_)?tool_result',
    'map_(?:rmcp_)?content_block',
    'map_(?:rmcp_)?icons',
    'map_(?:rmcp_)?annotations',
  ];
  const mcpRemoteTransportRuleText = mcpRemoteTransportRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const helper of mcpRemoteTransportHelpers) {
    if (!mcpRemoteTransportRuleText.includes(helper)) {
      throw new Error(`MCP remote transport boundary rule must forbid helper: ${helper}`);
    }
  }

  const mcpJsonrpcRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/protocol/jsonrpc.rs',
  );
  if (!mcpJsonrpcRule) {
    throw new Error('missing MCP JSON-RPC boundary rule');
  }
  const mcpJsonrpcHelpers = [
    'serialize_params',
    'create_initialize_request',
    'create_resources_list_request',
    'create_resources_read_request',
    'create_prompts_list_request',
    'create_prompts_get_request',
    'create_tools_list_request',
    'create_tools_call_request',
    'create_ping_request',
  ];
  const mcpJsonrpcRuleText = mcpJsonrpcRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const helper of mcpJsonrpcHelpers) {
    if (!mcpJsonrpcRuleText.includes(helper)) {
      throw new Error(`MCP JSON-RPC boundary rule must forbid helper: ${helper}`);
    }
  }

  const mcpServerConfigRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/mcp/server/config.rs',
  );
  if (!mcpServerConfigRule) {
    throw new Error('missing MCP server config boundary rule');
  }
  const mcpServerConfigContracts = [
    'MCPServerTransport',
    'MCPServerOAuthConfig',
    'MCPServerXaaConfig',
    'MCPServerConfig',
    'default_true',
    'resolved_transport',
    'validate',
  ];
  const mcpServerConfigRuleText = mcpServerConfigRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of mcpServerConfigContracts) {
    if (!mcpServerConfigRuleText.includes(contract)) {
      throw new Error(`MCP server config boundary rule must forbid contract: ${contract}`);
    }
  }

  const servicesIntegrationsProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'services-integrations',
  );
  for (const dep of ['futures', 'reqwest', 'sse-stream']) {
    if (!servicesIntegrationsProfile?.forbiddenNonOptionalDeps.includes(dep)) {
      throw new Error(`services-integrations default profile must forbid non-optional ${dep}`);
    }
  }

  const remoteConnectRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/remote_connect/remote_server.rs',
  );
  if (!remoteConnectRule) {
    throw new Error('missing remote-connect remote_server boundary rule');
  }
  const remoteConnectContracts = [
    'ImageAttachment',
    'ChatImageAttachment',
    'ChatMessage',
    'ChatMessageItem',
    'RemoteToolStatus',
    'ActiveTurnSnapshot',
    'SessionInfo',
  ];
  const remoteConnectRuleText = remoteConnectRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of remoteConnectContracts) {
    if (!remoteConnectRuleText.includes(contract)) {
      throw new Error(`remote-connect boundary rule must forbid contract: ${contract}`);
    }
  }

  const facadePaths = new Set(facadeOnlyFiles.map((facade) => facade.path));
  for (const path of [
    'src/crates/core/src/service/mcp/protocol/transport.rs',
    'src/crates/core/src/service/mcp/protocol/transport_remote.rs',
    'src/crates/core/src/service/mcp/server/connection.rs',
  ]) {
    if (!facadePaths.has(path)) {
      throw new Error(`missing MCP runtime facade-only rule for ${path}`);
    }
  }
}

function checkCargoManifest(crateDir) {
  checkForbiddenManifestDeps(crateDir, ['bitfun-core'], () => {
    return 'extracted crate must not depend on bitfun-core';
  });
}

function checkForbiddenManifestDeps(crateDir, forbiddenDeps, messageForDep) {
  const manifestPath = join(crateDir, 'Cargo.toml');
  const lines = readText(manifestPath).split(/\r?\n/);
  lines.forEach((line, index) => {
    const trimmed = line.trim();
    if (trimmed.startsWith('#')) {
      return;
    }
    for (const dep of forbiddenDeps) {
      if (isManifestDependencyDeclaration(trimmed, dep)) {
        failures.push({
          path: manifestPath,
          line: index + 1,
          message: messageForDep(dep),
        });
      }
    }
  });
}

function checkForbiddenNonOptionalManifestDeps(crateDir, forbiddenDeps, messageForDep) {
  const manifestPath = join(crateDir, 'Cargo.toml');
  const deps = parseManifestDependencies(readText(manifestPath).split(/\r?\n/));
  for (const dep of deps) {
    if (!dep.optional && forbiddenDeps.includes(dep.name)) {
      failures.push({
        path: manifestPath,
        line: dep.line,
        message: messageForDep(dep.name),
      });
    }
  }
}

function checkRustImports(crateDir) {
  const srcDir = join(crateDir, 'src');
  try {
    if (!statSync(srcDir).isDirectory()) {
      return;
    }
  } catch {
    return;
  }

  walkFiles(srcDir, (path) => {
    if (!path.endsWith('.rs')) {
      return;
    }
    const lines = readText(path).split(/\r?\n/);
    lines.forEach((line, index) => {
      if (/\bbitfun_core::/.test(line)) {
        failures.push({
          path,
          line: index + 1,
          message: 'extracted crate must not import bitfun_core',
        });
      }
    });
  });
}

function checkForbiddenRustImports(crateDir, forbiddenDeps, messageForDep) {
  const srcDir = join(crateDir, 'src');
  try {
    if (!statSync(srcDir).isDirectory()) {
      return;
    }
  } catch {
    return;
  }

  const forbiddenImports = forbiddenDeps.map((dep) => ({
    dep,
    pattern: new RegExp(`\\b${escapeRegex(rustImportName(dep))}::`),
  }));

  walkFiles(srcDir, (path) => {
    if (!path.endsWith('.rs')) {
      return;
    }
    const lines = readText(path).split(/\r?\n/);
    lines.forEach((line, index) => {
      for (const forbidden of forbiddenImports) {
        if (forbidden.pattern.test(line)) {
          failures.push({
            path,
            line: index + 1,
            message: messageForDep(forbidden.dep),
          });
        }
      }
    });
  });
}

function createFacadeLineChecker(importPrefix) {
  let inPubUseBlock = false;
  const escapedPrefix = escapeRegex(importPrefix);
  const singleReexportPattern = new RegExp(
    `^pub use ${escapedPrefix}(?:::[A-Za-z_][A-Za-z0-9_]*)*(?:::\\*)?;$`,
  );
  const blockItemPattern = /^[A-Za-z_][A-Za-z0-9_]*(?:,\s*[A-Za-z_][A-Za-z0-9_]*)*,?$/;
  const blockStart = `pub use ${importPrefix}::{`;

  const checker = (line) => {
    const trimmed = line.trim();
    if (
      trimmed === '' ||
      trimmed.startsWith('//') ||
      trimmed.startsWith('/*') ||
      trimmed.startsWith('*') ||
      trimmed.startsWith('*/')
    ) {
      return true;
    }

    if (inPubUseBlock) {
      if (trimmed === '};') {
        inPubUseBlock = false;
        return true;
      }
      return blockItemPattern.test(trimmed);
    }

    if (singleReexportPattern.test(trimmed)) {
      return true;
    }

    if (trimmed.startsWith(blockStart)) {
      if (trimmed.endsWith('};')) {
        return true;
      }
      if (trimmed.endsWith('{')) {
        inPubUseBlock = true;
        return true;
      }
    }

    return false;
  };

  checker.isComplete = () => !inPubUseBlock;
  return checker;
}

function checkFacadeOnlyFile(repoPath, importPrefix, reason) {
  const path = join(ROOT, ...repoPath.split('/'));
  const acceptsLine = createFacadeLineChecker(importPrefix);
  const lines = readText(path).split(/\r?\n/);
  lines.forEach((line, index) => {
    if (!acceptsLine(line)) {
      failures.push({
        path,
        line: index + 1,
        message: reason,
      });
    }
  });

  if (!acceptsLine.isComplete()) {
    failures.push({
      path,
      line: lines.length,
      message: `${reason}; unterminated pub use block`,
    });
  }
}

function checkForbiddenContent(repoPath, patterns) {
  const path = join(ROOT, ...repoPath.split('/'));
  const lines = readText(path).split(/\r?\n/);
  lines.forEach((line, index) => {
    for (const pattern of patterns) {
      if (pattern.regex.test(line)) {
        failures.push({
          path,
          line: index + 1,
          message: pattern.message,
        });
      }
    }
  });
}

function checkRequiredContent(repoPath, patterns, reason) {
  const path = join(ROOT, ...repoPath.split('/'));
  const text = readText(path);
  for (const pattern of patterns) {
    if (!pattern.regex.test(text)) {
      failures.push({
        path,
        line: 1,
        message: `${reason}; ${pattern.message}`,
      });
    }
  }
}

function checkForbiddenContentUnder(repoDir, patterns, reason) {
  const dir = join(ROOT, ...repoDir.split('/'));
  walkFiles(dir, (path) => {
    if (!path.endsWith('.rs')) {
      return;
    }
    const lines = readText(path).split(/\r?\n/);
    lines.forEach((line, index) => {
      for (const pattern of patterns) {
        if (pattern.regex.test(line)) {
          failures.push({
            path,
            line: index + 1,
            message: `${reason}; ${pattern.message}`,
          });
        }
      }
    });
  });
}

if (process.env.BITFUN_BOUNDARY_CHECK_SELF_TEST === '1') {
  runManifestParserSelfTest();
  console.log('Core boundary check self-test passed.');
  process.exit(0);
}

for (const crateName of noCoreDependencyCrates) {
  const crateDir = join(ROOT, 'src', 'crates', crateName);
  checkCargoManifest(crateDir);
  checkRustImports(crateDir);
}

for (const rule of lightweightBoundaryRules) {
  const crateDir = join(ROOT, 'src', 'crates', rule.crateName);
  const messageForDep = (dep) => `${rule.reason}; forbidden dependency: ${dep}`;
  checkForbiddenManifestDeps(crateDir, rule.forbiddenDeps, messageForDep);
  checkForbiddenRustImports(crateDir, rule.forbiddenDeps, messageForDep);
}

for (const rule of dependencyProfileRules) {
  const crateDir = join(ROOT, 'src', 'crates', rule.crateName);
  const messageForDep = (dep) =>
    `${rule.reason}; ${rule.profileName} forbids non-optional dependency: ${dep}`;
  checkForbiddenNonOptionalManifestDeps(crateDir, rule.forbiddenNonOptionalDeps, messageForDep);
}

for (const facade of facadeOnlyFiles) {
  checkFacadeOnlyFile(facade.path, facade.importPrefix, facade.reason);
}

for (const rule of forbiddenContentRules) {
  checkForbiddenContent(rule.path, rule.patterns);
}

for (const rule of forbiddenContentUnderRules) {
  checkForbiddenContentUnder(rule.path, rule.patterns, rule.reason);
}

for (const rule of requiredContentRules) {
  checkRequiredContent(rule.path, rule.patterns, rule.reason);
}

if (failures.length > 0) {
  console.error('Core boundary check failed.');
  for (const failure of failures) {
    console.error(`${toRepoPath(failure.path)}:${failure.line}: ${failure.message}`);
  }
  process.exit(1);
}

console.log('Core boundary check passed.');
