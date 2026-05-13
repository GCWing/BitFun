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
      'bitfun-services-core',
      'chrono',
      'git2',
      'notify',
      'rand',
      'rmcp',
      'sha2',
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
    path: 'src/crates/core/src/service/mcp/config/location.rs',
    importPrefix: 'bitfun_services_integrations::mcp',
    reason: 'core MCP config location facade must only re-export the integrations owner crate',
  },
  {
    path: 'src/crates/core/src/service/announcement/types.rs',
    importPrefix: 'bitfun_services_integrations::announcement',
    reason: 'core announcement types facade must only re-export the integrations owner crate',
  },
];

const forbiddenContentRules = [
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

  const productDomainProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'product-domains',
  );
  if (!productDomainProfile?.forbiddenNonOptionalDeps.includes('dirs')) {
    throw new Error('product-domains default profile must forbid non-optional dirs');
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
  ];
  const ruleText = remoteWorkspaceRule.patterns.map((pattern) => pattern.regex.source).join('\n');
  for (const helper of remoteWorkspaceHelpers) {
    if (!ruleText.includes(helper)) {
      throw new Error(`remote SSH workspace boundary rule must forbid helper: ${helper}`);
    }
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
  ];
  const mcpProcessRuleText = mcpProcessRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const helper of mcpProcessHelpers) {
    if (!mcpProcessRuleText.includes(helper)) {
      throw new Error(`MCP server process boundary rule must forbid helper: ${helper}`);
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

if (failures.length > 0) {
  console.error('Core boundary check failed.');
  for (const failure of failures) {
    console.error(`${toRepoPath(failure.path)}:${failure.line}: ${failure.message}`);
  }
  process.exit(1);
}

console.log('Core boundary check passed.');
