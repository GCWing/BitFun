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
      'dunce',
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
      {
        regex: /\bpub struct ToolContextFacts\b/,
        message: 'core tool framework must not redefine ToolContextFacts; use bitfun-agent-tools',
      },
      {
        regex: /\bpub enum ToolWorkspaceKind\b/,
        message: 'core tool framework must not redefine ToolWorkspaceKind; use bitfun-agent-tools',
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
    path: 'src/crates/core/src/function_agents/git-func-agent/commit_generator.rs',
    patterns: [
      {
        regex: /\bGitService::get_status\b/,
        message:
          'Git function-agent commit generator must use CoreProductDomainRuntime for Git adapter wiring',
      },
      {
        regex: /\bAIAnalysisService::new_with_agent_config\b/,
        message:
          'Git function-agent commit generator must use CoreProductDomainRuntime for AI adapter wiring',
      },
      {
        regex: /\bto_string_lossy\b/,
        message:
          'Git function-agent commit generator must preserve PathBuf paths when routing through the facade',
      },
    ],
  },
  {
    path: 'src/crates/core/src/function_agents/startchat-func-agent/work_state_analyzer.rs',
    patterns: [
      {
        regex: /\bAIWorkStateService::new_with_agent_config\b/,
        message:
          'Startchat work-state analyzer must use CoreProductDomainRuntime for AI adapter wiring',
      },
      {
        regex: /\bcreate_command\("git"\)/,
        message:
          'Startchat work-state analyzer must use CoreProductDomainRuntime for Git adapter wiring',
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
      {
        regex: /\bdunce::canonicalize\b/,
        message: 'core remote SSH workspace runtime must not own local root canonicalization; use the integrations path helper',
      },
      {
        regex: /\bfn path_buf_to_stable_local_root_string\b/,
        message: 'core remote SSH workspace runtime must not own local root string normalization; use the integrations path helper',
      },
      {
        regex: /join\("_unresolved"\)/,
        message: 'core remote SSH workspace runtime must not own unresolved session path layout; use the integrations path helper',
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
      {
        regex: /\bpub struct RemoteDefaultModelsConfig\b/,
        message: 'core remote-connect server must not redefine remote model default DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct RemoteModelConfig\b/,
        message: 'core remote-connect server must not redefine remote model DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct RemoteModelCatalog\b/,
        message: 'core remote-connect server must not redefine remote model catalog DTOs; use the integrations contract',
      },
      {
        regex: /\bpub struct RemoteModelCatalogPollDelta\b/,
        message: 'core remote-connect server must not redefine remote model catalog poll delta; use the integrations contract',
      },
      {
        regex: /\bpub enum RemoteCommand\b/,
        message: 'core remote-connect server must not redefine remote command wire DTOs; use the integrations contract',
      },
      {
        regex: /\bpub enum RemoteResponse\b/,
        message: 'core remote-connect server must not redefine remote response wire DTOs; use the integrations contract',
      },
      {
        regex: /\bstruct TrackerState\b/,
        message: 'core remote-connect server must not own tracker state; use the integrations tracker',
      },
      {
        regex: /\bpub enum TrackerEvent\b/,
        message: 'core remote-connect server must not redefine tracker events; use the integrations tracker',
      },
      {
        regex: /\bpub struct RemoteSessionStateTracker\b/,
        message: 'core remote-connect server must not own tracker state; use the integrations tracker',
      },
      {
        regex: /\bDashMap\b/,
        message: 'core remote-connect server must not own tracker storage; use the integrations registry',
      },
      {
        regex: /\bfn make_slim_params\b/,
        message: 'core remote-connect server must not own remote tool preview slimming; use the integrations helper',
      },
      {
        regex: /\bmatch mobile_type\b/,
        message: 'core remote-connect server must not own remote agent type alias mapping; use the integrations helper',
      },
      {
        regex: /\bfn resolve_remote_cancel_decision\b/,
        message: 'core remote-connect server must not own cancel decision policy; use the integrations helper',
      },
      {
        regex: /\benum RemoteCancelDecision\b/,
        message: 'core remote-connect server must not own cancel decision types; use the integrations contract',
      },
      {
        regex: /\bfn remote_session_restore_target\b/,
        message: 'core remote-connect server must not own restore-target policy; use the integrations helper',
      },
      {
        regex: /\bfn resolve_remote_execution_image_contexts\b/,
        message: 'core remote-connect server must not own image-context preference policy; use the integrations helper',
      },
      {
        regex: /\btrait RemoteImageContextAdapter\b/,
        message: 'core remote-connect server must not own image-context adapter contracts; use the integrations contract',
      },
      {
        regex: /\bconst MAX_SIZE\b/,
        message: 'core remote-connect server must not own remote file max-read policy; use the integrations helper',
      },
      {
        regex: /\bconst MAX_CHUNK\b/,
        message: 'core remote-connect server must not own remote file chunk policy; use the integrations helper',
      },
      {
        regex: /unwrap_or\("file"\)/,
        message: 'core remote-connect server must not own remote file display-name fallback; use the integrations helper',
      },
      {
        regex: /\bresolve_workspace_path\b/,
        message: 'core remote-connect server must not own workspace file path resolution; use the integrations helper',
      },
      {
        regex: /\bdetect_mime_type\b/,
        message: 'core remote-connect server must not own workspace file MIME detection; use the integrations helper',
      },
      {
        regex: /\bread_workspace_file\b/,
        message: 'core remote-connect server must not own workspace file read helpers; use the integrations helper',
      },
      {
        regex: /\bfn read_remote_workspace_file\b/,
        message: 'core remote-connect server must not redefine remote workspace full-file readers; use the integrations helper',
      },
      {
        regex: /\bfn read_remote_workspace_file_chunk\b/,
        message: 'core remote-connect server must not redefine remote workspace chunk readers; use the integrations helper',
      },
      {
        regex: /\bfn read_remote_workspace_file_info\b/,
        message: 'core remote-connect server must not redefine remote workspace file-info readers; use the integrations helper',
      },
      {
        regex: /\bfn should_send_remote_model_catalog\b/,
        message: 'core remote-connect server must not own poll model-catalog policy; use the integrations helper',
      },
      {
        regex: /\bfn remote_model_catalog_poll_delta\b/,
        message: 'core remote-connect server must not own poll model-catalog delta policy; use the integrations helper',
      },
      {
        regex: /\bfn remote_no_change_poll_response\b/,
        message: 'core remote-connect server must not own no-change poll response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_snapshot_poll_response\b/,
        message: 'core remote-connect server must not own streaming poll response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_persisted_poll_response\b/,
        message: 'core remote-connect server must not own persisted poll response assembly; use the integrations helper',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/remote_connect/bot/mod.rs',
    patterns: [
      {
        regex: /\bfn strip_workspace_path_prefix\b/,
        message: 'core remote-connect bot facade must not own workspace path prefix stripping; use the integrations helper',
      },
      {
        regex: /\bfn is_absolute_workspace_path\b/,
        message: 'core remote-connect bot facade must not own workspace path absolute detection; use the integrations helper',
      },
      {
        regex: /\bmatch ext\.as_str\(\)/,
        message: 'core remote-connect bot facade must not own workspace file MIME mapping; use the integrations helper',
      },
      {
        regex: /\btokio::fs::read\(&abs_path\)/,
        message: 'core remote-connect bot facade must not own workspace file reads; use the integrations helper',
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
      'agent-tools may own pure tool manifest contracts, but not product manifest runtime or GetToolSpec execution without an approved provider migration',
    patterns: [
      {
        regex: /\bGetToolSpecTool\b/,
        message: 'GetToolSpec implementation stays in core product tool runtime',
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
        regex: /\bToolUseContext\b/,
        message: 'ToolUseContext stays in core until a portable context port is reviewed',
      },
    ],
  },
  {
    path: 'src/crates/tool-packs/src',
    reason:
      'tool-packs may own provider group plans, but not product tool manifest/exposure or GetToolSpec runtime',
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
    path: 'src/crates/runtime-ports/src/lib.rs',
    reason:
      'runtime-ports must keep remote runtime boundary contracts DTO/trait-only',
    patterns: [
      {
        regex: /\bpub trait AgentTurnCancellationPort\b/,
        message: 'missing turn cancellation port contract',
      },
      {
        regex: /\bpub trait RemoteControlStatePort\b/,
        message: 'missing remote control state port contract',
      },
      {
        regex: /\bpub trait RuntimeEventSink\b/,
        message: 'missing runtime event sink contract',
      },
      {
        regex: /\bpub fn remote_image\b/,
        message: 'missing remote image attachment helper contract',
      },
    ],
  },
  {
    path: 'src/crates/agent-tools/src/framework.rs',
    reason:
      'agent-tools may own pure and generic prompt-visible tool manifest contracts without owning product registry or execution',
    patterns: [
      {
        regex: /\bpub const GET_TOOL_SPEC_TOOL_NAME\b/,
        message: 'missing shared GetToolSpec manifest name contract',
      },
      {
        regex: /\bpub enum ToolExposure\b/,
        message: 'missing lightweight tool exposure contract',
      },
      {
        regex: /\bpub struct ToolManifestPolicyTool\b/,
        message: 'missing pure tool manifest policy input contract',
      },
      {
        regex: /\bpub fn resolve_tool_manifest_policy\b/,
        message: 'missing pure tool manifest policy resolver',
      },
      {
        regex: /\bfn default_exposure\b/,
        message: 'missing generic tool exposure contract',
      },
      {
        regex: /\bpub fn build_tool_manifest_policy_tools\b/,
        message: 'missing registry snapshot to manifest policy input helper',
      },
      {
        regex: /\bpub fn build_collapsed_tool_stub_definition\b/,
        message: 'missing collapsed-tool prompt stub contract',
      },
      {
        regex: /\bpub enum PromptVisibleToolManifestItem\b/,
        message: 'missing prompt-visible manifest item contract',
      },
      {
        regex: /\bpub fn build_prompt_visible_tool_manifest_definitions\b/,
        message: 'missing prompt-visible manifest definition builder',
      },
      {
        regex: /\bpub trait ContextualToolManifestItem\b/,
        message: 'missing generic contextual manifest item adapter contract',
      },
      {
        regex: /\bpub trait ToolCatalogSnapshotProvider\b/,
        message: 'missing generic tool catalog snapshot provider contract',
      },
      {
        regex: /\bpub trait GetToolSpecCatalogProvider\b/,
        message: 'missing generic GetToolSpec catalog provider contract',
      },
      {
        regex: /\bpub struct ContextualVisibleTools\b/,
        message: 'missing generic contextual visible-tools result contract',
      },
      {
        regex: /\bpub struct ContextualToolManifest\b/,
        message: 'missing generic contextual tool manifest result contract',
      },
      {
        regex: /\bpub async fn resolve_contextual_visible_tools\b/,
        message: 'missing generic contextual visible-tools resolver',
      },
      {
        regex: /\bpub async fn resolve_contextual_tool_manifest\b/,
        message: 'missing generic contextual tool manifest resolver',
      },
      {
        regex: /\bpub async fn resolve_contextual_visible_tools_from_provider\b/,
        message: 'missing provider-backed contextual visible-tools resolver',
      },
      {
        regex: /\bpub async fn resolve_contextual_tool_manifest_from_provider\b/,
        message: 'missing provider-backed contextual manifest resolver',
      },
      {
        regex: /\bpub async fn build_get_tool_spec_catalog_description_from_provider\b/,
        message: 'missing provider-backed GetToolSpec catalog description builder',
      },
      {
        regex: /\bpub async fn resolve_get_tool_spec_detail_from_provider\b/,
        message: 'missing provider-backed GetToolSpec detail resolver',
      },
      {
        regex: /\bpub fn build_get_tool_spec_description\b/,
        message: 'missing pure GetToolSpec prompt description contract',
      },
      {
        regex: /\bpub struct GetToolSpecCollapsedToolSummary\b/,
        message: 'missing pure GetToolSpec collapsed catalog summary',
      },
      {
        regex: /\bpub struct GetToolSpecDetail\b/,
        message: 'missing pure GetToolSpec detail contract',
      },
      {
        regex: /\bpub fn summarize_get_tool_spec_collapsed_tools\b/,
        message: 'missing pure GetToolSpec collapsed summary helper',
      },
      {
        regex: /\bpub async fn resolve_get_tool_spec_detail\b/,
        message: 'missing generic GetToolSpec detail resolver',
      },
      {
        regex: /\bpub fn build_get_tool_spec_catalog_description\b/,
        message: 'missing pure GetToolSpec catalog description builder',
      },
      {
        regex: /\bpub fn get_tool_spec_input_schema\b/,
        message: 'missing pure GetToolSpec input schema contract',
      },
      {
        regex: /\bpub fn get_tool_spec_short_description\b/,
        message: 'missing pure GetToolSpec short description contract',
      },
      {
        regex: /\bpub fn render_get_tool_spec_tool_use_message\b/,
        message: 'missing pure GetToolSpec tool-use message renderer',
      },
      {
        regex: /\bpub fn get_tool_spec_is_readonly\b/,
        message: 'missing pure GetToolSpec readonly metadata contract',
      },
      {
        regex: /\bpub fn get_tool_spec_is_concurrency_safe\b/,
        message: 'missing pure GetToolSpec concurrency metadata contract',
      },
      {
        regex: /\bpub fn get_tool_spec_needs_permissions\b/,
        message: 'missing pure GetToolSpec permission metadata contract',
      },
      {
        regex: /\bpub fn validate_get_tool_spec_input\b/,
        message: 'missing pure GetToolSpec input validation contract',
      },
      {
        regex: /\bpub fn build_get_tool_spec_assistant_detail\b/,
        message: 'missing pure GetToolSpec assistant detail rendering contract',
      },
      {
        regex: /\bpub fn build_get_tool_spec_duplicate_load_result\b/,
        message: 'missing pure GetToolSpec duplicate-load result assembly contract',
      },
      {
        regex: /\bpub fn build_get_tool_spec_detail_result\b/,
        message: 'missing pure GetToolSpec detail result assembly contract',
      },
      {
        regex: /\bpub enum GetToolSpecExecutionPlan\b/,
        message: 'missing pure GetToolSpec execution plan contract',
      },
      {
        regex: /\bpub enum GetToolSpecExecutionError\b/,
        message: 'missing pure GetToolSpec execution error contract',
      },
      {
        regex: /\bpub fn resolve_get_tool_spec_execution_plan\b/,
        message: 'missing pure GetToolSpec execution plan resolver',
      },
      {
        regex: /\bpub async fn resolve_get_tool_spec_execution_result_from_provider\b/,
        message: 'missing provider-backed GetToolSpec execution result resolver',
      },
      {
        regex: /\bpub struct GetToolSpecRuntime\b/,
        message: 'missing provider-backed GetToolSpec runtime facade',
      },
      {
        regex: /\bpub async fn call_results\b/,
        message: 'missing provider-backed GetToolSpec Tool-result vector adapter facade',
      },
      {
        regex: /\bpub struct GetToolSpecLoadObservation\b/,
        message: 'missing pure GetToolSpec load observation contract',
      },
      {
        regex: /\bpub fn collect_loaded_collapsed_tool_names\b/,
        message: 'missing pure collapsed-tool load collection contract',
      },
      {
        regex: /\bpub fn sort_tool_manifest_definitions\b/,
        message: 'missing prompt-visible manifest ordering helper',
      },
      {
        regex: /\bpub struct StaticToolProviderGroup\b/,
        message: 'missing generic static provider group container',
      },
      {
        regex: /\bpub struct ToolRuntimeAssembly\b/,
        message: 'missing generic tool runtime assembly owner',
      },
      {
        regex: /\bpub type ToolDecoratorRef\b/,
        message: 'missing generic tool decorator reference contract',
      },
      {
        regex: /\bpub trait SnapshotToolWrapper\b/,
        message: 'missing generic snapshot wrapper port',
      },
      {
        regex: /\bpub struct SnapshotToolDecorator\b/,
        message: 'missing generic snapshot decorator adapter',
      },
      {
        regex: /\bcreate_registry_from_static_providers\b/,
        message: 'missing generic static-provider runtime assembly helper',
      },
      {
        regex: /\bpub fn is_tool_collapsed\b/,
        message: 'missing generic collapsed-tool registry query',
      },
      {
        regex: /\bpub fn get_collapsed_tool_names\b/,
        message: 'missing generic collapsed-tool registry catalog query',
      },
      {
        regex: /\bpub async fn resolve_readonly_enabled_tools\b/,
        message: 'missing generic readonly enabled tool filter',
      },
      {
        regex: /\bpub struct ToolCatalogRuntime\b/,
        message: 'missing provider-backed tool catalog runtime facade',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/coordination/coordinator.rs',
    reason:
      'core must keep current coordinator port adapters and attachment guard until remote runtime migration is reviewed',
    patterns: [
      {
        regex: /impl bitfun_runtime_ports::AgentSubmissionPort for ConversationCoordinator/,
        message: 'missing agent submission port adapter',
      },
      {
        regex: /impl bitfun_runtime_ports::SessionTranscriptReader for ConversationCoordinator/,
        message: 'missing session transcript reader adapter',
      },
      {
        regex: /impl bitfun_runtime_ports::AgentTurnCancellationPort for ConversationCoordinator/,
        message: 'missing turn cancellation port adapter',
      },
      {
        regex: /impl bitfun_runtime_ports::RemoteControlStatePort for ConversationCoordinator/,
        message: 'missing remote control state port adapter',
      },
      {
        regex: /agent submission port does not yet accept generic attachments/,
        message: 'missing generic attachment guard on agent submission port',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service_agent_runtime.rs',
    reason:
      'core service/agent runtime owner must centralize concrete remote-connect and agent runtime port bindings without moving runtime behavior',
    patterns: [
      {
        regex: /\bpub\(crate\) struct CoreServiceAgentRuntime\b/,
        message: 'missing core service/agent runtime owner type',
      },
      {
        regex: /\bfn remote_dialog_host\b/,
        message: 'missing remote dialog host owner factory',
      },
      {
        regex: /\bfn remote_image_context\b/,
        message: 'missing remote image context owner adapter',
      },
      {
        regex: /\bfn agent_submission_port\b/,
        message: 'missing agent submission port owner binding',
      },
      {
        regex: /\bCoreRemoteDialogRuntimeHost\b/,
        message: 'missing core remote dialog host binding',
      },
      {
        regex: /\bRemoteExecutionDispatcher\b/,
        message: 'missing remote execution dispatcher binding',
      },
      {
        regex: /\bImageContextData\b/,
        message: 'missing core image context binding',
      },
      {
        regex: /\bRemoteImageContextAdapter\b/,
        message: 'missing remote image context adapter implementation',
      },
      {
        regex: /\bAgentSubmissionPort\b/,
        message: 'missing agent submission port binding',
      },
      {
        regex: /\bAgentTurnCancellationPort\b/,
        message: 'missing agent turn cancellation port contract guard',
      },
      {
        regex: /\bRemoteControlStatePort\b/,
        message: 'missing remote control state port contract guard',
      },
      {
        regex: /\bSessionTranscriptReader\b/,
        message: 'missing session transcript reader contract guard',
      },
      {
        regex: /\bcore_service_agent_runtime_owner_keeps_coordinator_port_contracts\b/,
        message: 'missing coordinator runtime port contract regression',
      },
    ],
  },
  {
    path: 'src/crates/services-integrations/src/remote_connect.rs',
    reason:
      'services-integrations must own remote-connect wire, tracker, dialog, file, and image adapter contracts',
    patterns: [
      {
        regex: /\bpub struct RemoteSessionStateTracker\b/,
        message: 'missing remote session state tracker owner',
      },
      {
        regex: /\bpub enum TrackerEvent\b/,
        message: 'missing remote tracker event owner',
      },
      {
        regex: /\bpub trait RemoteSessionTrackerHost\b/,
        message: 'missing remote tracker host port',
      },
      {
        regex: /\bpub struct RemoteSessionTrackerRegistry\b/,
        message: 'missing remote tracker registry owner',
      },
      {
        regex: /\bpub fn make_slim_tool_params\b/,
        message: 'missing remote tool preview slimming helper',
      },
      {
        regex: /\bfn handle_agentic_event\b/,
        message: 'missing tracker event reducer',
      },
      {
        regex: /\bpub fn resolve_remote_agent_type\b/,
        message: 'missing remote agent type helper',
      },
      {
        regex: /\bpub struct RemoteImageContext\b/,
        message: 'missing portable remote image context contract',
      },
      {
        regex: /\bpub trait RemoteImageContextAdapter\b/,
        message: 'missing remote image context adapter contract',
      },
      {
        regex: /\bpub fn build_remote_image_contexts\b/,
        message: 'missing legacy remote image context builder',
      },
      {
        regex: /\bpub fn resolve_remote_execution_image_contexts\b/,
        message: 'missing remote image context preference helper',
      },
      {
        regex: /\bpub fn remote_session_restore_target\b/,
        message: 'missing remote restore-target helper',
      },
      {
        regex: /\bpub enum RemoteCancelDecision\b/,
        message: 'missing remote cancel decision contract',
      },
      {
        regex: /\bpub fn resolve_remote_cancel_decision\b/,
        message: 'missing remote cancel decision resolver',
      },
      {
        regex: /\bpub trait RemoteDialogRuntimeHost\b/,
        message: 'missing remote dialog runtime host port',
      },
      {
        regex: /\bpub async fn submit_remote_dialog\b/,
        message: 'missing remote dialog orchestration owner',
      },
      {
        regex: /\bpub const REMOTE_FILE_MAX_READ_BYTES\b/,
        message: 'missing remote file max-read policy',
      },
      {
        regex: /\bpub const REMOTE_FILE_MAX_CHUNK_BYTES\b/,
        message: 'missing remote file chunk policy',
      },
      {
        regex: /\bpub fn resolve_remote_file_chunk_range\b/,
        message: 'missing remote file chunk range helper',
      },
      {
        regex: /\bpub fn remote_file_display_name\b/,
        message: 'missing remote file display-name fallback',
      },
      {
        regex: /\bpub fn resolve_remote_workspace_path\b/,
        message: 'missing remote workspace path resolver',
      },
      {
        regex: /\bpub fn detect_remote_mime_type\b/,
        message: 'missing remote MIME detector',
      },
      {
        regex: /\bpub async fn read_remote_workspace_file\b/,
        message: 'missing remote workspace full-file reader',
      },
      {
        regex: /\bpub async fn read_remote_workspace_file_chunk\b/,
        message: 'missing remote workspace chunk reader',
      },
      {
        regex: /\bpub async fn read_remote_workspace_file_info\b/,
        message: 'missing remote workspace file-info reader',
      },
      {
        regex: /\bpub struct RemoteDefaultModelsConfig\b/,
        message: 'missing remote model default DTO',
      },
      {
        regex: /\bpub struct RemoteModelConfig\b/,
        message: 'missing remote model DTO',
      },
      {
        regex: /\bpub struct RemoteModelCatalog\b/,
        message: 'missing remote model catalog DTO',
      },
      {
        regex: /\bpub struct RemoteModelCatalogPollDelta\b/,
        message: 'missing remote model catalog poll delta',
      },
      {
        regex: /\bpub enum RemoteCommand\b/,
        message: 'missing remote command wire contract',
      },
      {
        regex: /\bpub enum RemoteResponse\b/,
        message: 'missing remote response wire contract',
      },
      {
        regex: /\bpub fn should_send_remote_model_catalog\b/,
        message: 'missing remote model catalog poll policy',
      },
      {
        regex: /\bpub fn remote_model_catalog_poll_delta\b/,
        message: 'missing remote model catalog poll delta helper',
      },
      {
        regex: /\bpub fn remote_no_change_poll_response\b/,
        message: 'missing remote no-change poll response helper',
      },
      {
        regex: /\bpub fn remote_snapshot_poll_response\b/,
        message: 'missing remote snapshot poll response helper',
      },
      {
        regex: /\bpub fn remote_persisted_poll_response\b/,
        message: 'missing remote persisted poll response helper',
      },
    ],
  },
  {
    path: 'src/crates/services-integrations/tests/remote_connect_contracts.rs',
    reason: 'remote-connect owner crate must keep focused behavior contracts',
    patterns: [
      {
        regex: /\bremote_connect_command_wire_shape_lives_in_owner_contract\b/,
        message: 'missing remote command wire contract test',
      },
      {
        regex: /\bremote_connect_response_wire_shape_lives_in_owner_contract\b/,
        message: 'missing remote response wire contract test',
      },
      {
        regex: /\bremote_connect_model_catalog_delta_preserves_poll_invalidation_policy\b/,
        message: 'missing remote model catalog delta contract test',
      },
      {
        regex: /\bremote_connect_poll_helpers_preserve_delta_and_completion_policy\b/,
        message: 'missing remote poll helper contract test',
      },
      {
        regex: /\bremote_connect_image_context_policy_preserves_legacy_fallback_shape\b/,
        message: 'missing legacy image context fallback test',
      },
      {
        regex: /\bremote_connect_image_context_policy_prefers_explicit_contexts\b/,
        message: 'missing explicit image context preference test',
      },
      {
        regex: /\bremote_connect_image_context_adapter_owns_portable_conversion_shape\b/,
        message: 'missing image context adapter contract test',
      },
      {
        regex: /\bremote_connect_cancel_and_restore_policy_preserve_runtime_decisions\b/,
        message: 'missing cancel/restore policy test',
      },
      {
        regex: /\bremote_connect_dialog_runtime_owns_restore_prewarm_and_submit_order\b/,
        message: 'missing dialog runtime order test',
      },
      {
        regex: /\bremote_connect_dialog_runtime_preserves_explicit_turn_without_restore\b/,
        message: 'missing dialog explicit-turn test',
      },
      {
        regex: /\bremote_connect_dialog_runtime_keeps_legacy_restore_failure_tolerance\b/,
        message: 'missing restore failure tolerance test',
      },
      {
        regex: /\bremote_connect_file_transfer_policy_preserves_limits_and_chunk_ranges\b/,
        message: 'missing remote file transfer policy test',
      },
      {
        regex: /\bremote_connect_file_transfer_policy_preserves_name_fallback\b/,
        message: 'missing remote file display-name test',
      },
      {
        regex: /\bremote_connect_file_path_resolution_stays_within_workspace_root\b/,
        message: 'missing remote file path resolution test',
      },
      {
        regex: /\bremote_connect_file_read_helpers_preserve_current_wire_inputs\b/,
        message: 'missing remote full-read helper test',
      },
      {
        regex: /\bremote_connect_file_chunk_and_info_helpers_preserve_response_facts\b/,
        message: 'missing remote chunk/info helper test',
      },
      {
        regex: /\bremote_connect_tracker_keeps_finished_turn_snapshot_until_persistence_finalizes\b/,
        message: 'missing tracker completion contract test',
      },
      {
        regex: /\bremote_connect_tracker_registry_owns_lifecycle_without_core_state\b/,
        message: 'missing tracker registry owner test',
      },
      {
        regex: /\bremote_connect_tracker_ignores_unrelated_direct_session_events\b/,
        message: 'missing tracker unrelated-event guard test',
      },
      {
        regex: /\bremote_connect_tool_preview_slimming_keeps_short_fields_and_drops_large_strings\b/,
        message: 'missing remote tool preview slimming test',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/remote_connect/remote_server.rs',
    reason:
      'core remote-connect server must remain a product runtime adapter around integrations-owned contracts',
    patterns: [
      {
        regex: /\bCoreServiceAgentRuntime\b/,
        message: 'missing core service/agent runtime owner routing',
      },
      {
        regex: /\bstruct CoreRemoteDialogRuntimeHost\b/,
        message: 'missing core remote dialog runtime adapter',
      },
      {
        regex: /\bimpl RemoteDialogRuntimeHost for CoreRemoteDialogRuntimeHost\b/,
        message: 'missing integrations dialog host adapter implementation',
      },
      {
        regex: /\bsubmit_remote_dialog\b/,
        message: 'missing remote dialog owner orchestration delegation',
      },
      {
        regex: /\bread_remote_workspace_file\b/,
        message: 'missing remote file full-read helper delegation',
      },
      {
        regex: /\bread_remote_workspace_file_chunk\b/,
        message: 'missing remote file chunk helper delegation',
      },
      {
        regex: /\bread_remote_workspace_file_info\b/,
        message: 'missing remote file info helper delegation',
      },
      {
        regex: /\bremote_image_context\b/,
        message: 'missing image context adapter contract delegation',
      },
      {
        regex: /\bcore_service_agent_runtime_owner_maps_remote_image_context\b/,
        message: 'missing core service/agent image-context owner regression',
      },
      {
        regex: /\bremote_execution_prefers_unified_image_contexts_over_legacy_images\b/,
        message: 'missing unified image context preference regression',
      },
      {
        regex: /\bremote_execution_falls_back_to_legacy_images_as_image_contexts\b/,
        message: 'missing legacy image context fallback regression',
      },
      {
        regex: /\bremote_cancel_decision_preserves_current_turn_boundaries\b/,
        message: 'missing remote cancel boundary regression',
      },
      {
        regex: /\bremote_restore_target_only_restores_cold_sessions_with_workspace_binding\b/,
        message: 'missing remote restore target regression',
      },
      {
        regex: /\bremote_command_snapshot_covers_execution_poll_and_cancel_surfaces\b/,
        message: 'missing remote command snapshot regression',
      },
      {
        regex: /\bremote_response_snapshot_preserves_active_turn_and_result_shapes\b/,
        message: 'missing remote response snapshot regression',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/remote_connect/bot/command_router.rs',
    reason:
      'remote-connect bot must route concrete agent runtime port bindings through the core service/agent runtime owner',
    patterns: [
      {
        regex: /\bCoreServiceAgentRuntime\b/,
        message: 'missing core service/agent runtime owner routing',
      },
      {
        regex: /\bagent_submission_port\b/,
        message: 'missing agent submission port owner binding',
      },
      {
        regex: /\bbuild_remote_session_create_request\b/,
        message: 'missing integrations-owned remote session create request builder',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/coordination/scheduler.rs',
    reason:
      'core scheduler keeps remote queue policy semantics until agent-runtime migration is reviewed',
    patterns: [
      {
        regex: /\bremote_queue_policy_preserves_interactive_preempt_and_confirmation_boundary\b/,
        message: 'missing remote queue policy regression',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/registry.rs',
    reason:
      'core registry must stay a compatibility container that delegates product tool runtime assembly through the core owner module',
    patterns: [
      {
        regex: /\bfrom_inner\b/,
        message: 'missing generic agent-tools registry adapter hook',
      },
      {
        regex: /\bProductToolDecoratorRef\b/,
        message: 'missing product decorator ref alias using agent-tools contract',
      },
      {
        regex: /\bProductToolRuntime\b/,
        message: 'missing product tool runtime owner delegation',
      },
      {
        regex: /\bget_collapsed_tool_names\b/,
        message: 'missing collapsed-tool catalog owner',
      },
      {
        regex: /\bresolve_product_readonly_enabled_tools\b/,
        message: 'missing product tool catalog readonly facade delegation',
      },
      {
        regex: /\bproduct_tool_runtime_owner_preserves_registry_contract\b/,
        message: 'missing collapsed-tool manifest migration baseline',
      },
      {
        regex: /\binner\.is_tool_collapsed\b/,
        message: 'missing collapsed exposure lookup delegation',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/product_runtime.rs',
    reason:
      'core product tool runtime owner keeps registry assembly, static tool materialization, catalog manifests, and GetToolSpec facades explicit until concrete tools migrate',
    patterns: [
      {
        regex: /\bProductToolRuntime\b/,
        message: 'missing core product tool runtime owner',
      },
      {
        regex: /\bSnapshotToolDecorator\b/,
        message: 'missing generic snapshot decorator injection',
      },
      {
        regex: /\bProductSnapshotToolWrapper\b/,
        message: 'missing core product snapshot wrapper adapter',
      },
      {
        regex: /\bbuiltin_static_tool_providers\b/,
        message: 'missing builtin provider assembly input',
      },
      {
        regex: /\bStaticToolProviderGroup\b/,
        message: 'missing generic static provider group contract use',
      },
      {
        regex: /\bproduct_tool_provider_group_plan\b/,
        message: 'missing tool-pack provider group plan delegation',
      },
      {
        regex: /\bmaterialize_tool\b/,
        message: 'missing core concrete tool materialization boundary',
      },
      {
        regex: /\bGetToolSpecTool::new\(\)/,
        message: 'missing GetToolSpec registration anchor',
      },
      {
        regex: /\bToolRuntimeAssembly\b/,
        message: 'missing generic agent-tools runtime assembly delegation',
      },
      {
        regex: /\bcreate_registry_from_static_providers\b/,
        message: 'missing generic static provider assembly delegation',
      },
      {
        regex: /\bwrap_tool_for_snapshot_tracking\b/,
        message: 'missing snapshot wrapper boundary',
      },
      {
        regex: /\bProductToolCatalogProvider\b/,
        message: 'missing core product tool catalog provider owner',
      },
      {
        regex: /\bimpl ToolCatalogSnapshotProvider<dyn Tool> for ProductToolCatalogProvider\b/,
        message: 'missing core tool catalog snapshot provider implementation',
      },
      {
        regex: /\bimpl GetToolSpecCatalogProvider<dyn Tool, ToolUseContext> for ProductToolCatalogProvider\b/,
        message: 'missing core product GetToolSpec catalog provider implementation',
      },
      {
        regex: /\bget_global_tool_registry\b/,
        message: 'missing core product registry snapshot access',
      },
      {
        regex: /\bget_agent_registry\b/,
        message: 'missing core agent policy source for contextual catalog',
      },
      {
        regex: /\bToolCatalogRuntime\b/,
        message: 'missing agent-tools product catalog runtime facade delegation',
      },
      {
        regex: /\bproduct_tool_catalog_runtime\b/,
        message: 'missing product catalog runtime factory',
      },
      {
        regex: /\bGetToolSpecRuntime\b/,
        message: 'missing agent-tools GetToolSpec runtime facade delegation',
      },
      {
        regex: /\bproduct_get_tool_spec_runtime\b/,
        message: 'missing product GetToolSpec runtime factory',
      },
      {
        regex: /\bresolve_product_tool_manifest\b/,
        message: 'missing product manifest facade',
      },
      {
        regex: /\bresolve_product_readonly_enabled_tools\b/,
        message: 'missing product readonly enabled tools facade',
      },
      {
        regex: /\bresolve_product_get_tool_spec_results\b/,
        message: 'missing product GetToolSpec Tool-result vector facade',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'missing core-owned collapsed-tool unlock state source',
      },
      {
        regex: /\bproduct_catalog_provider_default_get_tool_spec_catalog_matches_registry\b/,
        message: 'missing product catalog provider collapsed catalog regression',
      },
      {
        regex: /\bproduct_tool_runtime_owner_preserves_registry_contract\b/,
        message: 'missing product runtime owner registry equivalence regression',
      },
      {
        regex: /\bGetToolSpec requires agent type context\b/,
        message: 'missing contextual GetToolSpec catalog validation boundary',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/tool_adapter.rs',
    reason:
      'core must keep the product Tool-to-agent-tools adapters explicit until ToolUseContext and concrete tools migrate',
    patterns: [
      {
        regex: /\bimpl ToolRegistryItem for dyn Tool\b/,
        message: 'missing core Tool registry adapter',
      },
      {
        regex: /\bimpl ContextualToolManifestItem<ToolUseContext> for dyn Tool\b/,
        message: 'missing core ToolUseContext contextual manifest adapter',
      },
      {
        regex: /\bTool::dynamic_tool_info\b/,
        message: 'missing dynamic tool metadata adapter',
      },
      {
        regex: /\bTool::is_readonly\b/,
        message: 'missing readonly metadata adapter',
      },
      {
        regex: /\bTool::is_enabled\b/,
        message: 'missing enabled-state metadata adapter',
      },
      {
        regex: /\bTool::description_with_context\b/,
        message: 'missing context-aware tool description adapter',
      },
      {
        regex: /\bTool::input_schema_for_model_with_context\b/,
        message: 'missing context-aware tool schema adapter',
      },
    ],
  },
  {
    path: 'src/crates/agent-tools/src/framework.rs',
    reason: 'agent-tools owns portable tool facts plus generic registry and provider contracts',
    patterns: [
      {
        regex: /\bpub struct ToolContextFacts\b/,
        message: 'missing portable tool context facts contract',
      },
      {
        regex: /\bpub trait PortableToolContextProvider\b/,
        message: 'missing portable tool context provider contract',
      },
      {
        regex: /\bpub enum ToolWorkspaceKind\b/,
        message: 'missing portable workspace kind contract',
      },
      {
        regex: /\bpub trait StaticToolProvider\b/,
        message: 'missing static tool provider contract',
      },
      {
        regex: /\bpub struct ToolRuntimeAssembly\b/,
        message: 'missing generic runtime assembly contract',
      },
      {
        regex: /\bpub type ToolDecoratorRef\b/,
        message: 'missing generic decorator ref contract',
      },
      {
        regex: /\bpub trait SnapshotToolWrapper\b/,
        message: 'missing generic snapshot wrapper contract',
      },
      {
        regex: /\bpub struct SnapshotToolDecorator\b/,
        message: 'missing generic snapshot decorator contract',
      },
      {
        regex: /\bcreate_registry_from_static_providers\b/,
        message: 'missing generic static-provider assembly helper',
      },
      {
        regex: /\bpub fn install_static_provider\b/,
        message: 'missing static provider registry installer',
      },
      {
        regex: /\bpub fn build_get_tool_spec_duplicate_load_result\b/,
        message: 'missing provider-neutral GetToolSpec duplicate-load result helper',
      },
      {
        regex: /\bpub fn build_get_tool_spec_detail_result\b/,
        message: 'missing provider-neutral GetToolSpec detail result helper',
      },
      {
        regex: /\bpub fn resolve_get_tool_spec_execution_plan\b/,
        message: 'missing provider-neutral GetToolSpec execution plan helper',
      },
      {
        regex: /\bpub async fn resolve_get_tool_spec_execution_result_from_provider\b/,
        message: 'missing provider-backed GetToolSpec execution result helper',
      },
      {
        regex: /\bpub struct GetToolSpecRuntime\b/,
        message: 'missing provider-backed GetToolSpec runtime facade',
      },
    ],
  },
  {
    path: 'src/crates/tool-packs/src/lib.rs',
    reason:
      'tool-packs must keep its feature-group scaffold explicit without owning concrete tools yet',
    patterns: [
      {
        regex: /\bpub enum ToolPackFeatureGroup\b/,
        message: 'missing tool-pack feature group scaffold',
      },
      {
        regex: /\bpub fn all_feature_groups\b/,
        message: 'missing tool-pack full feature group metadata helper',
      },
      {
        regex: /\bpub fn enabled_feature_groups\b/,
        message: 'missing tool-pack compile-time feature metadata helper',
      },
      {
        regex: /\bpub struct ToolProviderGroupPlan\b/,
        message: 'missing tool-pack provider group plan contract',
      },
      {
        regex: /\bpub fn product_tool_provider_group_plan\b/,
        message: 'missing product tool provider group plan',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/manifest_resolver.rs',
    reason:
      'core must continue owning manifest resolver wrappers while delegating product catalog access and generic manifest assembly',
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
        regex: /\bresolve_product_visible_tools\b/,
        message: 'missing core product visible-tools facade delegation',
      },
      {
        regex: /\bresolve_product_tool_manifest\b/,
        message: 'missing core product manifest facade delegation',
      },
      {
        regex: /\bcollapsed_tool_names\b/,
        message: 'missing collapsed-tool name tracking',
      },
      {
        regex: /\bmanifest_preserves_explicit_get_tool_spec_runtime_contract\b/,
        message: 'missing core GetToolSpec manifest insertion regression',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/implementations/get_tool_spec_tool.rs',
    reason:
      'core must continue owning the GetToolSpec Tool adapter and product boundary while delegating generic runtime surface to agent-tools',
    patterns: [
      {
        regex: /\bpub struct GetToolSpecTool\b/,
        message: 'missing GetToolSpec owner type',
      },
      {
        regex: /\bresolve_product_get_tool_spec_results\b/,
        message: 'missing product GetToolSpec Tool-result vector facade delegation',
      },
      {
        regex: /\bmap_get_tool_spec_execution_error\b/,
        message: 'missing core GetToolSpec execution error mapping boundary',
      },
      {
        regex: /\bbuild_product_get_tool_spec_catalog_description\b/,
        message: 'missing core product GetToolSpec catalog facade delegation',
      },
      {
        regex: /\bproduct_get_tool_spec_runtime\b/,
        message: 'missing product GetToolSpec runtime facade delegation',
      },
      {
        regex: /\bwith_runtime\b/,
        message: 'missing core GetToolSpec static surface facade boundary',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/framework.rs',
    reason:
      'core must continue owning ToolUseContext while re-exporting pure exposure contracts until a portable context port is reviewed',
    patterns: [
      {
        regex: /\bToolExposure\b/,
        message: 'missing ToolExposure compatibility re-export',
      },
      {
        regex: /\bpub struct ToolUseContext\b/,
        message: 'missing ToolUseContext owner type',
      },
      {
        regex: /\bto_tool_context_facts\b/,
        message: 'missing portable ToolUseContext facts projection',
      },
      {
        regex: /\btool_context_facts_omit_runtime_owner_fields_even_when_context_is_populated\b/,
        message: 'missing portable facts runtime-owner leak guard',
      },
      {
        regex: /customData/,
        message: 'missing custom data runtime-only facts guard',
      },
      {
        regex: /cancellationToken/,
        message: 'missing cancellation token runtime-only facts guard',
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
        regex: /\bpipeline_preserves_core_owned_tool_context_without_portable_runtime_leak\b/,
        message: 'missing ToolUseContext runtime boundary regression',
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
        regex: /\bcollect_unlocked_collapsed_tools_dedupes_and_filters_runtime_unlocks\b/,
        message: 'missing GetToolSpec unlock filtering regression',
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
    path: 'src/crates/core/src/agentic/agents/definitions/modes/mod.rs',
    reason:
      'core agent mode definitions must continue exposing Multitask mode until an approved agent-runtime migration preserves mode registration semantics',
    patterns: [
      {
        regex: /\bmod multitask\b/,
        message: 'missing Multitask mode module',
      },
      {
        regex: /\bpub use multitask::MultitaskMode\b/,
        message: 'missing Multitask mode export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/agents/definitions/subagents/mod.rs',
    reason:
      'core subagent definitions must continue exposing the built-in GeneralPurpose subagent until registry ownership migration has equivalence coverage',
    patterns: [
      {
        regex: /\bmod general_purpose\b/,
        message: 'missing GeneralPurpose subagent module',
      },
      {
        regex: /\bpub use general_purpose::GeneralPurposeAgent\b/,
        message: 'missing GeneralPurpose subagent export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/agents/registry/builtin.rs',
    reason:
      'core builtin registry must continue registering latest-main mode and subagent defaults until agent registry ownership migrates with API equivalence tests',
    patterns: [
      {
        regex: /\bbuiltin_agent_specs\(\)/,
        message: 'missing builtin agent spec registration source',
      },
      {
        regex: /"Multitask"\s*=>\s*"auto"/,
        message: 'missing Multitask default model mapping',
      },
      {
        regex: /"GeneralPurpose"\s*=>\s*"fast"/,
        message: 'missing GeneralPurpose default model mapping',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/implementations/task_tool.rs',
    reason:
      'core Task tool must continue owning background subagent launch semantics until a reviewed agent-runtime port preserves delivery behavior',
    patterns: [
      {
        regex: /"run_in_background"/,
        message: 'missing Task run_in_background schema flag',
      },
      {
        regex: /\bstart_background_subagent\b/,
        message: 'missing background subagent launch path',
      },
      {
        regex: /\bbackground_task_id\b/,
        message: 'missing background task id result contract',
      },
      {
        regex: /Background subagent/,
        message: 'missing assistant-visible background subagent acknowledgement',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/coordination/scheduler.rs',
    reason:
      'core scheduler must continue owning background subagent result delivery until running-turn and idle-session routing equivalence tests exist',
    patterns: [
      {
        regex: /\bdeliver_background_subagent_result\b/,
        message: 'missing background subagent delivery entry point',
      },
      {
        regex: /RoundInjectionKind::BackgroundSubagentResult/,
        message: 'missing running-turn background result injection',
      },
      {
        regex: /RoundInjectionTarget::CurrentRunningTurn/,
        message: 'missing current-turn injection target',
      },
      {
        regex: /DialogTriggerSource::AgentSession/,
        message: 'missing idle-session agent-session follow-up turn source',
      },
    ],
  },
  {
    path: 'src/apps/cli/src/ui/startup.rs',
    reason:
      'CLI mode-aware subagent management remains an app-layer product surface until agent registry migration has CLI equivalence coverage',
    patterns: [
      {
        regex: /\bfn show_available_subagent_list\b/,
        message: 'missing CLI subagent list surface',
      },
      {
        regex: /\bfn show_subagent_config_selector\b/,
        message: 'missing CLI subagent config surface',
      },
      {
        regex: /\bget_subagents_for_query\b/,
        message: 'missing CLI mode-scoped subagent query',
      },
      {
        regex: /\bSubagentQueryContext\b/,
        message: 'missing CLI subagent query context',
      },
      {
        regex: /\bupdate_subagent_override\b/,
        message: 'missing CLI subagent availability update path',
      },
    ],
  },
  {
    path: 'src/apps/cli/src/ui/subagent_selector.rs',
    reason:
      'CLI subagent selector presentation must remain app-layer UI while registry availability semantics stay in core',
    patterns: [
      {
        regex: /\bpub enum SubagentSelectorAction\b/,
        message: 'missing CLI subagent selector action contract',
      },
      {
        regex: /\bpub fn show_list\b/,
        message: 'missing CLI subagent list mode',
      },
      {
        regex: /\bpub fn show_config\b/,
        message: 'missing CLI subagent config mode',
      },
      {
        regex: /\bdefault_enabled\b/,
        message: 'missing CLI default availability display',
      },
      {
        regex: /\bfn render_subagent_line\b/,
        message: 'missing CLI subagent presentation renderer',
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
    path: 'src/web-ui/src/main.tsx',
    reason:
      'web startup scheduling and trace orchestration remain web product-surface behavior, not core contract runtime',
    patterns: [
      {
        regex: /\bstartupTrace\b/,
        message: 'missing web startup trace surface',
      },
      {
        regex: /\bbackgroundTaskScheduler\b/,
        message: 'missing deferred startup scheduler surface',
      },
      {
        regex: /\binitializeAllTools\b/,
        message: 'missing narrow tool-startup entry integration',
      },
      {
        regex: /\bafter_render_start\b/,
        message: 'missing post-render startup phase',
      },
    ],
  },
  {
    path: 'src/web-ui/src/shared/utils/startupTrace.ts',
    reason:
      'web startup trace classification and redaction remain web infrastructure behavior until a telemetry contract is reviewed',
    patterns: [
      {
        regex: /\bfunction sanitizeTraceData\b/,
        message: 'missing startup trace sanitization',
      },
      {
        regex: /\bexport function isRemoteTraceRequest\b/,
        message: 'missing remote request classifier',
      },
      {
        regex: /\brecordApiCall\b/,
        message: 'missing startup API-call trace recorder',
      },
      {
        regex: /\bflushSummary\b/,
        message: 'missing bounded startup summary flush',
      },
      {
        regex: /\bmarkPhaseAfterAnimationFrames\b/,
        message: 'missing frame-delayed startup marker',
      },
    ],
  },
  {
    path: 'src/web-ui/src/shared/utils/backgroundTaskScheduler.ts',
    reason:
      'web background startup scheduling remains web infrastructure behavior and must preserve dedupe/cancel semantics',
    patterns: [
      {
        regex: /\bexport class BackgroundTaskScheduler\b/,
        message: 'missing background task scheduler',
      },
      {
        regex: /\binFlightKey\b/,
        message: 'missing in-flight dedupe key',
      },
      {
        regex: /\bAbortController\b/,
        message: 'missing cancellation controller',
      },
      {
        regex: /\bBackgroundTaskCancelledError\b/,
        message: 'missing cancellation error contract',
      },
      {
        regex: /\bcancelIdle\b/,
        message: 'missing idle callback cancellation',
      },
    ],
  },
  {
    path: 'src/web-ui/src/tools/initializeTools.ts',
    reason:
      'web tool startup must stay behind a narrow app-layer entry instead of importing product tools through shared contracts',
    patterns: [
      {
        regex: /\bexport async function initializeAllTools\b/,
        message: 'missing narrow tool startup entry',
      },
      {
        regex: /\binitializeLsp\b/,
        message: 'missing LSP startup initializer call',
      },
      {
        regex: /\binitializeGit\b/,
        message: 'missing Git startup initializer call',
      },
      {
        regex: /does not import every tool/,
        message: 'missing narrow startup import guard',
      },
    ],
  },
  {
    path: 'src/web-ui/src/tools/editor/services/MonacoStartupWarmup.ts',
    reason:
      'Monaco startup warmup remains a deferred web-app optimization, not a core runtime dependency',
    patterns: [
      {
        regex: /\bexport function scheduleMonacoStartupWarmup\b/,
        message: 'missing deferred Monaco warmup entry',
      },
      {
        regex: /\bbackgroundTaskScheduler\b/,
        message: 'missing background scheduler integration',
      },
      {
        regex: /startup:monaco-warmup/,
        message: 'missing Monaco warmup dedupe key',
      },
    ],
  },
  {
    path: 'src/web-ui/src/flow_chat/services/flow-chat-manager/SessionModule.ts',
    reason:
      'flow-chat history hydration remains web startup/product-surface behavior until a UI equivalence plan exists',
    patterns: [
      {
        regex: /\bhistorical_session_hydrate_request\b/,
        message: 'missing historical session hydrate trace',
      },
      {
        regex: /Load history in the background/,
        message: 'missing non-blocking history load contract',
      },
      {
        regex: /\bhistoryState: 'ready'/,
        message: 'missing history-ready state contract',
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
    path: 'src/crates/core/src/miniapp/builtin/mod.rs',
    reason:
      'core must continue owning built-in MiniApp asset includes, seeding IO, marker writes, and recompilation until builtin asset runtime migration is reviewed',
    patterns: [
      {
        regex: /id: "builtin-pr-review"/,
        message: 'missing built-in PR Review MiniApp anchor',
      },
      {
        regex: /\bBUILTIN_APPS\b/,
        message: 'missing built-in MiniApp asset include owner',
      },
      {
        regex: /\bbuiltin_content_hash\b/,
        message: 'missing product-domain built-in MiniApp content hash use',
      },
      {
        regex: /\bshould_seed_builtin_app\b/,
        message: 'missing product-domain built-in MiniApp seed decision use',
      },
      {
        regex: /\bresolve_builtin_seed_check\b/,
        message: 'missing product-domain built-in MiniApp seed check use',
      },
      {
        regex: /\bresolve_builtin_seed_action\b/,
        message: 'missing product-domain built-in MiniApp seed action use',
      },
      {
        regex: /\bbuiltin_source_files\b/,
        message: 'missing product-domain built-in MiniApp source payload use',
      },
      {
        regex: /\bBUILTIN_PLACEHOLDER_COMPILED_HTML\b/,
        message: 'missing product-domain built-in MiniApp placeholder payload use',
      },
      {
        regex: /\bread_builtin_install_marker\b/,
        message: 'missing core-owned built-in MiniApp marker read IO',
      },
      {
        regex: /\bparse_builtin_install_marker\b/,
        message: 'missing product-domain built-in MiniApp marker parse helper use',
      },
      {
        regex: /\bwrite_builtin_install_marker\b/,
        message: 'missing core-owned built-in MiniApp marker write IO',
      },
      {
        regex: /\bserialize_builtin_install_marker\b/,
        message: 'missing product-domain built-in MiniApp marker serialization helper use',
      },
      {
        regex: /\brecompile\b/,
        message: 'missing core-owned built-in MiniApp recompile orchestration',
      },
      {
        regex: /\bload_customization_metadata\b/,
        message: 'missing customized built-in preservation path',
      },
      {
        regex: /\bavailable_builtin_update\b/,
        message: 'missing customized built-in update metadata path',
      },
    ],
  },
  {
    path: 'src/crates/core/src/miniapp/host_dispatch.rs',
    reason:
      'core must continue owning MiniApp host-dispatch execution until host/runtime migration is reviewed',
    patterns: [
      {
        regex: /\bpub async fn dispatch_host\b/,
        message: 'missing MiniApp host dispatch entry',
      },
      {
        regex: /\basync fn dispatch_fs\b/,
        message: 'missing MiniApp fs host dispatch',
      },
      {
        regex: /\basync fn dispatch_shell\b/,
        message: 'missing MiniApp shell host dispatch',
      },
      {
        regex: /\bcommand_basename_allowed\b/,
        message: 'missing MiniApp shell allowlist policy use',
      },
      {
        regex: /\bhost_allowed_by_allowlist\b/,
        message: 'missing MiniApp net allowlist policy use',
      },
    ],
  },
  {
    path: 'src/crates/services-integrations/src/remote_ssh/paths.rs',
    reason:
      'services-integrations remote-ssh owns workspace path/session identity helpers that do not require concrete SSH runtime handles',
    patterns: [
      {
        regex: /\bpub fn remote_workspace_runtime_root\b/,
        message: 'missing remote workspace runtime root helper',
      },
      {
        regex: /\bpub fn remote_workspace_session_mirror_dir\b/,
        message: 'missing remote workspace session mirror helper',
      },
      {
        regex: /\bpub fn canonicalize_local_workspace_root\b/,
        message: 'missing local workspace canonicalization helper',
      },
      {
        regex: /\bpub fn normalize_local_workspace_root_for_stable_id\b/,
        message: 'missing local workspace stable-root helper',
      },
      {
        regex: /\bpub fn local_workspace_roots_equal\b/,
        message: 'missing local workspace equality helper',
      },
      {
        regex: /\bpub fn unresolved_remote_session_storage_dir\b/,
        message: 'missing unresolved remote session storage helper',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/miniapp/storage.rs',
    reason:
      'product-domains owns MiniApp storage shape contracts while core/adapters keep filesystem IO',
    patterns: [
      {
        regex: /\bpub struct MiniAppStorageLayout\b/,
        message: 'missing MiniApp storage layout contract',
      },
      {
        regex: /\bpub const META_JSON\b/,
        message: 'missing MiniApp metadata filename contract',
      },
      {
        regex: /\bpub fn source_file_path\b/,
        message: 'missing MiniApp source file layout helper',
      },
      {
        regex: /\bpub fn versions_dir\b/,
        message: 'missing MiniApp versions directory layout helper',
      },
      {
        regex: /\bpub const DRAFT_JSON\b/,
        message: 'missing MiniApp draft manifest filename contract',
      },
      {
        regex: /\bpub const REQUIRED_SOURCE_FILES\b/,
        message: 'missing MiniApp import source file list contract',
      },
      {
        regex: /\bpub const PLACEHOLDER_COMPILED_HTML\b/,
        message: 'missing MiniApp placeholder compiled HTML contract',
      },
      {
        regex: /\bpub struct MiniAppImportLayout\b/,
        message: 'missing MiniApp import layout contract',
      },
      {
        regex: /\bpub fn build_import_fallbacks\b/,
        message: 'missing MiniApp import fallback payload helper',
      },
      {
        regex: /\bpub fn draft_dir\b/,
        message: 'missing MiniApp draft directory layout helper',
      },
      {
        regex: /\bpub fn customization_path\b/,
        message: 'missing MiniApp customization metadata path helper',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/miniapp/lifecycle.rs',
    reason:
      'product-domains owns pure MiniApp lifecycle state transitions while core keeps compile, storage IO, and runtime execution',
    patterns: [
      {
        regex: /\bpub fn mark_deps_installed_state\b/,
        message: 'missing MiniApp deps-installed state helper',
      },
      {
        regex: /\bpub fn clear_worker_restart_required_state\b/,
        message: 'missing MiniApp worker-restart clear state helper',
      },
      {
        regex: /\bpub fn prepare_rollback_app\b/,
        message: 'missing MiniApp rollback state helper',
      },
      {
        regex: /\bpub fn apply_recompile_result\b/,
        message: 'missing MiniApp recompile result state helper',
      },
      {
        regex: /\bpub fn apply_sync_from_fs_result\b/,
        message: 'missing MiniApp sync-from-fs state helper',
      },
      {
        regex: /\bpub fn apply_import_runtime_state\b/,
        message: 'missing MiniApp import runtime state helper',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/miniapp/draft.rs',
    reason:
      'product-domains owns MiniApp draft DTO and response shape while core keeps draft filesystem IO',
    patterns: [
      {
        regex: /\bpub struct MiniAppDraftManifest\b/,
        message: 'missing MiniApp draft manifest DTO',
      },
      {
        regex: /\bpub struct MiniAppDraft\b/,
        message: 'missing MiniApp draft response DTO',
      },
      {
        regex: /\bpub fn build_draft_manifest\b/,
        message: 'missing MiniApp draft manifest helper',
      },
      {
        regex: /\bpub fn build_draft_response\b/,
        message: 'missing MiniApp draft response helper',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/miniapp/runtime.rs',
    reason:
      'product-domains owns MiniApp runtime search-plan contracts while core keeps executable lookup and version process execution',
    patterns: [
      {
        regex: /\bpub fn runtime_lookup_order\b/,
        message: 'missing MiniApp runtime lookup order contract',
      },
      {
        regex: /\bpub fn candidate_executable_path\b/,
        message: 'missing MiniApp runtime candidate executable helper',
      },
      {
        regex: /\bpub fn versioned_executable_candidate\b/,
        message: 'missing MiniApp version-manager executable helper',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/miniapp/host_routing.rs',
    reason:
      'product-domains owns MiniApp host-routing and allowlist string policy while core keeps host execution',
    patterns: [
      {
        regex: /\bpub fn command_basename_for_allowlist\b/,
        message: 'missing MiniApp command basename allowlist helper',
      },
      {
        regex: /\bpub fn command_basename_allowed\b/,
        message: 'missing MiniApp command allowlist policy helper',
      },
      {
        regex: /\bpub fn host_allowed_by_allowlist\b/,
        message: 'missing MiniApp host allowlist policy helper',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/miniapp/customization.rs',
    reason:
      'product-domains owns MiniApp customization metadata, built-in update policy, and permission-diff contracts while core keeps draft storage/runtime',
    patterns: [
      {
        regex: /\bpub struct MiniAppCustomizationMetadata\b/,
        message: 'missing MiniApp customization metadata contract',
      },
      {
        regex: /\bpub struct MiniAppDeclinedBuiltinUpdate\b/,
        message: 'missing MiniApp declined built-in update contract',
      },
      {
        regex: /\bpub struct MiniAppPermissionDiff\b/,
        message: 'missing MiniApp permission diff contract',
      },
      {
        regex: /\bpub fn diff_permissions\b/,
        message: 'missing MiniApp permission diff helper',
      },
      {
        regex: /\bpub fn apply_draft_customization_metadata\b/,
        message: 'missing MiniApp customization draft-apply helper',
      },
      {
        regex: /\bpub fn mark_builtin_update_available_metadata\b/,
        message: 'missing MiniApp built-in update availability helper',
      },
      {
        regex: /\bpub fn decline_builtin_update_metadata\b/,
        message: 'missing MiniApp built-in update decline helper',
      },
      {
        regex: /\bpub fn is_current_declined_builtin_update\b/,
        message: 'missing MiniApp declined update current-state helper',
      },
    ],
  },
  {
    path: 'src/crates/core/src/miniapp/manager.rs',
    reason:
      'core MiniApp manager must use product-domain policy/facade helpers while retaining compile, storage IO, and built-in source-hash lookup',
    patterns: [
      {
        regex: /\bapply_draft_customization_metadata\b/,
        message: 'missing product-domain draft customization helper use',
      },
      {
        regex: /\bmark_builtin_update_available_metadata\b/,
        message: 'missing product-domain built-in update availability helper use',
      },
      {
        regex: /\bdecline_builtin_update_metadata\b/,
        message: 'missing product-domain built-in update decline helper use',
      },
      {
        regex: /\bMiniAppRuntimeFacade\b/,
        message: 'missing product-domain MiniApp runtime-state facade use',
      },
      {
        regex: /\bCoreProductDomainRuntime\b/,
        message: 'missing core-owned product-domain runtime owner delegation',
      },
      {
        regex: /\bpersist_sync_from_fs_result_for_app\b/,
        message: 'missing product-domain MiniApp sync-from-fs facade delegation',
      },
      {
        regex: /\bcompile_source\b/,
        message: 'missing core-owned MiniApp compile orchestration',
      },
      {
        regex: /\bREQUIRED_SOURCE_FILES\b/,
        message: 'missing product-domain MiniApp import file-shape contract use',
      },
      {
        regex: /\bMiniAppImportLayout\b/,
        message: 'missing product-domain MiniApp import layout helper use',
      },
      {
        regex: /\bbuild_import_fallbacks\b/,
        message: 'missing product-domain MiniApp import fallback helper use',
      },
      {
        regex: /\bapply_import_runtime_state\b/,
        message: 'missing product-domain MiniApp import runtime state helper use',
      },
      {
        regex: /\bstorage\.load_customization_metadata\b/,
        message: 'missing core-owned customization metadata storage IO',
      },
      {
        regex: /\bruntime_preflight_preserves_recompile_sync_rollback_and_deps_state\b/,
        message: 'missing MiniApp manager runtime preflight regression test',
      },
      {
        regex: /\bimport_from_path_preserves_fallback_files_recompile_and_runtime_state\b/,
        message: 'missing MiniApp import runtime preflight regression test',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/miniapp/ports.rs',
    reason:
      'product-domains owns MiniApp runtime-state port facade while core keeps concrete storage IO, compile, worker, and host execution',
    patterns: [
      {
        regex: /\bpub struct MiniAppRuntimeFacade\b/,
        message: 'missing MiniApp runtime-state facade',
      },
      {
        regex: /\bmark_deps_installed_state\b/,
        message: 'missing MiniApp deps-installed state transition in facade',
      },
      {
        regex: /\bpersist_sync_from_fs_result_for_app\b/,
        message: 'missing MiniApp sync-from-fs preloaded snapshot facade path',
      },
    ],
  },
  {
    path: 'src/crates/core/src/function_agents/git-func-agent/ai_service.rs',
    reason:
      'core must continue owning Git function-agent AI client calls while product-domains owns prompt and response policy',
    patterns: [
      {
        regex: /\bprepare_commit_ai_prompt\b/,
        message: 'missing product-domain Git function-agent prompt policy use',
      },
      {
        regex: /\bparse_commit_ai_response\b/,
        message: 'missing product-domain Git function-agent response policy use',
      },
      {
        regex: /\bai_client\s*\.\s*send_message\b/,
        message: 'missing core-owned function-agent AI call',
      },
      {
        regex: /\bAgentError::internal_error\b/,
        message: 'missing core-owned function-agent AI transport error mapping',
      },
      {
        regex: /\bparse_commit_response_preserves_product_domain_response_policy\b/,
        message: 'missing Git function-agent AI response boundary regression test',
      },
    ],
  },
  {
    path: 'src/crates/core/src/function_agents/startchat-func-agent/ai_service.rs',
    reason:
      'core must continue owning Startchat AI client calls while product-domains owns prompt and response policy',
    patterns: [
      {
        regex: /\bbuild_work_state_analysis_prompt\b/,
        message: 'missing product-domain Startchat prompt policy use',
      },
      {
        regex: /\bparse_work_state_analysis_response\b/,
        message: 'missing product-domain Startchat response policy use',
      },
      {
        regex: /\bai_client\s*\.\s*send_message\b/,
        message: 'missing core-owned Startchat AI call',
      },
      {
        regex: /\bAgentError::internal_error\b/,
        message: 'missing core-owned Startchat AI transport error mapping',
      },
      {
        regex: /\bparse_complete_analysis_preserves_product_domain_response_policy\b/,
        message: 'missing Startchat AI response boundary regression test',
      },
    ],
  },
  {
    path: 'src/crates/core/src/function_agents/git-func-agent/commit_generator.rs',
    reason:
      'Git function-agent commit generation must route through the core product-domain runtime owner while core keeps concrete adapters',
    patterns: [
      {
        regex: /\bCoreProductDomainRuntime\b/,
        message: 'missing core product-domain runtime owner routing',
      },
      {
        regex: /\bfunction_agent_git_adapter\b/,
        message: 'missing core-owned Git adapter factory wiring',
      },
      {
        regex: /\bfunction_agent_ai_adapter\b/,
        message: 'missing core-owned AI adapter factory wiring',
      },
      {
        regex: /\bfunction_agent_runtime_facade\b/,
        message: 'missing product-domain function-agent runtime facade owner routing',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/miniapp/builtin.rs',
    reason:
      'product-domains owns pure built-in MiniApp bundle, marker, hash, and seed-decision contracts while core keeps asset seeding IO and recompilation',
    patterns: [
      {
        regex: /\bpub struct BuiltinMiniAppBundle\b/,
        message: 'missing built-in MiniApp bundle contract',
      },
      {
        regex: /\bpub struct BuiltinInstallMarker\b/,
        message: 'missing built-in MiniApp install marker contract',
      },
      {
        regex: /\bpub const BUILTIN_INSTALL_MARKER\b/,
        message: 'missing built-in MiniApp marker filename contract',
      },
      {
        regex: /\bpub fn builtin_content_hash\b/,
        message: 'missing built-in MiniApp content hash helper',
      },
      {
        regex: /\bpub fn should_seed_builtin_app\b/,
        message: 'missing built-in MiniApp seed decision helper',
      },
      {
        regex: /\bpub struct BuiltinSeedArtifacts\b/,
        message: 'missing built-in MiniApp seed artifacts contract',
      },
      {
        regex: /\bpub enum BuiltinSeedCheck\b/,
        message: 'missing built-in MiniApp seed check contract',
      },
      {
        regex: /\bpub enum BuiltinSeedAction\b/,
        message: 'missing built-in MiniApp seed action contract',
      },
      {
        regex: /\bpub fn resolve_builtin_seed_check\b/,
        message: 'missing built-in MiniApp seed check helper',
      },
      {
        regex: /\bpub fn resolve_builtin_seed_action\b/,
        message: 'missing built-in MiniApp seed action helper',
      },
      {
        regex: /\bpub fn serialize_builtin_install_marker\b/,
        message: 'missing built-in MiniApp marker serialization helper',
      },
      {
        regex: /\bpub fn parse_builtin_install_marker\b/,
        message: 'missing built-in MiniApp marker parse helper',
      },
      {
        regex: /\bpub fn builtin_source_files\b/,
        message: 'missing built-in MiniApp source payload helper',
      },
      {
        regex: /\bpub const BUILTIN_PLACEHOLDER_COMPILED_HTML\b/,
        message: 'missing built-in MiniApp placeholder payload contract',
      },
      {
        regex: /\bpub fn build_builtin_package_json\b/,
        message: 'missing built-in MiniApp package payload helper',
      },
    ],
  },
  {
    path: 'src/crates/core/src/function_agents/startchat-func-agent/work_state_analyzer.rs',
    reason:
      'Startchat work-state analysis must route through the core product-domain runtime owner while core keeps concrete adapters',
    patterns: [
      {
        regex: /\bCoreProductDomainRuntime\b/,
        message: 'missing core product-domain runtime owner routing',
      },
      {
        regex: /\bfunction_agent_git_adapter\b/,
        message: 'missing core-owned Git adapter factory wiring',
      },
      {
        regex: /\bfunction_agent_ai_adapter\b/,
        message: 'missing core-owned AI adapter factory wiring',
      },
      {
        regex: /\bfunction_agent_runtime_facade\b/,
        message: 'missing product-domain function-agent runtime facade owner routing',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/function_agents/ports.rs',
    reason:
      'product-domains owns port-backed function-agent facade orchestration while core keeps concrete Git/AI runtime calls',
    patterns: [
      {
        regex: /\bpub struct FunctionAgentRuntimeFacade\b/,
        message: 'missing function-agent runtime facade',
      },
      {
        regex: /\bgenerate_commit_message\b/,
        message: 'missing function-agent commit facade orchestration',
      },
      {
        regex: /\banalyze_work_state\b/,
        message: 'missing function-agent work-state facade orchestration',
      },
      {
        regex: /\bgit_work_state_from_snapshot\b/,
        message: 'missing Startchat Git snapshot projection helper',
      },
      {
        regex: /\bStartchatTimeSnapshot\b/,
        message: 'missing Startchat time snapshot contract',
      },
      {
        regex: /\bstartchat_time_snapshot\b/,
        message: 'missing Startchat time snapshot port',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/function_agents/common.rs',
    reason:
      'product-domains owns function-agent AI response JSON extraction while core keeps concrete AI clients',
    patterns: [
      {
        regex: /\bfn extract_json_from_ai_response\b/,
        message: 'missing function-agent AI response JSON extraction helper',
      },
      {
        regex: /\bfn try_repair_json\b/,
        message: 'missing function-agent AI response JSON repair helper',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/function_agents/startchat_func_agent/utils.rs',
    reason:
      'product-domains owns Startchat function-agent prompt and response policy while core keeps AI calls',
    patterns: [
      {
        regex: /\bpub const WORK_STATE_ANALYSIS_PROMPT\b/,
        message: 'missing product-domain Startchat prompt template',
      },
      {
        regex: /\bpub fn build_work_state_analysis_prompt\b/,
        message: 'missing product-domain Startchat prompt builder',
      },
      {
        regex: /\bpub struct ParsedCompleteAnalysis\b/,
        message: 'missing Startchat complete-analysis parse result contract',
      },
      {
        regex: /\bpub fn parse_complete_analysis_value\b/,
        message: 'missing Startchat complete-analysis value parser',
      },
      {
        regex: /\bpub fn parse_complete_analysis_json\b/,
        message: 'missing Startchat complete-analysis JSON parser',
      },
      {
        regex: /\bpub fn parse_work_state_analysis_response\b/,
        message: 'missing Startchat AI response policy',
      },
      {
        regex: /\bwork_state_ai_response_policy_extracts_json_and_maps_domain_errors\b/,
        message: 'missing Startchat AI response policy regression test',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/function_agents/git_func_agent/utils.rs',
    reason:
      'product-domains owns Git function-agent prompt and response policy while core keeps AI calls',
    patterns: [
      {
        regex: /\bpub const COMMIT_MESSAGE_PROMPT\b/,
        message: 'missing product-domain Git function-agent prompt template',
      },
      {
        regex: /\bpub fn parse_commit_analysis_value\b/,
        message: 'missing Git function-agent commit analysis value parser',
      },
      {
        regex: /\bpub fn parse_commit_analysis_json\b/,
        message: 'missing Git function-agent commit analysis JSON parser',
      },
      {
        regex: /\bpub fn truncate_diff_for_commit_prompt\b/,
        message: 'missing Git function-agent diff truncation helper',
      },
      {
        regex: /\bpub fn prepare_commit_prompt\b/,
        message: 'missing Git function-agent prompt preparation helper',
      },
      {
        regex: /\bpub fn prepare_commit_ai_prompt\b/,
        message: 'missing Git function-agent AI prompt policy',
      },
      {
        regex: /\bpub fn parse_commit_ai_response\b/,
        message: 'missing Git function-agent AI response policy',
      },
      {
        regex: /\bcommit_ai_response_policy_extracts_json_and_maps_domain_errors\b/,
        message: 'missing Git function-agent AI response policy regression test',
      },
    ],
  },
  {
    path: 'src/crates/core/src/miniapp/runtime_detect.rs',
    reason:
      'core MiniApp runtime detection must use product-domain search-plan helpers while retaining process-backed executable/version checks',
    patterns: [
      {
        regex: /\bruntime_lookup_order\b/,
        message: 'missing product-domain runtime lookup order use',
      },
      {
        regex: /\bcandidate_executable_path\b/,
        message: 'missing product-domain candidate executable helper use',
      },
      {
        regex: /\bversioned_executable_candidate\b/,
        message: 'missing product-domain version-manager executable helper use',
      },
      {
        regex: /\bget_version\b/,
        message: 'missing core-owned version process execution',
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
      'core must continue owning function-agent Git/AI runtime adapters until Git/AI service migration is reviewed',
    patterns: [
      {
        regex: /\bpub struct CoreFunctionAgentGitAdapter\b/,
        message: 'missing core function-agent Git adapter type',
      },
      {
        regex: /\bimpl FunctionAgentGitPort for CoreFunctionAgentGitAdapter\b/,
        message: 'missing function-agent Git port adapter owner',
      },
      {
        regex: /\bpub struct CoreFunctionAgentAiAdapter\b/,
        message: 'missing core function-agent AI adapter type',
      },
      {
        regex: /\bimpl FunctionAgentAiPort for CoreFunctionAgentAiAdapter\b/,
        message: 'missing function-agent AI port adapter owner',
      },
      {
        regex: /\bgit_adapter_commit_snapshot_keeps_staged_diff_and_unstaged_count_separate\b/,
        message: 'missing function-agent Git snapshot boundary regression test',
      },
      {
        regex: /\bgit_adapter_startchat_snapshot_preserves_git_state_when_diff_has_no_head\b/,
        message: 'missing Startchat Git diff fallback regression test',
      },
    ],
  },
  {
    path: 'src/crates/core/src/product_domain_runtime.rs',
    reason:
      'core product-domain runtime owner must centralize concrete MiniApp and function-agent runtime port bindings without moving runtime behavior',
    patterns: [
      {
        regex: /\bpub\(crate\) struct CoreProductDomainRuntime\b/,
        message: 'missing core product-domain runtime owner type',
      },
      {
        regex: /\bfn miniapp_runtime_facade\b/,
        message: 'missing MiniApp runtime facade owner factory',
      },
      {
        regex: /\bfn function_agent_git_adapter\b/,
        message: 'missing function-agent Git adapter owner factory',
      },
      {
        regex: /\bfn function_agent_ai_adapter\b/,
        message: 'missing function-agent AI adapter owner factory',
      },
      {
        regex: /\bfn function_agent_runtime_facade\b/,
        message: 'missing function-agent runtime facade owner factory',
      },
      {
        regex: /\bCoreFunctionAgentGitAdapter\b/,
        message: 'missing core-owned Git adapter binding',
      },
      {
        regex: /\bCoreFunctionAgentAiAdapter\b/,
        message: 'missing core-owned AI adapter binding',
      },
      {
        regex: /\bMiniAppRuntimeFacade\b/,
        message: 'missing MiniApp product-domain facade binding',
      },
      {
        regex: /\bMiniAppStoragePort\b/,
        message: 'missing MiniApp storage port owner binding',
      },
      {
        regex: /\bFunctionAgentRuntimeFacade\b/,
        message: 'missing function-agent product-domain facade binding',
      },
      {
        regex: /\bFunctionAgentGitPort\b/,
        message: 'missing function-agent Git port owner binding',
      },
      {
        regex: /\bFunctionAgentAiPort\b/,
        message: 'missing function-agent AI port owner binding',
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
  const agentToolsRuntimeForbiddenContracts = [
    'GetToolSpecTool',
    'manifest_resolver',
    'unlocked_collapsed_tools',
    'ToolUseContext',
  ];
  const agentToolsManifestRuleText = agentToolsManifestRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of agentToolsRuntimeForbiddenContracts) {
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
  const toolPacksManifestContracts = [
    'GetToolSpecTool',
    'GET_TOOL_SPEC_TOOL_NAME',
    'manifest_resolver',
    'unlocked_collapsed_tools',
    'ToolExposure',
  ];
  for (const contract of toolPacksManifestContracts) {
    if (!toolPacksManifestRuleText.includes(contract)) {
      throw new Error(`tool-packs manifest boundary rule must forbid: ${contract}`);
    }
  }

  const requiredContentContracts = [
    {
      path: 'src/crates/runtime-ports/src/lib.rs',
      contracts: [
        'AgentTurnCancellationPort',
        'RemoteControlStatePort',
        'RuntimeEventSink',
        'remote_image',
      ],
    },
    {
      path: 'src/crates/agent-tools/src/framework.rs',
      contracts: [
        'GET_TOOL_SPEC_TOOL_NAME',
        'ToolExposure',
        'ToolManifestPolicyTool',
        'resolve_tool_manifest_policy',
        'default_exposure',
        'build_tool_manifest_policy_tools',
        'build_collapsed_tool_stub_definition',
        'PromptVisibleToolManifestItem',
        'build_prompt_visible_tool_manifest_definitions',
        'ContextualToolManifestItem',
        'ToolCatalogSnapshotProvider',
        'GetToolSpecCatalogProvider',
        'ContextualVisibleTools',
        'ContextualToolManifest',
        'resolve_contextual_visible_tools',
        'resolve_contextual_tool_manifest',
        'resolve_contextual_visible_tools_from_provider',
        'resolve_contextual_tool_manifest_from_provider',
        'build_get_tool_spec_catalog_description_from_provider',
        'resolve_get_tool_spec_detail_from_provider',
        'build_get_tool_spec_description',
        'GetToolSpecCollapsedToolSummary',
        'GetToolSpecDetail',
        'summarize_get_tool_spec_collapsed_tools',
        'resolve_get_tool_spec_detail',
        'build_get_tool_spec_catalog_description',
        'get_tool_spec_input_schema',
        'get_tool_spec_short_description',
        'render_get_tool_spec_tool_use_message',
        'get_tool_spec_is_readonly',
        'get_tool_spec_is_concurrency_safe',
        'get_tool_spec_needs_permissions',
        'validate_get_tool_spec_input',
        'build_get_tool_spec_assistant_detail',
        'build_get_tool_spec_duplicate_load_result',
        'build_get_tool_spec_detail_result',
        'GetToolSpecExecutionPlan',
        'GetToolSpecExecutionError',
        'resolve_get_tool_spec_execution_plan',
        'resolve_get_tool_spec_execution_result_from_provider',
        'GetToolSpecRuntime',
        'call_results',
        'GetToolSpecLoadObservation',
        'collect_loaded_collapsed_tool_names',
        'sort_tool_manifest_definitions',
        'is_tool_collapsed',
        'get_collapsed_tool_names',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/coordination/coordinator.rs',
      contracts: [
        'AgentSubmissionPort',
        'SessionTranscriptReader',
        'AgentTurnCancellationPort',
        'RemoteControlStatePort',
        'generic attachments',
      ],
    },
    {
      path: 'src/crates/core/src/service_agent_runtime.rs',
      contracts: [
        'CoreServiceAgentRuntime',
        'remote_dialog_host',
        'remote_image_context',
        'agent_submission_port',
        'CoreRemoteDialogRuntimeHost',
        'RemoteExecutionDispatcher',
        'ImageContextData',
        'RemoteImageContextAdapter',
        'AgentSubmissionPort',
        'AgentTurnCancellationPort',
        'RemoteControlStatePort',
        'SessionTranscriptReader',
        'core_service_agent_runtime_owner_keeps_coordinator_port_contracts',
      ],
    },
    {
      path: 'src/crates/services-integrations/src/remote_connect.rs',
      contracts: [
        'RemoteSessionStateTracker',
        'TrackerEvent',
        'RemoteSessionTrackerHost',
        'RemoteSessionTrackerRegistry',
        'make_slim_tool_params',
        'handle_agentic_event',
        'resolve_remote_agent_type',
        'RemoteImageContext',
        'build_remote_image_contexts',
        'resolve_remote_execution_image_contexts',
        'remote_session_restore_target',
        'RemoteCancelDecision',
        'resolve_remote_cancel_decision',
        'REMOTE_FILE_MAX_READ_BYTES',
        'REMOTE_FILE_MAX_CHUNK_BYTES',
        'resolve_remote_file_chunk_range',
        'remote_file_display_name',
        'RemoteDefaultModelsConfig',
        'RemoteModelConfig',
        'RemoteModelCatalog',
        'RemoteModelCatalogPollDelta',
        'RemoteCommand',
        'RemoteResponse',
        'should_send_remote_model_catalog',
        'remote_model_catalog_poll_delta',
        'remote_no_change_poll_response',
        'remote_snapshot_poll_response',
        'remote_persisted_poll_response',
      ],
    },
    {
      path: 'src/crates/services-integrations/tests/remote_connect_contracts.rs',
      contracts: [
        'remote_connect_command_wire_shape_lives_in_owner_contract',
        'remote_connect_response_wire_shape_lives_in_owner_contract',
        'remote_connect_model_catalog_delta_preserves_poll_invalidation_policy',
        'remote_connect_poll_helpers_preserve_delta_and_completion_policy',
        'remote_connect_image_context_policy_preserves_legacy_fallback_shape',
        'remote_connect_image_context_policy_prefers_explicit_contexts',
        'remote_connect_cancel_and_restore_policy_preserve_runtime_decisions',
        'remote_connect_file_transfer_policy_preserves_limits_and_chunk_ranges',
        'remote_connect_file_transfer_policy_preserves_name_fallback',
        'remote_connect_tracker_keeps_finished_turn_snapshot_until_persistence_finalizes',
        'remote_connect_tracker_registry_owns_lifecycle_without_core_state',
        'remote_connect_tracker_ignores_unrelated_direct_session_events',
        'remote_connect_tool_preview_slimming_keeps_short_fields_and_drops_large_strings',
      ],
    },
    {
      path: 'src/crates/core/src/service/remote_connect/remote_server.rs',
      contracts: [
        'CoreServiceAgentRuntime',
        'remote_image_context',
        'core_service_agent_runtime_owner_maps_remote_image_context',
        'remote_execution_prefers_unified_image_contexts_over_legacy_images',
        'remote_cancel_decision_preserves_current_turn_boundaries',
        'remote_restore_target_only_restores_cold_sessions_with_workspace_binding',
        'remote_command_snapshot_covers_execution_poll_and_cancel_surfaces',
        'remote_response_snapshot_preserves_active_turn_and_result_shapes',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/coordination/scheduler.rs',
      contracts: ['remote_queue_policy_preserves_interactive_preempt_and_confirmation_boundary'],
    },
    {
      path: 'src/crates/core/src/agentic/tools/registry.rs',
      contracts: [
        'from_inner',
        'ProductToolDecoratorRef',
        'ProductToolRuntime',
        'get_collapsed_tool_names',
        'resolve_product_readonly_enabled_tools',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/tools/product_runtime.rs',
      contracts: [
        'ProductToolRuntime',
        'SnapshotToolDecorator',
        'ProductSnapshotToolWrapper',
        'builtin_static_tool_providers',
        'StaticToolProviderGroup',
        'product_tool_provider_group_plan',
        'materialize_tool',
        'GetToolSpecTool',
        'ToolRuntimeAssembly',
        'create_registry_from_static_providers',
        'wrap_tool_for_snapshot_tracking',
        'ProductToolCatalogProvider',
        'ToolCatalogSnapshotProvider',
        'GetToolSpecCatalogProvider',
        'get_global_tool_registry',
        'get_agent_registry',
        'ToolCatalogRuntime',
        'product_tool_catalog_runtime',
        'GetToolSpecRuntime',
        'product_get_tool_spec_runtime',
        'resolve_product_tool_manifest',
        'resolve_product_readonly_enabled_tools',
        'resolve_product_get_tool_spec_results',
        'unlocked_collapsed_tools',
        'product_catalog_provider_default_get_tool_spec_catalog_matches_registry',
        'product_tool_runtime_owner_preserves_registry_contract',
        'GetToolSpec requires agent type context',
      ],
    },
    {
      path: 'src/crates/agent-tools/src/framework.rs',
      contracts: [
        'ToolContextFacts',
        'PortableToolContextProvider',
        'ToolWorkspaceKind',
        'StaticToolProvider',
        'StaticToolProviderGroup',
        'ToolRuntimeAssembly',
        'ToolCatalogRuntime',
        'ToolDecoratorRef',
        'SnapshotToolWrapper',
        'SnapshotToolDecorator',
        'create_registry_from_static_providers',
        'install_static_provider',
        'resolve_readonly_enabled_tools',
        'build_get_tool_spec_duplicate_load_result',
        'build_get_tool_spec_detail_result',
        'resolve_get_tool_spec_execution_plan',
        'resolve_get_tool_spec_execution_result_from_provider',
        'GetToolSpecRuntime',
        'call_results',
      ],
    },
    {
      path: 'src/crates/tool-packs/src/lib.rs',
      contracts: [
        'ToolPackFeatureGroup',
        'ToolProviderGroupPlan',
        'all_feature_groups',
        'enabled_feature_groups',
        'product_tool_provider_group_plan',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/tools/tool_adapter.rs',
      contracts: [
        'ToolRegistryItem',
        'ContextualToolManifestItem',
        'Tool::dynamic_tool_info',
        'Tool::is_readonly',
        'Tool::is_enabled',
        'Tool::description_with_context',
        'Tool::input_schema_for_model_with_context',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/tools/manifest_resolver.rs',
      contracts: [
        'resolve_tool_manifest',
        'GET_TOOL_SPEC_TOOL_NAME',
        'resolve_product_visible_tools',
        'resolve_product_tool_manifest',
        'collapsed_tool_names',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/tools/implementations/get_tool_spec_tool.rs',
      contracts: [
        'GetToolSpecTool',
        'build_product_get_tool_spec_catalog_description',
        'product_get_tool_spec_runtime',
        'with_runtime',
        'resolve_product_get_tool_spec_results',
        'map_get_tool_spec_execution_error',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/tools/framework.rs',
      contracts: [
        'ToolExposure',
        'ToolUseContext',
        'to_tool_context_facts',
        'tool_context_facts_omit_runtime_owner_fields_even_when_context_is_populated',
        'customData',
        'cancellationToken',
        'unlocked_collapsed_tools',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs',
      contracts: ['validate_collapsed_tool_usage', 'unlocked_collapsed_tools', 'GetToolSpec'],
    },
    {
      path: 'src/crates/core/src/agentic/execution/execution_engine.rs',
      contracts: [
        'collect_unlocked_collapsed_tools',
        'collect_unlocked_collapsed_tools_dedupes_and_filters_runtime_unlocks',
        'collapsed_tool_names',
        'GetToolSpec',
        'citation_renumber',
      ],
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
      path: 'src/crates/core/src/agentic/agents/definitions/modes/mod.rs',
      contracts: ['mod multitask', 'MultitaskMode'],
    },
    {
      path: 'src/crates/core/src/agentic/agents/definitions/subagents/mod.rs',
      contracts: ['mod general_purpose', 'GeneralPurposeAgent'],
    },
    {
      path: 'src/crates/core/src/agentic/agents/registry/builtin.rs',
      contracts: ['builtin_agent_specs', 'Multitask', 'GeneralPurpose'],
    },
    {
      path: 'src/crates/core/src/agentic/tools/implementations/task_tool.rs',
      contracts: ['run_in_background', 'start_background_subagent', 'background_task_id', 'Background subagent'],
    },
    {
      path: 'src/crates/core/src/agentic/coordination/scheduler.rs',
      contracts: [
        'deliver_background_subagent_result',
        'BackgroundSubagentResult',
        'CurrentRunningTurn',
        'AgentSession',
      ],
    },
    {
      path: 'src/apps/cli/src/ui/startup.rs',
      contracts: [
        'show_available_subagent_list',
        'show_subagent_config_selector',
        'get_subagents_for_query',
        'SubagentQueryContext',
        'update_subagent_override',
      ],
    },
    {
      path: 'src/apps/cli/src/ui/subagent_selector.rs',
      contracts: [
        'SubagentSelectorAction',
        'show_list',
        'show_config',
        'default_enabled',
        'render_subagent_line',
      ],
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
      path: 'src/web-ui/src/main.tsx',
      contracts: ['startupTrace', 'backgroundTaskScheduler', 'initializeAllTools', 'after_render_start'],
    },
    {
      path: 'src/web-ui/src/shared/utils/startupTrace.ts',
      contracts: [
        'sanitizeTraceData',
        'isRemoteTraceRequest',
        'recordApiCall',
        'flushSummary',
        'markPhaseAfterAnimationFrames',
      ],
    },
    {
      path: 'src/web-ui/src/shared/utils/backgroundTaskScheduler.ts',
      contracts: [
        'BackgroundTaskScheduler',
        'inFlightKey',
        'AbortController',
        'BackgroundTaskCancelledError',
        'cancelIdle',
      ],
    },
    {
      path: 'src/web-ui/src/tools/initializeTools.ts',
      contracts: ['initializeAllTools', 'initializeLsp', 'initializeGit', 'does not import every tool'],
    },
    {
      path: 'src/web-ui/src/tools/editor/services/MonacoStartupWarmup.ts',
      contracts: ['scheduleMonacoStartupWarmup', 'backgroundTaskScheduler', 'startup:monaco-warmup'],
    },
    {
      path: 'src/web-ui/src/flow_chat/services/flow-chat-manager/SessionModule.ts',
      contracts: ['historical_session_hydrate_request', 'Load history in the background', "historyState: 'ready'"],
    },
    {
      path: 'src/crates/core/src/miniapp/storage.rs',
      contracts: ['MiniAppStoragePort'],
    },
    {
      path: 'src/crates/core/src/miniapp/builtin/mod.rs',
      contracts: [
        'builtin-pr-review',
        'BUILTIN_APPS',
        'builtin_content_hash',
        'should_seed_builtin_app',
        'resolve_builtin_seed_check',
        'resolve_builtin_seed_action',
        'builtin_source_files',
        'BUILTIN_PLACEHOLDER_COMPILED_HTML',
        'read_builtin_install_marker',
        'parse_builtin_install_marker',
        'write_builtin_install_marker',
        'serialize_builtin_install_marker',
        'recompile',
        'load_customization_metadata',
        'available_builtin_update',
      ],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/builtin.rs',
      contracts: [
        'BuiltinMiniAppBundle',
        'BuiltinInstallMarker',
        'BUILTIN_INSTALL_MARKER',
        'builtin_content_hash',
        'should_seed_builtin_app',
        'BuiltinSeedArtifacts',
        'BuiltinSeedCheck',
        'BuiltinSeedAction',
        'resolve_builtin_seed_check',
        'resolve_builtin_seed_action',
        'serialize_builtin_install_marker',
        'parse_builtin_install_marker',
        'builtin_source_files',
        'BUILTIN_PLACEHOLDER_COMPILED_HTML',
        'build_builtin_package_json',
      ],
    },
    {
      path: 'src/crates/core/src/miniapp/host_dispatch.rs',
      contracts: [
        'dispatch_host',
        'dispatch_fs',
        'dispatch_shell',
        'command_basename_allowed',
        'host_allowed_by_allowlist',
      ],
    },
    {
      path: 'src/crates/core/src/miniapp/js_worker_pool.rs',
      contracts: ['MiniAppRuntimePort'],
    },
    {
      path: 'src/crates/core/src/function_agents/port_adapters.rs',
      contracts: [
        'CoreFunctionAgentGitAdapter',
        'FunctionAgentGitPort',
        'CoreFunctionAgentAiAdapter',
        'FunctionAgentAiPort',
        'git_adapter_commit_snapshot_keeps_staged_diff_and_unstaged_count_separate',
      ],
    },
    {
      path: 'src/crates/core/src/service/remote_connect/bot/command_router.rs',
      contracts: [
        'CoreServiceAgentRuntime',
        'agent_submission_port',
        'build_remote_session_create_request',
      ],
    },
    {
      path: 'src/crates/core/src/product_domain_runtime.rs',
      contracts: [
        'CoreProductDomainRuntime',
        'miniapp_runtime_facade',
        'function_agent_git_adapter',
        'function_agent_ai_adapter',
        'function_agent_runtime_facade',
        'CoreFunctionAgentGitAdapter',
        'CoreFunctionAgentAiAdapter',
        'MiniAppRuntimeFacade',
        'MiniAppStoragePort',
        'FunctionAgentRuntimeFacade',
        'FunctionAgentGitPort',
        'FunctionAgentAiPort',
      ],
    },
    {
      path: 'src/crates/services-integrations/src/remote_ssh/paths.rs',
      contracts: [
        'remote_workspace_runtime_root',
        'remote_workspace_session_mirror_dir',
        'canonicalize_local_workspace_root',
        'normalize_local_workspace_root_for_stable_id',
        'local_workspace_roots_equal',
        'unresolved_remote_session_storage_dir',
      ],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/ports.rs',
      contracts: [
        'MiniAppRuntimeFacade',
        'mark_deps_installed_state',
        'persist_sync_from_fs_result_for_app',
      ],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/storage.rs',
      contracts: [
        'MiniAppStorageLayout',
        'META_JSON',
        'source_file_path',
        'versions_dir',
        'DRAFT_JSON',
        'draft_dir',
        'customization_path',
        'REQUIRED_SOURCE_FILES',
        'PLACEHOLDER_COMPILED_HTML',
        'MiniAppImportLayout',
        'build_import_fallbacks',
      ],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/lifecycle.rs',
      contracts: [
        'mark_deps_installed_state',
        'clear_worker_restart_required_state',
        'prepare_rollback_app',
        'apply_recompile_result',
        'apply_sync_from_fs_result',
        'apply_import_runtime_state',
      ],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/draft.rs',
      contracts: ['MiniAppDraftManifest', 'MiniAppDraft', 'build_draft_manifest', 'build_draft_response'],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/runtime.rs',
      contracts: ['runtime_lookup_order', 'candidate_executable_path', 'versioned_executable_candidate'],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/host_routing.rs',
      contracts: [
        'command_basename_for_allowlist',
        'command_basename_allowed',
        'host_allowed_by_allowlist',
      ],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/customization.rs',
      contracts: [
        'MiniAppCustomizationMetadata',
        'MiniAppDeclinedBuiltinUpdate',
        'MiniAppPermissionDiff',
        'diff_permissions',
        'apply_draft_customization_metadata',
        'mark_builtin_update_available_metadata',
        'decline_builtin_update_metadata',
        'is_current_declined_builtin_update',
      ],
    },
    {
      path: 'src/crates/core/src/miniapp/manager.rs',
      contracts: [
        'apply_draft_customization_metadata',
        'mark_builtin_update_available_metadata',
        'decline_builtin_update_metadata',
        'storage.load_customization_metadata',
        'CoreProductDomainRuntime',
        'MiniAppRuntimeFacade',
        'persist_sync_from_fs_result_for_app',
        'compile_source',
        'REQUIRED_SOURCE_FILES',
        'MiniAppImportLayout',
        'build_import_fallbacks',
        'apply_import_runtime_state',
        'runtime_preflight_preserves_recompile_sync_rollback_and_deps_state',
        'import_from_path_preserves_fallback_files_recompile_and_runtime_state',
      ],
    },
    {
      path: 'src/crates/core/src/function_agents/git-func-agent/ai_service.rs',
      contracts: [
        'prepare_commit_ai_prompt',
        'parse_commit_ai_response',
        'send_message',
        'AgentError::internal_error',
        'parse_commit_response_preserves_product_domain_response_policy',
      ],
    },
    {
      path: 'src/crates/core/src/function_agents/startchat-func-agent/ai_service.rs',
      contracts: [
        'build_work_state_analysis_prompt',
        'parse_work_state_analysis_response',
        'send_message',
        'AgentError::internal_error',
        'parse_complete_analysis_preserves_product_domain_response_policy',
      ],
    },
    {
      path: 'src/crates/core/src/function_agents/git-func-agent/commit_generator.rs',
      contracts: [
        'CoreProductDomainRuntime',
        'function_agent_git_adapter',
        'function_agent_ai_adapter',
        'function_agent_runtime_facade',
      ],
    },
    {
      path: 'src/crates/core/src/function_agents/startchat-func-agent/work_state_analyzer.rs',
      contracts: [
        'CoreProductDomainRuntime',
        'function_agent_git_adapter',
        'function_agent_ai_adapter',
        'function_agent_runtime_facade',
      ],
    },
    {
      path: 'src/crates/product-domains/src/function_agents/ports.rs',
      contracts: [
        'FunctionAgentRuntimeFacade',
        'generate_commit_message',
        'analyze_work_state',
        'git_work_state_from_snapshot',
        'StartchatTimeSnapshot',
        'startchat_time_snapshot',
      ],
    },
    {
      path: 'src/crates/product-domains/src/function_agents/common.rs',
      contracts: ['extract_json_from_ai_response', 'try_repair_json'],
    },
    {
      path: 'src/crates/product-domains/src/function_agents/startchat_func_agent/utils.rs',
      contracts: [
        'WORK_STATE_ANALYSIS_PROMPT',
        'build_work_state_analysis_prompt',
        'ParsedCompleteAnalysis',
        'parse_complete_analysis_value',
        'parse_complete_analysis_json',
        'parse_work_state_analysis_response',
      ],
    },
    {
      path: 'src/crates/product-domains/src/function_agents/git_func_agent/utils.rs',
      contracts: [
        'COMMIT_MESSAGE_PROMPT',
        'parse_commit_analysis_value',
        'parse_commit_analysis_json',
        'truncate_diff_for_commit_prompt',
        'prepare_commit_prompt',
        'prepare_commit_ai_prompt',
        'parse_commit_ai_response',
      ],
    },
    {
      path: 'src/crates/core/src/miniapp/runtime_detect.rs',
      contracts: ['runtime_lookup_order', 'candidate_executable_path', 'versioned_executable_candidate', 'get_version'],
    },
  ];
  for (const { path, contracts } of requiredContentContracts) {
    const matchingRules = requiredContentRules.filter((rule) => rule.path === path);
    if (matchingRules.length === 0) {
      throw new Error(`missing owner content anchor rule for ${path}`);
    }
    const ruleText = matchingRules
      .flatMap((rule) => rule.patterns)
      .map((pattern) => pattern.regex.source)
      .join('\n');
    for (const contract of contracts) {
      if (!ruleText.includes(contract) && !ruleText.includes(escapeRegex(contract))) {
        throw new Error(`owner content anchor rule for ${path} must require: ${contract}`);
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
    'dunce::canonicalize',
    'path_buf_to_stable_local_root_string',
    'join\\("_unresolved"\\)',
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
  for (const dep of ['dunce', 'futures', 'reqwest', 'sse-stream']) {
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
    'RemoteDefaultModelsConfig',
    'RemoteModelConfig',
    'RemoteModelCatalog',
    'RemoteModelCatalogPollDelta',
    'RemoteCommand',
    'RemoteResponse',
    'TrackerState',
    'TrackerEvent',
    'RemoteSessionStateTracker',
    'DashMap',
    'make_slim_params',
    'match mobile_type',
    'RemoteCancelDecision',
    'resolve_remote_cancel_decision',
    'remote_session_restore_target',
    'resolve_remote_execution_image_contexts',
    'RemoteImageContextAdapter',
    'MAX_SIZE',
    'MAX_CHUNK',
    'unwrap_or\\("file"\\)',
    'resolve_workspace_path',
    'detect_mime_type',
    'read_workspace_file',
    'read_remote_workspace_file',
    'read_remote_workspace_file_chunk',
    'read_remote_workspace_file_info',
    'should_send_remote_model_catalog',
    'remote_model_catalog_poll_delta',
    'remote_no_change_poll_response',
    'remote_snapshot_poll_response',
    'remote_persisted_poll_response',
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
