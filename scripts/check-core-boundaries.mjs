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
  'agent-runtime',
  'harness',
  'runtime-ports',
  'runtime-services',
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
    crateName: 'runtime-services',
    reason: 'runtime-services must stay a typed service assembly contract without concrete runtime implementations',
    forbiddenDeps: [
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
    crateName: 'agent-runtime',
    reason: 'agent-runtime must own portable runtime decisions without concrete service or product implementations',
    forbiddenDeps: [
      'bitfun-core',
      'bitfun-ai-adapters',
      'bitfun-services-core',
      'bitfun-services-integrations',
      'bitfun-tool-packs',
      'bitfun-product-domains',
      'bitfun-transport',
      'terminal-core',
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
    crateName: 'harness',
    reason:
      'harness must own workflow contracts without concrete service, product, or platform implementations',
    forbiddenDeps: [
      'bitfun-core',
      'bitfun-ai-adapters',
      'bitfun-services-core',
      'bitfun-services-integrations',
      'bitfun-tool-packs',
      'bitfun-product-domains',
      'bitfun-transport',
      'terminal-core',
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
    crateName: 'core',
    profileName: 'no-default runtime-surface-light profile',
    reason:
      'bitfun-core no-default profile must not force product/runtime integration dependencies',
    forbiddenNonOptionalDeps: [
      'aes',
      'aes-gcm',
      'bitfun-product-domains',
      'bitfun-relay-server',
      'bitfun-tool-packs',
      'chrono-tz',
      'cron',
      'dashmap',
      'eventsource-stream',
      'filetime',
      'flate2',
      'fs2',
      'git2',
      'glob',
      'globset',
      'hostname',
      'image',
      'include_dir',
      'indexmap',
      'local-ip-address',
      'mac_address',
      'md5',
      'qrcode',
      'rand',
      'rmcp',
      'russh',
      'russh-keys',
      'russh-sftp',
      'shellexpand',
      'sse-stream',
      'ssh_config',
      'similar',
      'tool-runtime',
      'tokio-tungstenite',
      'x25519-dalek',
    ],
  },
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
    crateName: 'runtime-services',
    profileName: 'default runtime service assembly profile',
    reason: 'runtime-services default profile must not compile concrete service or product runtime implementations',
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
    crateName: 'agent-runtime',
    profileName: 'default agent runtime decision profile',
    reason: 'agent-runtime default profile must not compile concrete services or product surfaces',
    forbiddenNonOptionalDeps: [
      'bitfun-core',
      'bitfun-ai-adapters',
      'bitfun-services-core',
      'bitfun-services-integrations',
      'bitfun-tool-packs',
      'bitfun-product-domains',
      'bitfun-transport',
      'terminal-core',
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
      'log',
      'sha2',
      'which',
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
      'uuid',
      'bitfun-relay-server',
    ],
  },
];

const optionalDependencyFeatureOwnerRules = [
  {
    crateName: 'core',
    reason:
      'bitfun-core product/runtime optional dependencies must stay owned by explicit feature gates',
    dependencies: [
      { depName: 'aes', ownerFeatures: ['service-integrations'] },
      { depName: 'aes-gcm', ownerFeatures: ['service-integrations', 'ssh-remote'] },
      { depName: 'bitfun-product-domains', ownerFeatures: ['product-domains'] },
      { depName: 'bitfun-relay-server', ownerFeatures: ['service-integrations'] },
      { depName: 'bitfun-tool-packs', ownerFeatures: ['tool-packs'] },
      { depName: 'chrono-tz', ownerFeatures: ['product-full'] },
      { depName: 'cron', ownerFeatures: ['product-full'] },
      { depName: 'dashmap', ownerFeatures: ['product-full'] },
      { depName: 'eventsource-stream', ownerFeatures: ['product-full'] },
      { depName: 'filetime', ownerFeatures: ['product-full'] },
      { depName: 'flate2', ownerFeatures: ['product-full'] },
      { depName: 'fs2', ownerFeatures: ['product-full'] },
      { depName: 'git2', ownerFeatures: ['service-integrations'] },
      { depName: 'glob', ownerFeatures: ['product-full'] },
      { depName: 'globset', ownerFeatures: ['product-full'] },
      { depName: 'hostname', ownerFeatures: ['service-integrations'] },
      { depName: 'image', ownerFeatures: ['service-integrations'] },
      { depName: 'include_dir', ownerFeatures: ['product-full'] },
      { depName: 'indexmap', ownerFeatures: ['product-full'] },
      { depName: 'local-ip-address', ownerFeatures: ['service-integrations'] },
      { depName: 'mac_address', ownerFeatures: ['service-integrations'] },
      { depName: 'md5', ownerFeatures: ['product-full', 'service-integrations'] },
      { depName: 'qrcode', ownerFeatures: ['service-integrations'] },
      { depName: 'rand', ownerFeatures: ['service-integrations', 'ssh-remote'] },
      { depName: 'rmcp', ownerFeatures: ['service-integrations'] },
      { depName: 'russh', ownerFeatures: ['ssh-remote'] },
      { depName: 'russh-keys', ownerFeatures: ['ssh-remote'] },
      { depName: 'russh-sftp', ownerFeatures: ['ssh-remote'] },
      { depName: 'shellexpand', ownerFeatures: ['ssh-remote'] },
      { depName: 'similar', ownerFeatures: ['product-full'] },
      { depName: 'sse-stream', ownerFeatures: ['service-integrations'] },
      { depName: 'ssh_config', ownerFeatures: ['ssh-remote'] },
      { depName: 'tokio-tungstenite', ownerFeatures: ['service-integrations'] },
      { depName: 'tool-runtime', ownerFeatures: ['product-full'] },
      { depName: 'x25519-dalek', ownerFeatures: ['service-integrations'] },
    ],
  },
  {
    crateName: 'services-integrations',
    reason:
      'services-integrations optional runtime dependencies must stay owned by explicit integration features',
    dependencies: [
      { depName: 'aes-gcm', ownerFeatures: ['mcp'] },
      { depName: 'anyhow', ownerFeatures: ['mcp'] },
      { depName: 'async-trait', ownerFeatures: ['mcp', 'remote-connect'] },
      { depName: 'base64', ownerFeatures: ['mcp', 'remote-connect'] },
      { depName: 'bitfun-runtime-ports', ownerFeatures: ['remote-connect'] },
      { depName: 'bitfun-services-core', ownerFeatures: ['git', 'mcp'] },
      { depName: 'chrono', ownerFeatures: ['git'] },
      { depName: 'dunce', ownerFeatures: ['remote-ssh'] },
      { depName: 'futures', ownerFeatures: ['mcp'] },
      { depName: 'git2', ownerFeatures: ['git'] },
      { depName: 'notify', ownerFeatures: ['file-watch'] },
      { depName: 'rand', ownerFeatures: ['mcp'] },
      { depName: 'reqwest', ownerFeatures: ['mcp'] },
      { depName: 'rmcp', ownerFeatures: ['mcp'] },
      { depName: 'sha2', ownerFeatures: ['remote-ssh'] },
      { depName: 'sse-stream', ownerFeatures: ['mcp'] },
      { depName: 'thiserror', ownerFeatures: ['git'] },
      { depName: 'tokio-util', ownerFeatures: ['remote-ssh'] },
      { depName: 'uuid', ownerFeatures: ['remote-connect'] },
    ],
  },
  {
    crateName: 'product-domains',
    reason:
      'product-domains optional runtime dependencies must stay owned by explicit product-domain features',
    dependencies: [
      { depName: 'dirs', ownerFeatures: ['miniapp'] },
      { depName: 'log', ownerFeatures: ['function-agents'] },
      { depName: 'sha2', ownerFeatures: ['miniapp'] },
      { depName: 'which', ownerFeatures: ['miniapp'] },
    ],
  },
];

const productCoreFeatureAssemblyRules = [
  {
    manifestPath: 'src/apps/desktop/Cargo.toml',
    dependencyName: 'bitfun-core',
    requiredFeatures: ['product-full'],
    reason: 'desktop must explicitly assemble the full bitfun-core product runtime',
  },
  {
    manifestPath: 'src/apps/cli/Cargo.toml',
    dependencyName: 'bitfun-core',
    requiredFeatures: ['product-full'],
    reason: 'CLI must explicitly assemble the full bitfun-core product runtime',
  },
  {
    manifestPath: 'src/crates/acp/Cargo.toml',
    dependencyName: 'bitfun-core',
    requiredFeatures: ['product-full'],
    reason: 'ACP must explicitly assemble the full bitfun-core product runtime',
  },
];

const productCoreFeatureAssemblyScanRoots = ['src/apps', 'src/crates/acp'];

const coreProductFullFeatureAssemblyRule = {
  manifestPath: 'src/crates/core/Cargo.toml',
  featureName: 'product-full',
  requiredFeatureRefs: ['ssh-remote', 'product-domains', 'service-integrations', 'tool-packs'],
  reason: 'bitfun-core product-full must explicitly assemble current owner feature groups',
};

const ownerCrateFeatureAssemblyRules = [
  {
    manifestPath: 'src/crates/tool-packs/Cargo.toml',
    reason: 'tool-packs must keep product feature groups explicit and default-light',
    requiredProductFullFeatures: [
      'basic',
      'git',
      'mcp',
      'browser-web',
      'computer-use',
      'image-analysis',
      'miniapp',
      'agent-control',
    ],
  },
  {
    manifestPath: 'src/crates/services-integrations/Cargo.toml',
    reason: 'services-integrations must keep integration feature groups explicit and default-light',
    requiredProductFullFeatures: [
      'announcement',
      'file-watch',
      'git',
      'mcp',
      'remote-connect',
      'remote-ssh',
    ],
  },
  {
    manifestPath: 'src/crates/product-domains/Cargo.toml',
    reason: 'product-domains must keep product domain feature groups explicit and default-light',
    requiredProductFullFeatures: ['miniapp', 'function-agents'],
  },
];

const facadeOnlyFiles = [
  {
    path: 'src/crates/core/src/infrastructure/filesystem/mod.rs',
    importPrefix: 'bitfun_services_core::filesystem',
    reason: 'core filesystem infrastructure facade must only re-export the services-core owner crate',
  },
  {
    path: 'src/crates/core/src/service/filesystem/listing.rs',
    importPrefix: 'bitfun_services_core::filesystem',
    reason: 'core filesystem listing facade must only re-export the services-core owner crate',
  },
  {
    path: 'src/crates/core/src/service/filesystem/types.rs',
    importPrefix: 'bitfun_services_core::filesystem',
    reason: 'core filesystem DTO facade must only re-export the services-core owner crate',
  },
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
    path: 'src/crates/core/src/service/filesystem/service.rs',
    patterns: [
      {
        regex: /\btokio::fs::/,
        message:
          'core filesystem service must not own async local filesystem IO; use bitfun-services-core filesystem primitives',
      },
      {
        regex: /\bstd::fs::/,
        message:
          'core filesystem service must not own sync local filesystem IO; use bitfun-services-core filesystem primitives',
      },
      {
        regex: /\bignore::WalkBuilder\b/,
        message:
          'core filesystem service must not own local file walking/search implementation; use bitfun-services-core filesystem primitives',
      },
      {
        regex: /\bsha2::/,
        message:
          'core filesystem service must not own editor-sync hashing implementation; use bitfun-services-core filesystem primitives',
      },
      {
        regex: /\bbase64::/,
        message:
          'core filesystem service must not own binary file encoding implementation; use bitfun-services-core filesystem primitives',
      },
    ],
  },
  {
    path: 'src/crates/core/src/miniapp/runtime_detect.rs',
    patterns: [
      {
        regex: /\bCoreMiniAppRuntimeProbe\b/,
        message:
          'core MiniApp runtime_detect must remain a compatibility facade; concrete probe owner is product-domains',
      },
      {
        regex: /\bwhich::which\b/,
        message:
          'core MiniApp runtime_detect must not own PATH lookup; use product-domain runtime detection',
      },
      {
        regex: /\bcreate_command\b/,
        message:
          'core MiniApp runtime_detect must not own version process execution; use product-domain runtime detection',
      },
      {
        regex: /\bstd::fs::read_dir\b/,
        message:
          'core MiniApp runtime_detect must not own version-manager directory scanning; use product-domain runtime detection',
      },
    ],
  },
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
      {
        regex: /\bpub struct ToolUseContext\b/,
        message:
          'core tool framework must not own ToolUseContext; re-export it from tool_context_runtime',
      },
      {
        regex: /\bcall_with_tool_runtime_hooks\b/,
        message:
          'core tool framework must not wire runtime hooks directly; delegate through tool_context_runtime',
      },
      {
        regex: /\bdeep_review_shared_context_measurement_snapshot\b/,
        message:
          'core tool framework must not own runtime hook regressions; keep them in tool_context_runtime',
      },
      {
        regex: /\bget_global_coordinator\b/,
        message:
          'core tool framework must not own runtime checkpoint coordination; keep it in tool_context_runtime',
      },
      {
        regex: /\bGitService\b/,
        message:
          'core tool framework must not own git-backed checkpoint runtime; keep it in tool_context_runtime',
      },
      {
        regex: /\bget_workspace_runtime_service_arc\b/,
        message:
          'core tool framework must not own workspace runtime lookup; keep it in tool_context_runtime',
      },
      {
        regex: /\bremote_workspace_runtime_root\b/,
        message:
          'core tool framework must not own remote runtime-root lookup; keep it in tool_context_runtime',
      },
      {
        regex: /\bget_path_manager_arc\b/,
        message:
          'core tool framework must not own host runtime-root lookup; keep it in tool_context_runtime',
      },
      {
        regex: /\bpost_call_hooks::record_successful_tool_call\b/,
        message:
          'core tool framework must not own post-call runtime hooks; keep them in tool_context_runtime',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'tool pipeline must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
      {
        regex: /\bfn serialize_result_for_assistant\b/,
        message:
          'core tool pipeline must not own provider-neutral assistant result rendering; use bitfun-agent-tools',
      },
      {
        regex: /\bconst TOOL_ERROR_ARGUMENTS_PREVIEW_BYTES\b/,
        message:
          'core tool pipeline must not own tool error argument preview limits; use bitfun-agent-tools',
      },
      {
        regex: /\bfn truncate_arguments_preview\b/,
        message:
          'core tool pipeline must not own tool error argument preview rendering; use bitfun-agent-tools',
      },
      {
        regex: /\bfn truncate_raw_arguments_preview\b/,
        message:
          'core tool pipeline must not own raw tool argument preview rendering; use bitfun-agent-tools',
      },
      {
        regex: /\bconst USER_STEERING_INTERRUPTED_MESSAGE\b/,
        message:
          'core tool pipeline must not own steering-interrupted result presentation; use bitfun-agent-tools',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/subagent_runtime/mod.rs',
    patterns: [
      {
        regex: /\bstruct\s+DelegationPolicy\b/,
        message:
          'core subagent runtime must not redefine DelegationPolicy; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+SubagentContextMode\b/,
        message:
          'core subagent runtime must not redefine SubagentContextMode; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/coordination/coordinator.rs',
    patterns: [
      {
        regex: /\benum\s+DialogTriggerSource\b/,
        message:
          'core coordinator must not redefine DialogTriggerSource; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/coordination/scheduler.rs',
    patterns: [
      {
        regex: /\benum\s+DialogQueuePriority\b/,
        message:
          'core scheduler must not redefine DialogQueuePriority; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+DialogSubmissionPolicy\b/,
        message:
          'core scheduler must not redefine DialogSubmissionPolicy; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+DialogSubmitOutcome\b/,
        message:
          'core scheduler must not redefine DialogSubmitOutcome; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+AgentSessionReplyRoute\b/,
        message:
          'core scheduler must not redefine AgentSessionReplyRoute; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+DialogSteerOutcome\b/,
        message:
          'core scheduler must not redefine DialogSteerOutcome; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/round_preempt.rs',
    patterns: [
      {
        regex: /\btrait\s+DialogRoundPreemptSource\b/,
        message:
          'core round preempt runtime must not redefine DialogRoundPreemptSource; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+RoundInjection\b/,
        message:
          'core round preempt runtime must not redefine RoundInjection; use bitfun-runtime-ports',
      },
      {
        regex: /\btrait\s+DialogRoundInjectionSource\b/,
        message:
          'core round preempt runtime must not redefine DialogRoundInjectionSource; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+RoundInjectionKind\b/,
        message:
          'core round preempt runtime must not redefine RoundInjectionKind; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+RoundInjectionTarget\b/,
        message:
          'core round preempt runtime must not redefine RoundInjectionTarget; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/goal_mode/mod.rs',
    patterns: [
      {
        regex: /\bconst\s+GOAL_MODE_METADATA_KEY\b/,
        message: 'core goal mode types must not redefine GOAL_MODE_METADATA_KEY; use bitfun-runtime-ports',
      },
      {
        regex: /\bconst\s+MAX_GOAL_CONTINUATIONS\b/,
        message: 'core goal mode types must not redefine MAX_GOAL_CONTINUATIONS; use bitfun-runtime-ports',
      },
      {
        regex: /\bconst\s+MAX_CONTEXT_SUMMARY_CHARS\b/,
        message: 'core goal mode types must not redefine MAX_CONTEXT_SUMMARY_CHARS; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+ThreadGoal\b/,
        message: 'core goal mode types must not redefine ThreadGoal; use bitfun-runtime-ports',
      },
      {
        regex: /\benum\s+ThreadGoalStatus\b/,
        message: 'core goal mode types must not redefine ThreadGoalStatus; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+GoalGenerationResult\b/,
        message: 'core goal mode types must not redefine GoalGenerationResult; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+ThreadGoalToolResponse\b/,
        message: 'core goal mode types must not redefine ThreadGoalToolResponse; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+GoalActivationResult\b/,
        message: 'core goal mode types must not redefine GoalActivationResult; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+GoalContinuationPlan\b/,
        message: 'core goal mode types must not redefine GoalContinuationPlan; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/core/message.rs',
    patterns: [
      {
        regex: /\bstruct\s+CompressionContract\b/,
        message: 'core message model must not redefine CompressionContract; use bitfun-runtime-ports',
      },
      {
        regex: /\bstruct\s+CompressionContractItem\b/,
        message: 'core message model must not redefine CompressionContractItem; use bitfun-runtime-ports',
      },
      {
        regex: /\bfn\s+render_contract_items\b/,
        message: 'core message model must not own compression contract rendering; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/workspace/manager.rs',
    patterns: [
      {
        regex: /\bstruct\s+RelatedPath\b/,
        message: 'core workspace manager must not redefine RelatedPath; use bitfun-runtime-ports',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/file_read_state_runtime.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'file read-state runtime must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/tool_result_storage.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'tool-result storage runtime must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/post_call_hooks.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'post-call hooks must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/tool_adapter.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'tool adapter must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/product_runtime.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'product tool runtime must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/product_runtime/catalog.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'product tool runtime catalog must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/product_runtime/get_tool_spec_tool.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'GetToolSpec adapter must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/manifest_resolver.rs',
    patterns: [
      {
        regex: /framework::(?:\{[^}]*\bToolUseContext\b[^}]*\}|\bToolUseContext\b)/,
        message:
          'manifest resolver must import ToolUseContext from tool_context_runtime, not the framework re-export',
      },
      {
        regex: /\bContextualToolManifest\b/,
        message:
          'manifest resolver must stay a compatibility facade; contextual manifest conversion belongs in product_runtime/catalog',
      },
      {
        regex: /\bContextualVisibleTools\b/,
        message:
          'manifest resolver must stay a compatibility facade; contextual visible-tool conversion belongs in product_runtime/catalog',
      },
      {
        regex: /\bToolManifestDefinition\b/,
        message:
          'manifest resolver must not own manifest DTO conversion; use product_runtime/catalog',
      },
      {
        regex: /\bfn\s+to_core_tool_definition\b/,
        message:
          'manifest resolver must not own ToolDefinition conversion; use product_runtime/catalog',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/workspace.rs',
    patterns: [
      {
        regex: /\bpub\s+trait\s+WorkspaceFileSystem\b/,
        message:
          'workspace file-system contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
      {
        regex: /\bpub\s+trait\s+WorkspaceShell\b/,
        message:
          'workspace shell contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
      {
        regex: /\bpub\s+struct\s+WorkspaceServices\b/,
        message:
          'workspace service bundle contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
      {
        regex: /\bpub\s+struct\s+WorkspaceCommandOptions\b/,
        message:
          'workspace command contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
      {
        regex: /\bpub\s+struct\s+WorkspaceCommandResult\b/,
        message:
          'workspace command result contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
      {
        regex: /\bpub\s+struct\s+WorkspaceDirEntry\b/,
        message:
          'workspace directory entry contract must be owned by bitfun-runtime-ports; keep only concrete adapters or re-exports in core',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/execution/execution_engine.rs',
    patterns: [
      {
        regex: /\bGetToolSpecLoadObservation\b/,
        message:
          'execution engine must not own collapsed-tool unlock observation details; use product_runtime unlock state owner',
      },
      {
        regex: /\bcollect_loaded_collapsed_tool_names\b/,
        message:
          'execution engine must not call generic collapsed-tool collector directly; use product_runtime unlock state owner',
      },
      {
        regex: /\bfn\s+collect_unlocked_collapsed_tools\b/,
        message:
          'execution engine must not own collapsed-tool unlock collection; use product_runtime unlock state owner',
      },
    ],
  },
  {
    path: 'src/crates/core/src/miniapp/manager.rs',
    patterns: [
      {
        regex: /\bbuild_runtime_state\b/,
        message:
          'core MiniApp manager must not build runtime state directly; use product-domain lifecycle helpers',
      },
      {
        regex: /\bbuild_source_revision\b/,
        message:
          'core MiniApp manager must not build source revisions directly; use product-domain lifecycle helpers',
      },
      {
        regex: /\bbuild_deps_revision\b/,
        message:
          'core MiniApp manager must not build dependency revisions directly; use product-domain lifecycle helpers',
      },
      {
        regex: /\bapp\.version\s*\+=\s*1\b/,
        message:
          'core MiniApp manager must not own version increments for lifecycle transitions; use product-domain lifecycle helpers',
      },
      {
        regex: /\bapp\.runtime\s*=/,
        message:
          'core MiniApp manager must not own runtime-state replacement for lifecycle transitions; use product-domain lifecycle helpers',
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
      {
        regex: /\bfn\s+normalize_absolute_posix_path\b/,
        message:
          'core tool restrictions must not redefine remote POSIX path normalization; use bitfun-agent-tools',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/workspace_paths.rs',
    patterns: [
      {
        regex: /\bpub const BITFUN_RUNTIME_URI_PREFIX\b/,
        message:
          'core workspace path facade must not redefine the runtime URI prefix; use bitfun-agent-tools',
      },
      {
        regex: /\bpub struct ParsedBitFunRuntimeUri\b/,
        message:
          'core workspace path facade must not redefine ParsedBitFunRuntimeUri; use bitfun-agent-tools',
      },
      {
        regex: /\bfn\s+posix_normalize_components\b/,
        message:
          'core workspace path facade must not redefine remote POSIX path normalization; use bitfun-agent-tools',
      },
      {
        regex: /Component::ParentDir/,
        message:
          'core workspace path facade must not redefine host path normalization; use bitfun-agent-tools',
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
    path: 'src/crates/core/src/agentic/tools/file_read_state_runtime.rs',
    patterns: [
      {
        regex: /\bnormalize_string\b/,
        message:
          'core file read-state runtime must delegate pure freshness normalization to bitfun-agent-tools',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/tool_result_storage.rs',
    patterns: [
      {
        regex: /\bfn\s+generate_preview\b/,
        message:
          'core tool result storage must delegate pure preview generation to bitfun-agent-tools',
      },
      {
        regex: /\bfn\s+build_persisted_output_message\b/,
        message:
          'core tool result storage must delegate persisted-output rendering to bitfun-agent-tools',
      },
      {
        regex: /\bfn\s+select_candidates_to_persist\b/,
        message:
          'core tool result storage must delegate round-budget selection to bitfun-agent-tools',
      },
      {
        regex: /\bstruct\s+ToolResultStoragePolicy\b/,
        message:
          'core tool result storage must use the provider-neutral storage policy from bitfun-agent-tools',
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
        regex: /\bpub\(crate\) struct CoreRemoteDialogRuntimeHost\b/,
        message:
          'remote_server must not own concrete remote dialog runtime host; keep it in service_agent_runtime',
      },
      {
        regex: /\bpub\(crate\) struct CoreRemoteCancelRuntimeHost\b/,
        message:
          'remote_server must not own concrete remote cancel runtime host; keep it in service_agent_runtime',
      },
      {
        regex: /\bpub\(crate\) struct CoreRemoteWorkspaceFileRuntimeHost\b/,
        message:
          'remote_server must not own concrete remote workspace file runtime host; keep it in service_agent_runtime',
      },
      {
        regex: /\bstruct CoreRemoteSessionTrackerHost\b/,
        message:
          'remote_server must not own concrete remote tracker host; keep it in service_agent_runtime',
      },
      {
        regex: /\basync fn resolve_session_model_id\b/,
        message:
          'remote_server must not own remote session model resolution; keep it in service_agent_runtime',
      },
      {
        regex: /\basync fn load_remote_model_catalog\b/,
        message:
          'remote_server must not own remote model catalog loading; keep it in service_agent_runtime',
      },
      {
        regex: /\bget_global_config_service\b/,
        message:
          'remote_server must not own remote model config access; route it through service_agent_runtime',
      },
      {
        regex: /\bfn compress_data_url_for_mobile\b/,
        message:
          'remote_server must not own remote chat thumbnail compression; keep it in service_agent_runtime',
      },
      {
        regex: /\bfn turns_to_chat_messages\b/,
        message:
          'remote_server must not own persisted turn to remote chat conversion; keep it in service_agent_runtime',
      },
      {
        regex: /\basync fn load_chat_messages_from_conversation_persistence\b/,
        message:
          'remote_server must not own remote chat history persistence loading; keep it in service_agent_runtime',
      },
      {
        regex: /\bfn strip_user_input_tags\b/,
        message:
          'remote_server must not own remote user input display cleanup; keep it in service_agent_runtime',
      },
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
        regex: /\bstruct RemoteCancelTaskRequest\b/,
        message: 'core remote-connect server must not own cancel task request contracts; use the integrations contract',
      },
      {
        regex: /\btrait RemoteCancelRuntimeHost\b/,
        message: 'core remote-connect server must not own cancel runtime host contracts; use the integrations contract',
      },
      {
        regex: /\bfn cancel_remote_task\b/,
        message: 'core remote-connect server must not own cancel orchestration; use the integrations helper',
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
        regex: /\bfn remote_file_content_response\b/,
        message: 'core remote-connect server must not own remote file content response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_file_chunk_response\b/,
        message: 'core remote-connect server must not own remote file chunk response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_file_info_response\b/,
        message: 'core remote-connect server must not own remote file-info response assembly; use the integrations helper',
      },
      {
        regex: /\bfn handle_remote_workspace_file_command\b/,
        message: 'core remote-connect server must not own remote file command orchestration; use the integrations helper',
      },
      {
        regex: /general_purpose::STANDARD\.encode/,
        message: 'core remote-connect server must not own remote response base64 wrapping; use the integrations helper',
      },
      {
        regex: /\bfn remote_dialog_submit_response\b/,
        message: 'core remote-connect server must not own remote dialog response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_task_cancel_response\b/,
        message: 'core remote-connect server must not own remote cancel response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_interaction_accepted_response\b/,
        message: 'core remote-connect server must not own remote interaction response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_answer_question_response\b/,
        message: 'core remote-connect server must not own remote answer response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_workspace_info_response\b/,
        message: 'core remote-connect server must not own workspace-info response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_recent_workspaces_response\b/,
        message: 'core remote-connect server must not own recent-workspaces response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_assistant_list_response\b/,
        message: 'core remote-connect server must not own assistant-list response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_workspace_updated_response\b/,
        message: 'core remote-connect server must not own workspace-updated response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_assistant_updated_response\b/,
        message: 'core remote-connect server must not own assistant-updated response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_session_info\b/,
        message: 'core remote-connect server must not own session response facts assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_session_list_response\b/,
        message: 'core remote-connect server must not own session-list response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_initial_sync_response\b/,
        message: 'core remote-connect server must not own initial-sync response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_session_created_response\b/,
        message: 'core remote-connect server must not own session-created response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_session_model_updated_response\b/,
        message: 'core remote-connect server must not own session-model response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_messages_response\b/,
        message: 'core remote-connect server must not own remote messages response assembly; use the integrations helper',
      },
      {
        regex: /\bfn remote_session_deleted_response\b/,
        message: 'core remote-connect server must not own session-deleted response assembly; use the integrations helper',
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
    path: 'src/crates/core/src',
    reason:
      'core must use runtime-ports as the owner path for portable subagent contracts',
    patterns: [
      {
        regex:
          /crate::agentic::subagent_runtime(?:::|\s*::|::\{)(?:[^;\n]*\b(?:DelegationPolicy|SubagentContextMode)\b)/,
        message:
          'DelegationPolicy and SubagentContextMode must be imported from bitfun-runtime-ports, not the core compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src',
    reason:
      'product-domains must not own IO/process/Git/AI/platform runtime behavior without an approved port/provider migration',
    patterns: [
      {
        regex: /\bCommand::new\(/,
        allowPaths: ['src/crates/product-domains/src/miniapp/runtime.rs'],
        message:
          'product-domains must not spawn processes outside the reviewed MiniApp runtime detector owner',
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
  {
    path: 'src/crates/core/src/agentic/tools/implementations',
    reason:
      'GetToolSpec concrete adapter belongs in the product tool runtime owner, not the generic concrete-tool implementations module',
    patterns: [
      {
        regex: /\bpub(?:\(crate\))? struct GetToolSpecTool\b/,
        message: 'move GetToolSpecTool into core product_runtime owner',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/pipeline',
    reason:
      'core pipeline must delegate deterministic tool execution admission policy to bitfun-agent-tools',
    patterns: [
      {
        regex: /\bvalidate_tool_allowed_by_list\s*\(/,
        message: 'allowed-list admission must stay behind validate_tool_execution_admission',
      },
      {
        regex: /\bruntime_tool_restrictions\s*\.\s*ensure_tool_allowed\s*\(/,
        message: 'runtime-restriction admission must stay behind validate_tool_execution_admission',
      },
      {
        regex: /\bvalidate_collapsed_tool_usage\s*\(/,
        message: 'collapsed-tool admission must stay behind validate_tool_execution_admission',
      },
    ],
  },
];

const requiredContentRules = [
  {
    path: 'src/crates/services-core/src/filesystem/mod.rs',
    reason:
      'services-core filesystem owner must expose local filesystem primitives behind a single module boundary',
    patterns: [
      {
        regex: /mod error;/,
        message: 'filesystem owner must expose its error boundary',
      },
      {
        regex: /mod operations;/,
        message: 'filesystem owner must expose local file operation primitives',
      },
      {
        regex: /mod tree;/,
        message: 'filesystem owner must expose local file tree/search primitives',
      },
      {
        regex: /pub use error::\{FileSystemError, FileSystemResult\};/,
        message: 'filesystem owner must re-export the unified filesystem error type',
      },
      {
        regex: /pub use service::FileSystemService;/,
        message: 'filesystem owner must keep the consolidated service facade',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/filesystem/service.rs',
    reason:
      'core filesystem service may keep remote-workspace overlay and BitFunError compatibility, but local filesystem owner must remain services-core',
    patterns: [
      {
        regex: /lookup_remote_connection_with_hint/,
        message: 'core filesystem wrapper must preserve remote workspace connection disambiguation',
      },
      {
        regex: /get_remote_workspace_manager/,
        message: 'core filesystem wrapper must preserve existing remote file service lookup',
      },
      {
        regex: /map_filesystem_error/,
        message: 'core filesystem wrapper must map services-core errors at the compatibility boundary',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/session/session_manager.rs',
    reason:
      'core session manager must keep forked Task prompt-cache and existing-context turn baselines until session branch ownership migrates',
    patterns: [
      {
        regex: /\bpub async fn clone_prompt_cache\b/,
        message: 'missing prompt cache clone runtime entry point',
      },
      {
        regex: /\bpub async fn start_dialog_turn_with_existing_context\b/,
        message: 'missing existing-context dialog turn entry point',
      },
      {
        regex: /\bstart_dialog_turn_with_existing_context_persists_turn_and_snapshot\b/,
        message: 'missing existing-context dialog turn persistence regression',
      },
      {
        regex: /\bclone_prompt_cache_copies_runtime_and_persisted_entries\b/,
        message: 'missing prompt cache clone runtime/disk regression',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs',
    reason:
      'core tool pipeline must keep latest-main truncation and per-tool denial behavior until tool runtime ownership migrates',
    patterns: [
      {
        regex: /\bfn build_truncation_recovery_notice\b/,
        message: 'missing tool-call truncation recovery notice helper',
      },
      {
        regex: /\btruncation_notice_for_interactive_tools_does_not_claim_file_write\b/,
        message: 'missing interactive-tool truncation recovery regression',
      },
      {
        regex: /\btruncation_notice_for_write_tools_keeps_write_continuation_guidance\b/,
        message: 'missing write-tool truncation recovery regression',
      },
      {
        regex: /\bdenied_tool_messages\b/,
        message: 'missing per-tool denial message propagation',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/restrictions.rs',
    reason:
      'core tool restrictions facade must preserve per-tool denial messages while runtime restrictions live in agent-tools',
    patterns: [
      {
        regex: /\bdenied_tool_messages\b/,
        message: 'missing per-tool denial message field propagation',
      },
      {
        regex: /\bcustom_deny_message_overrides_generic_runtime_error\b/,
        message: 'missing custom deny message regression',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/tool_result_storage.rs',
    reason:
      'core tool-result storage must keep explicit file flush until runtime artifact ownership migrates',
    patterns: [
      {
        regex: /\basync fn write_once\b/,
        message: 'missing single-write persistence helper',
      },
      {
        regex: /file\.flush\(\)\.await/,
        message: 'missing explicit persisted tool-result flush',
      },
    ],
  },
  {
    path: 'src/crates/services-integrations/src/mcp/server/connection.rs',
    reason:
      'services-integrations MCP connection must keep initialize-scoped timeout and channel-close cleanup until MCP owner migration is reviewed',
    patterns: [
      {
        regex: /\bsend_request_with_id\b/,
        message: 'missing stable local JSON-RPC request id path',
      },
      {
        regex: /\binitialize_timeout\b/,
        message: 'missing initialize-scoped timeout',
      },
      {
        regex: /notifications\/initialized/,
        message: 'missing MCP initialized notification',
      },
      {
        regex: /\bpending\.clear\(\)/,
        message: 'missing pending request waiter drain on channel close',
      },
      {
        regex: /\blocal_tool_calls_do_not_inherit_initialize_timeout\b/,
        message: 'missing local tool request timeout-scope regression',
      },
      {
        regex: /\blocal_initialize_uses_initialize_timeout\b/,
        message: 'missing local initialize timeout regression',
      },
    ],
  },
  {
    path: 'src/crates/services-integrations/src/mcp/protocol/transport.rs',
    reason:
      'services-integrations MCP local transport must keep explicit request ids and stdin flush semantics',
    patterns: [
      {
        regex: /\bpub async fn send_request_with_id\b/,
        message: 'missing explicit JSON-RPC request id send path',
      },
      {
        regex: /\.flush\(\)\s*\.await/,
        message: 'missing local MCP stdin flush',
      },
    ],
  },
  {
    path: 'src/crates/core/Cargo.toml',
    reason:
      'bitfun-core product-full must explicitly aggregate owner crate feature groups instead of forcing them through dependency declarations',
    patterns: [
      {
        regex:
          /bitfun-tool-packs = \{ path = "\.\.\/tool-packs", default-features = false, optional = true \}/,
        message: 'bitfun-tool-packs dependency must stay optional and not force product-full outside the core feature graph',
      },
      {
        regex:
          /bitfun-services-integrations = \{ path = "\.\.\/services-integrations", default-features = false, features = \["remote-ssh"\] \}/,
        message:
          'bitfun-services-integrations dependency may keep remote workspace identity helpers but must not force product-full outside the core feature graph',
      },
      {
        regex:
          /bitfun-product-domains = \{ path = "\.\.\/product-domains", default-features = false, optional = true \}/,
        message:
          'bitfun-product-domains dependency must stay optional and not force product-full outside the core feature graph',
      },
      {
        regex: /"dep:bitfun-tool-packs"/,
        message: 'core tool-packs feature must explicitly enable the optional dependency',
      },
      {
        regex: /"bitfun-tool-packs\/product-full"/,
        message: 'core product-full must explicitly enable tool pack product features',
      },
      {
        regex: /"bitfun-services-integrations\/product-full"/,
        message: 'core product-full must explicitly enable integration product features',
      },
      {
        regex: /"dep:bitfun-product-domains"/,
        message: 'core product-domains feature must explicitly enable the optional dependency',
      },
      {
        regex: /"bitfun-product-domains\/product-full"/,
        message: 'core product-full must explicitly enable product-domain features',
      },
    ],
  },
  {
    path: 'src/crates/core/src/lib.rs',
    reason:
      'no-default bitfun-core must keep product runtime surfaces behind explicit features',
    patterns: [
      {
        regex: /#\[cfg\(feature = "product-full"\)\]\s*pub mod agentic\b/s,
        message: 'agentic runtime must stay behind product-full for no-default builds',
      },
      {
        regex: /#\[cfg\(feature = "product-domains"\)\]\s*pub mod function_agents\b/s,
        message: 'function-agent product domain facade must stay behind product-domains',
      },
      {
        regex: /#\[cfg\(feature = "product-domains"\)\]\s*pub mod miniapp\b/s,
        message: 'MiniApp product domain facade must stay behind product-domains',
      },
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*pub\(crate\) mod service_agent_runtime\b/s,
        message: 'service agent runtime owner assembly must stay behind service-integrations',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/mod.rs',
    reason:
      'service integration and agent-runtime surfaces must not compile in no-default core builds',
    patterns: [
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*pub mod git\b/s,
        message: 'git service facade must stay behind service-integrations',
      },
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*pub mod mcp\b/s,
        message: 'MCP service facade must stay behind service-integrations',
      },
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*pub mod remote_connect\b/s,
        message: 'remote-connect service facade must stay behind service-integrations',
      },
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*pub mod review_platform\b/s,
        message: 'review platform facade must stay behind service-integrations',
      },
      {
        regex: /#\[cfg\(feature = "product-full"\)\]\s*pub mod snapshot\b/s,
        message: 'snapshot service must stay behind product-full until tool-runtime ownership is split',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/config/mod.rs',
    reason:
      'mode config canonicalization depends on product agent/tool registries and must stay out of no-default builds',
    patterns: [
      {
        regex: /#\[cfg\(feature = "product-full"\)\]\s*pub mod mode_config_canonicalizer\b/s,
        message: 'mode config canonicalizer must stay behind product-full',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/workspace/manager.rs',
    reason:
      'workspace metadata may omit git worktree enrichment when service integrations are disabled',
    patterns: [
      {
        regex: /#\[cfg\(feature = "service-integrations"\)\]\s*use crate::service::git::GitService\b/s,
        message: 'GitService import must stay gated for no-default builds',
      },
      {
        regex: /#\[cfg\(not\(feature = "service-integrations"\)\)\]\s*\{\s*let _ = workspace_root;\s*return None;\s*\}/s,
        message: 'no-default worktree enrichment fallback must remain explicit',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/workspace_runtime/service.rs',
    reason:
      'workspace runtime binding helpers may depend on agentic runtime only in full product builds',
    patterns: [
      {
        regex: /#\[cfg\(feature = "product-full"\)\]\s*use crate::agentic::WorkspaceBinding\b/s,
        message: 'WorkspaceBinding import must stay gated for no-default builds',
      },
      {
        regex: /#\[cfg\(feature = "product-full"\)\]\s*pub async fn ensure_runtime_for_workspace_binding\b/s,
        message: 'WorkspaceBinding runtime helper must stay behind product-full',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/remote_ssh/mod.rs',
    reason:
      'core remote SSH runtime must keep concrete SSH dependencies behind the ssh-remote feature while preserving lightweight workspace identity helpers',
    patterns: [
      {
        regex: /#\[cfg\(not\(feature = "ssh-remote"\)\)\]\s*mod disabled\b/s,
        message: 'missing disabled remote SSH runtime surface for no-default builds',
      },
      {
        regex: /#\[cfg\(feature = "ssh-remote"\)\]\s*pub mod manager\b/s,
        message: 'remote SSH manager must stay gated behind ssh-remote',
      },
      {
        regex: /#\[cfg\(feature = "ssh-remote"\)\]\s*pub mod remote_fs\b/s,
        message: 'remote SSH filesystem runtime must stay gated behind ssh-remote',
      },
      {
        regex: /#\[cfg\(feature = "ssh-remote"\)\]\s*pub mod remote_terminal\b/s,
        message: 'remote SSH terminal runtime must stay gated behind ssh-remote',
      },
      {
        regex: /\bpub mod workspace_state\b/,
        message: 'remote workspace identity helpers must remain available without ssh-remote',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/remote_ssh/disabled.rs',
    reason:
      'no-default core builds must expose explicit unsupported remote SSH stubs instead of compiling russh-backed runtime code',
    patterns: [
      {
        regex: /Remote SSH support is disabled; enable the `ssh-remote` feature/,
        message: 'missing explicit disabled remote SSH diagnostic',
      },
      {
        regex: /\bpub struct SSHConnectionManager\b/,
        message: 'missing disabled SSH manager compatibility surface',
      },
      {
        regex: /\bpub struct RemoteFileService\b/,
        message: 'missing disabled remote file compatibility surface',
      },
      {
        regex: /\bpub struct RemoteTerminalManager\b/,
        message: 'missing disabled remote terminal compatibility surface',
      },
    ],
  },
  {
    path: 'src/crates/runtime-ports/src/lib.rs',
    reason:
      'runtime-ports must keep remote and subagent runtime boundary contracts DTO/trait-only',
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
      {
        regex: /\bpub type DialogTriggerSource = AgentSubmissionSource\b/,
        message: 'missing dialog trigger source compatibility contract',
      },
      {
        regex: /\bdialog_trigger_source_reuses_agent_submission_source_contract\b/,
        message: 'missing dialog trigger source alias regression',
      },
      {
        regex: /\bpub enum DialogQueuePriority\b/,
        message: 'missing dialog queue priority contract',
      },
      {
        regex: /\bpub struct DialogSubmissionPolicy\b/,
        message: 'missing dialog submission policy contract',
      },
      {
        regex: /\bdialog_submission_policy_preserves_current_surface_queue_defaults\b/,
        message: 'missing dialog submission policy regression',
      },
      {
        regex: /\bpub enum DialogSubmitOutcome\b/,
        message: 'missing dialog submit outcome contract',
      },
      {
        regex: /\bdialog_submit_outcome_preserves_started_and_queued_fields\b/,
        message: 'missing dialog submit outcome regression',
      },
      {
        regex: /\bpub enum DialogSessionStateFact\b/,
        message: 'missing dialog session state fact contract',
      },
      {
        regex: /\bpub struct DialogSubmitQueueFacts\b/,
        message: 'missing dialog submit queue facts contract',
      },
      {
        regex: /\bpub enum DialogSubmitQueueAction\b/,
        message: 'missing dialog submit queue action contract',
      },
      {
        regex: /\bpub const fn dialog_policy_may_preempt\b/,
        message: 'missing dialog preempt policy contract',
      },
      {
        regex: /\bpub const fn resolve_dialog_submit_queue_action\b/,
        message: 'missing dialog submit queue action resolver',
      },
      {
        regex: /\bdialog_submit_queue_action_preserves_current_scheduler_routing_policy\b/,
        message: 'missing dialog submit queue action regression',
      },
      {
        regex: /\bpub fn should_suppress_agent_session_cancelled_reply\b/,
        message: 'missing agent-session cancel suppression contract',
      },
      {
        regex: /\bpub enum DialogTurnOutcomeKind\b/,
        message: 'missing dialog turn outcome kind contract',
      },
      {
        regex: /\bpub const fn should_skip_agent_session_reply\b/,
        message: 'missing agent-session reply skip contract',
      },
      {
        regex: /\bagent_session_reply_decisions_preserve_cancel_suppression_boundary\b/,
        message: 'missing agent-session reply decision regression',
      },
      {
        regex: /\bpub struct AgentSessionReplyRoute\b/,
        message: 'missing agent session reply route contract',
      },
      {
        regex: /\bagent_session_reply_route_keeps_requester_fields\b/,
        message: 'missing agent session reply route regression',
      },
      {
        regex: /\bpub enum DialogSteerOutcome\b/,
        message: 'missing dialog steer outcome contract',
      },
      {
        regex: /\bdialog_steer_outcome_preserves_buffered_fields\b/,
        message: 'missing dialog steer outcome regression',
      },
      {
        regex: /\bpub enum RoundInjectionKind\b/,
        message: 'missing round injection kind contract',
      },
      {
        regex: /\bpub enum RoundInjectionTarget\b/,
        message: 'missing round injection target contract',
      },
      {
        regex: /\bpub struct RoundInjection\b/,
        message: 'missing round injection message contract',
      },
      {
        regex: /\bpub trait DialogRoundPreemptSource\b/,
        message: 'missing dialog round preempt source contract',
      },
      {
        regex: /\bpub trait DialogRoundInjectionSource\b/,
        message: 'missing dialog round injection source contract',
      },
      {
        regex: /\bround_injection_contract_keeps_kind_and_target_identity\b/,
        message: 'missing round injection contract regression',
      },
      {
        regex: /\bround_injection_source_contract_drains_portable_injections\b/,
        message: 'missing round injection source contract regression',
      },
      {
        regex: /\bpub enum ThreadGoalStatus\b/,
        message: 'missing thread goal status contract',
      },
      {
        regex: /\bpub struct ThreadGoal\b/,
        message: 'missing thread goal contract',
      },
      {
        regex: /\bpub struct SetThreadGoalResult\b/,
        message: 'missing set thread goal result contract',
      },
      {
        regex: /\bpub struct ThreadGoalContinuationPlan\b/,
        message: 'missing thread goal continuation plan contract',
      },
      {
        regex: /\bpub struct ThreadGoalToolResponse\b/,
        message: 'missing thread goal tool response contract',
      },
      {
        regex: /\bthread_goal_active_status_includes_budget_limited\b/,
        message: 'missing thread goal status contract regression',
      },
      {
        regex: /\bthread_goal_tool_response_serializes_optional_fields\b/,
        message: 'missing thread goal tool response wire-shape regression',
      },
      {
        regex: /\bpub struct CompressionContract\b/,
        message: 'missing compression contract',
      },
      {
        regex: /\bpub struct CompressionContractItem\b/,
        message: 'missing compression contract item',
      },
      {
        regex: /\bcompression_contract_renders_model_visible_fields\b/,
        message: 'missing compression contract rendering regression',
      },
      {
        regex: /\bpub struct RelatedPath\b/,
        message: 'missing related path request-context contract',
      },
      {
        regex: /\brelated_path_serializes_as_request_context_fact\b/,
        message: 'missing related path serialization regression',
      },
      {
        regex: /\bpub struct DelegationPolicy\b/,
        message: 'missing delegation policy contract',
      },
      {
        regex: /\bpub enum SubagentContextMode\b/,
        message: 'missing subagent context mode contract',
      },
      {
        regex: /\bdelegation_policy_child_blocks_recursive_spawn_without_losing_depth\b/,
        message: 'missing delegation policy contract regression',
      },
      {
        regex: /\bsubagent_context_mode_preserves_fork_wire_value\b/,
        message: 'missing subagent context mode contract regression',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/subagent_runtime/mod.rs',
    reason:
      'core subagent runtime must preserve legacy import path while runtime-ports owns portable subagent contracts',
    patterns: [
      {
        regex: /pub\(crate\) use bitfun_runtime_ports::\{DelegationPolicy, SubagentContextMode\};/,
        message: 'missing core compatibility re-export for subagent runtime contracts',
      },
      {
        regex: /pub\(crate\) mod queue_timing;/,
        message: 'queue timing must remain core-owned until it has a reviewed non-DTO owner',
      },
    ],
  },
  {
    path: 'src/crates/agent-tools/src/framework.rs',
    reason:
      'agent-tools may own pure and generic prompt-visible tool contracts and provider-neutral execution gate policy without owning product registry or concrete execution',
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
        regex: /\bpub enum CollapsedToolUsageError\b/,
        message: 'missing collapsed-tool execution gate error contract',
      },
      {
        regex: /\bpub enum ToolExecutionAccessError\b/,
        message: 'missing tool execution allowed-list gate error contract',
      },
      {
        regex: /\bpub fn validate_tool_allowed_by_list\b/,
        message: 'missing tool execution allowed-list gate policy',
      },
      {
        regex: /\bpub fn validate_collapsed_tool_usage\b/,
        message: 'missing collapsed-tool execution gate policy',
      },
      {
        regex: /\bpub fn is_tool_path_allowed_by_resolved_roots\b/,
        message: 'missing provider-neutral path policy root matcher',
      },
      {
        regex: /\bpub fn build_tool_path_policy_denial_message\b/,
        message: 'missing provider-neutral path policy denial message',
      },
      {
        regex: /\bpub fn resolve_tool_path_with_context\b/,
        message: 'missing provider-neutral tool path resolution owner',
      },
      {
        regex: /\bpub fn tool_path_is_effectively_absolute\b/,
        message: 'missing provider-neutral tool path absolute check',
      },
      {
        regex: /\bpub fn build_tool_runtime_artifact_reference\b/,
        message: 'missing provider-neutral runtime artifact reference builder',
      },
      {
        regex: /\bpub fn build_tool_session_runtime_artifact_reference\b/,
        message: 'missing provider-neutral session runtime artifact reference builder',
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
    path: 'src/crates/agent-tools/src/file_guidance.rs',
    reason: 'agent-tools owns provider-neutral file tool guidance marker contracts',
    patterns: [
      {
        regex: /\bpub const FILE_TOOL_GUIDANCE_PREFIX\b/,
        message: 'missing file tool guidance marker prefix',
      },
      {
        regex: /\bpub fn file_tool_guidance_message\b/,
        message: 'missing file tool guidance message helper',
      },
      {
        regex: /\bpub fn is_file_tool_guidance_message\b/,
        message: 'missing file tool guidance classifier',
      },
    ],
  },
  {
    path: 'src/crates/agent-tools/src/file_read_freshness.rs',
    reason: 'agent-tools owns pure file-read freshness policy for Read/Edit/Write guardrails',
    patterns: [
      {
        regex: /\bpub struct FileReadFreshnessFacts\b/,
        message: 'missing file-read freshness facts contract',
      },
      {
        regex: /\bpub fn normalize_tool_file_content\b/,
        message: 'missing provider-neutral file content normalization helper',
      },
      {
        regex: /\bpub fn file_read_facts_content_matches\b/,
        message: 'missing file-read content equivalence helper',
      },
      {
        regex: /\bpub fn file_read_facts_are_fresh\b/,
        message: 'missing file-read freshness policy helper',
      },
    ],
  },
  {
    path: 'src/crates/agent-tools/src/tool_result_storage.rs',
    reason:
      'agent-tools owns pure oversized tool-result storage policy and rendering without session IO',
    patterns: [
      {
        regex: /\bpub struct ToolResultStoragePolicy\b/,
        message: 'missing provider-neutral tool result storage policy',
      },
      {
        regex: /\bpub struct PersistedToolOutput\b/,
        message: 'missing persisted tool output render contract',
      },
      {
        regex: /\bpub struct ToolResultPersistenceCandidate\b/,
        message: 'missing provider-neutral persistence candidate contract',
      },
      {
        regex: /\bpub fn select_tool_result_indices_for_persistence\b/,
        message: 'missing round-budget persistence candidate selector',
      },
      {
        regex: /\bpub fn sanitize_tool_result_file_component\b/,
        message: 'missing tool-result file component sanitizer',
      },
      {
        regex: /\bpub fn generate_tool_result_preview\b/,
        message: 'missing tool-result preview generator',
      },
      {
        regex: /\bpub fn count_tool_result_lines\b/,
        message: 'missing tool-result line counter',
      },
      {
        regex: /\bpub fn build_persisted_tool_output_message\b/,
        message: 'missing persisted-output message renderer',
      },
      {
        regex: /\bpub fn tool_result_is_persisted_output\b/,
        message: 'missing persisted-output classifier',
      },
    ],
  },
  {
    path: 'src/crates/agent-tools/src/tool_execution_presentation.rs',
    reason:
      'agent-tools owns provider-neutral tool execution result and error presentation helpers',
    patterns: [
      {
        regex: /\bpub const TOOL_ERROR_ARGUMENTS_PREVIEW_BYTES\b/,
        message: 'missing tool error argument preview limit contract',
      },
      {
        regex: /\bpub const USER_STEERING_INTERRUPTED_MESSAGE\b/,
        message: 'missing steering-interrupted assistant message contract',
      },
      {
        regex: /\bpub struct ToolExecutionErrorPresentation\b/,
        message: 'missing tool execution error presentation DTO',
      },
      {
        regex: /\bpub fn render_tool_result_for_assistant\b/,
        message: 'missing tool result assistant rendering helper',
      },
      {
        regex: /\bpub fn truncate_tool_arguments_preview\b/,
        message: 'missing structured tool argument preview helper',
      },
      {
        regex: /\bpub fn truncate_raw_tool_arguments_preview\b/,
        message: 'missing raw tool argument preview helper',
      },
      {
        regex: /\bpub fn build_tool_execution_error_presentation\b/,
        message: 'missing tool execution error presentation helper',
      },
      {
        regex: /\bpub fn build_user_steering_interrupted_presentation\b/,
        message: 'missing steering-interrupted presentation helper',
      },
      {
        regex: /\bpub fn build_invalid_tool_call_error_message\b/,
        message: 'missing invalid tool call error message helper',
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
      {
        regex: /pub use bitfun_runtime_ports::DialogTriggerSource;/,
        message: 'missing dialog trigger source compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/coordination/scheduler.rs',
    reason:
      'core scheduler must preserve legacy submission policy import path while runtime-ports owns portable dialog policy contracts',
    patterns: [
      {
        regex:
          /pub use bitfun_runtime_ports::\{[\s\S]*AgentSessionReplyRoute[\s\S]*DialogQueuePriority[\s\S]*DialogSteerOutcome[\s\S]*DialogSubmissionPolicy[\s\S]*DialogSubmitOutcome[\s\S]*\};/,
        message: 'missing dialog submission policy compatibility re-export',
      },
      {
        regex:
          /use bitfun_runtime_ports::\{(?=[\s\S]*DialogSessionStateFact)(?=[\s\S]*DialogSubmitQueueAction)(?=[\s\S]*DialogSubmitQueueFacts)(?=[\s\S]*DialogTurnOutcomeKind)(?=[\s\S]*resolve_dialog_submit_queue_action)(?=[\s\S]*should_skip_agent_session_reply_contract)(?=[\s\S]*should_suppress_agent_session_cancelled_reply_contract)[\s\S]*\};/,
        message: 'missing dialog scheduler decision contract import',
      },
      {
        regex:
          /use bitfun_agent_runtime::scheduler::\{(?=[\s\S]*BackgroundDeliveryAction)(?=[\s\S]*BackgroundDeliveryFacts)(?=[\s\S]*resolve_background_delivery_action)[\s\S]*\};/,
        message: 'missing agent-runtime background delivery decision import',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/round_preempt.rs',
    reason:
      'core round preempt runtime must preserve legacy injection import path while runtime-ports owns portable injection contracts',
    patterns: [
      {
        regex:
          /pub use bitfun_runtime_ports::\{[\s\S]*DialogRoundInjectionSource[\s\S]*DialogRoundPreemptSource[\s\S]*RoundInjection[\s\S]*RoundInjectionKind[\s\S]*RoundInjectionTarget[\s\S]*\};/,
        message: 'missing round injection compatibility re-export',
      },
      {
        regex: /\bpub struct SessionRoundInjectionBuffer\b/,
        message: 'round injection buffer must remain core-owned until concrete runtime migration',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/goal_mode/mod.rs',
    reason:
      'core goal mode types must preserve legacy import path while runtime-ports owns portable goal contracts',
    patterns: [
      {
        regex:
          /pub use bitfun_runtime_ports::\{[\s\S]*SetThreadGoalResult[\s\S]*ThreadGoal[\s\S]*ThreadGoalContinuationPlan[\s\S]*ThreadGoalStatus[\s\S]*ThreadGoalToolResponse[\s\S]*GOAL_MODE_METADATA_KEY[\s\S]*MAX_CONTEXT_SUMMARY_CHARS[\s\S]*MAX_THREAD_GOAL_OBJECTIVE_CHARS[\s\S]*THREAD_GOAL_METADATA_KEY[\s\S]*\};/,
        message: 'missing thread goal compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/core/message.rs',
    reason:
      'core message model must preserve legacy compression contract import path while runtime-ports owns portable compaction facts',
    patterns: [
      {
        regex: /pub use bitfun_runtime_ports::\{CompressionContract, CompressionContractItem\};/,
        message: 'missing compression contract compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/workspace/manager.rs',
    reason:
      'core workspace manager must preserve legacy related-path import path while runtime-ports owns portable request-context facts',
    patterns: [
      {
        regex: /pub use bitfun_runtime_ports::RelatedPath;/,
        message: 'missing related path compatibility re-export',
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
        regex: /\bfn remote_cancel_host\b/,
        message: 'missing remote cancel host owner factory',
      },
      {
        regex: /\bfn remote_image_context\b/,
        message: 'missing remote image context owner adapter',
      },
      {
        regex: /\bfn load_remote_model_catalog\b/,
        message: 'missing remote model catalog owner adapter',
      },
      {
        regex: /\bRemoteModelCatalogFacts\b/,
        message: 'missing remote model catalog fact projection',
      },
      {
        regex: /\bRemoteModelCapabilityFact\b/,
        message: 'missing remote model capability fact projection',
      },
      {
        regex: /\bRemoteReasoningModeFact\b/,
        message: 'missing remote reasoning mode fact projection',
      },
      {
        regex: /\bbuild_remote_model_catalog\b/,
        message: 'missing remote model catalog assembly delegation',
      },
      {
        regex: /\bfn update_remote_session_model\b/,
        message: 'missing remote session model update owner adapter',
      },
      {
        regex: /\bfn normalize_remote_session_model_id\b/,
        message: 'missing remote session model id normalization regression hook',
      },
      {
        regex: /\bnormalize_remote_session_model_id_contract\b/,
        message: 'missing remote session model id owner delegation',
      },
      {
        regex: /\bfn normalize_remote_model_selection\b/,
        message: 'missing remote model selection normalization regression hook',
      },
      {
        regex: /\bnormalize_remote_model_selection_contract\b/,
        message: 'missing remote model selection owner delegation',
      },
      {
        regex: /\bfn remote_chat_messages_from_turns\b/,
        message: 'missing remote chat history conversion owner adapter',
      },
      {
        regex: /\bRemoteDialogSchedulerOutcomeFact\b/,
        message: 'missing remote dialog scheduler outcome fact projection',
      },
      {
        regex: /\bremote_dialog_submit_outcome_from_scheduler\b/,
        message: 'missing remote dialog submit outcome assembly delegation',
      },
      {
        regex: /\bRemoteChatHistoryTurn\b/,
        message: 'missing remote chat history owner DTO projection',
      },
      {
        regex: /\bbuild_remote_chat_messages\b/,
        message: 'missing remote chat history assembly delegation',
      },
      {
        regex: /\bfn strip_remote_user_input_tags\b/,
        message: 'missing remote user input display cleanup owner adapter',
      },
      {
        regex: /\bfn compress_remote_chat_data_url_for_mobile\b/,
        message: 'missing remote chat thumbnail compression owner adapter',
      },
      {
        regex: /\bfn load_remote_chat_messages\b/,
        message: 'missing remote chat history persistence owner adapter',
      },
      {
        regex: /\bfn agent_submission_port\b/,
        message: 'missing agent submission port owner binding',
      },
      {
        regex: /\bfn agent_turn_cancellation_port\b/,
        message: 'missing agent turn cancellation port owner binding',
      },
      {
        regex: /\bfn remote_control_state_port\b/,
        message: 'missing remote control state port owner binding',
      },
      {
        regex: /\bCoreRemoteDialogRuntimeHost\b/,
        message: 'missing core remote dialog host binding',
      },
      {
        regex: /\bCoreRemoteCancelRuntimeHost\b/,
        message: 'missing core remote cancel host binding',
      },
      {
        regex: /\bCoreRemoteWorkspaceFileRuntimeHost\b/,
        message: 'missing core remote workspace file host binding',
      },
      {
        regex: /\bCoreRemoteSessionTrackerHost\b/,
        message: 'missing core remote session tracker host binding',
      },
      {
        regex: /\bRemoteExecutionDispatcher\b/,
        message: 'missing remote execution dispatcher binding',
      },
      {
        regex: /\bimpl RemoteDialogRuntimeHost for CoreRemoteDialogRuntimeHost\b/,
        message: 'missing remote dialog host adapter implementation in runtime owner',
      },
      {
        regex: /\bimpl RemoteCancelRuntimeHost for CoreRemoteCancelRuntimeHost\b/,
        message: 'missing remote cancel host adapter implementation in runtime owner',
      },
      {
        regex: /\bimpl RemoteWorkspaceFileRuntimeHost for CoreRemoteWorkspaceFileRuntimeHost\b/,
        message: 'missing remote workspace file host adapter implementation in runtime owner',
      },
      {
        regex: /\bimpl RemoteSessionTrackerHost for CoreRemoteSessionTrackerHost\b/,
        message: 'missing remote tracker host adapter implementation in runtime owner',
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
      {
        regex: /\bcore_service_agent_runtime_owner_normalizes_remote_session_model_ids\b/,
        message: 'missing remote session model id normalization regression',
      },
      {
        regex: /\bcore_service_agent_runtime_owner_normalizes_remote_model_selection_aliases\b/,
        message: 'missing remote model selection alias regression',
      },
      {
        regex: /\bcore_service_agent_runtime_owner_preserves_remote_chat_history_shape\b/,
        message: 'missing remote chat history conversion regression',
      },
      {
        regex: /\bcore_service_agent_runtime_owner_skips_in_progress_remote_assistant_history\b/,
        message: 'missing in-progress remote assistant history regression',
      },
    ],
  },
  {
    path: 'src/crates/services-integrations/src/remote_connect.rs',
    reason:
      'services-integrations must own remote-connect wire/response assembly and preserve remote owner compatibility re-exports',
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
        regex: /\bpub struct RemoteCancelTaskRequest\b/,
        message: 'missing remote cancel task request contract',
      },
      {
        regex: /\bpub trait RemoteCancelRuntimeHost\b/,
        message: 'missing remote cancel runtime host port',
      },
      {
        regex: /\bpub async fn cancel_remote_task\b/,
        message: 'missing remote cancel orchestration owner',
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
        regex: /\bpub struct RemoteChatHistoryTurn\b/,
        message: 'missing remote chat history turn DTO',
      },
      {
        regex: /\bpub struct RemoteChatHistoryRound\b/,
        message: 'missing remote chat history round DTO',
      },
      {
        regex: /\bpub struct RemoteChatHistoryToolItem\b/,
        message: 'missing remote chat history tool item DTO',
      },
      {
        regex: /\bpub fn build_remote_chat_messages\b/,
        message: 'missing remote chat history assembly owner',
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
        regex: /\bRemoteWorkspaceFileRuntimeHost\b/,
        message: 'missing remote workspace file runtime host contract',
      },
      {
        regex: /\bpub async fn handle_remote_workspace_file_command\b/,
        message: 'missing remote workspace file command owner handler',
      },
      {
        regex: /\bpub fn remote_file_content_response\b/,
        message: 'missing remote file content response assembly helper',
      },
      {
        regex: /\bpub fn remote_file_chunk_response\b/,
        message: 'missing remote file chunk response assembly helper',
      },
      {
        regex: /\bpub fn remote_file_info_response\b/,
        message: 'missing remote file-info response assembly helper',
      },
      {
        regex: /\bpub fn remote_dialog_submit_response\b/,
        message: 'missing remote dialog response assembly helper',
      },
      {
        regex: /\bpub fn remote_task_cancel_response\b/,
        message: 'missing remote task cancel response assembly helper',
      },
      {
        regex: /\bpub fn remote_interaction_accepted_response\b/,
        message: 'missing remote interaction response assembly helper',
      },
      {
        regex: /\bpub fn remote_answer_question_response\b/,
        message: 'missing remote answer response assembly helper',
      },
      {
        regex: /\bRemoteWorkspaceFacts\b/,
        message: 'missing remote workspace response facts DTO',
      },
      {
        regex: /\bRemoteSessionMetadata\b/,
        message: 'missing remote session response metadata DTO',
      },
      {
        regex: /\bpub fn remote_workspace_info_response\b/,
        message: 'missing remote workspace-info response assembly helper',
      },
      {
        regex: /\bpub fn remote_recent_workspaces_response\b/,
        message: 'missing remote recent-workspaces response assembly helper',
      },
      {
        regex: /\bpub fn remote_assistant_list_response\b/,
        message: 'missing remote assistant-list response assembly helper',
      },
      {
        regex: /\bpub fn remote_session_info\b/,
        message: 'missing remote session response facts helper',
      },
      {
        regex: /\bpub fn remote_session_list_response\b/,
        message: 'missing remote session-list response assembly helper',
      },
      {
        regex: /\bpub fn remote_initial_sync_response\b/,
        message: 'missing remote initial-sync response assembly helper',
      },
      {
        regex: /\bpub fn remote_messages_response\b/,
        message: 'missing remote messages response assembly helper',
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
        regex: /\bpub enum RemoteModelCapabilityFact\b/,
        message: 'missing remote model capability owner fact',
      },
      {
        regex: /\bpub enum RemoteReasoningModeFact\b/,
        message: 'missing remote reasoning mode owner fact',
      },
      {
        regex: /\bpub struct RemoteModelFacts\b/,
        message: 'missing remote model owner facts',
      },
      {
        regex: /\bpub struct RemoteModelCatalogFacts\b/,
        message: 'missing remote model catalog owner facts',
      },
      {
        regex: /\bpub fn build_remote_model_catalog\b/,
        message: 'missing remote model catalog assembly owner',
      },
      {
        regex: /\bpub struct RemoteModelCatalogPollDelta\b/,
        message: 'missing remote model catalog poll delta',
      },
      {
        regex: /\bpub fn normalize_remote_session_model_id\b/,
        message: 'missing remote session model normalization policy',
      },
      {
        regex: /\bpub fn normalize_remote_model_selection\b/,
        message: 'missing remote model selection policy',
      },
      {
        regex: /\bpub fn remote_model_selection_needs_config\b/,
        message: 'missing remote model selection config-gate policy',
      },
      {
        regex: /\bpub enum RemoteCommand\b/,
        message: 'missing remote command wire contract',
      },
      {
        regex: /\bpub enum RemoteDialogSchedulerOutcomeFact\b/,
        message: 'missing remote dialog scheduler outcome fact',
      },
      {
        regex: /\bpub fn remote_dialog_submit_outcome_from_scheduler\b/,
        message: 'missing remote dialog submit outcome assembly owner',
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
        regex: /\bremote_connect_model_catalog_builder_preserves_config_shape\b/,
        message: 'missing remote model catalog builder contract test',
      },
      {
        regex: /\bremote_connect_model_selection_policy_owns_alias_and_config_reference_rules\b/,
        message: 'missing remote model selection policy contract test',
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
        regex: /\bremote_connect_dialog_submit_outcome_builder_preserves_scheduler_shape\b/,
        message: 'missing remote dialog outcome builder contract test',
      },
      {
        regex: /\bremote_connect_dialog_runtime_keeps_legacy_restore_failure_tolerance\b/,
        message: 'missing restore failure tolerance test',
      },
      {
        regex: /\bremote_chat_history_assembly_preserves_message_shape_and_item_order\b/,
        message: 'missing remote chat history assembly shape/order test',
      },
      {
        regex: /\bremote_chat_history_assembly_skips_in_progress_assistant_history\b/,
        message: 'missing remote chat history in-progress guard test',
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
        regex: /\bremote_connect_file_response_assembly_owns_base64_wire_shape\b/,
        message: 'missing remote file response assembly contract test',
      },
      {
        regex: /\bremote_connect_file_command_handler_owns_owner_flow_and_uses_host_root\b/,
        message: 'missing remote file command handler owner-flow test',
      },
      {
        regex: /\bremote_connect_execution_response_helpers_preserve_wire_shape\b/,
        message: 'missing remote execution response helper contract test',
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
      {
        regex: /\bremote_connect_cancel_runtime_restores_missing_session_before_cancel\b/,
        message: 'missing remote cancel restore/order regression',
      },
      {
        regex: /\bremote_connect_cancel_runtime_preserves_stale_and_idle_errors_without_restore\b/,
        message: 'missing remote cancel stale/idle regression',
      },
      {
        regex: /\bremote_connect_cancel_runtime_preserves_restore_failure_error\b/,
        message: 'missing remote cancel restore failure regression',
      },
      {
        regex: /\bremote_connect_workspace_response_helpers_own_wire_shape\b/,
        message: 'missing remote workspace response assembly regression',
      },
      {
        regex: /\bremote_connect_session_response_helpers_own_pagination_and_timestamps\b/,
        message: 'missing remote session response assembly regression',
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
        regex: /\bsubmit_remote_dialog\b/,
        message: 'missing remote dialog owner orchestration delegation',
      },
      {
        regex: /\bcancel_remote_task\b/,
        message: 'missing remote cancel owner orchestration delegation',
      },
      {
        regex: /\bhandle_remote_workspace_file_command\b/,
        message: 'missing remote file command owner delegation',
      },
      {
        regex: /\bremote_dialog_submit_response\b/,
        message: 'missing remote dialog response assembly delegation',
      },
      {
        regex: /\bremote_task_cancel_response\b/,
        message: 'missing remote cancel response assembly delegation',
      },
      {
        regex: /\bhandle_remote_interaction_command\b/,
        message: 'missing remote interaction command owner orchestration delegation',
      },
      {
        regex: /\bgenerate_remote_initial_sync\b/,
        message: 'missing remote initial-sync owner orchestration delegation',
      },
      {
        regex: /\bhandle_remote_workspace_command\b/,
        message: 'missing remote workspace command owner orchestration delegation',
      },
      {
        regex: /\bhandle_remote_session_command\b/,
        message: 'missing remote session command owner orchestration delegation',
      },
      {
        regex: /\bhandle_remote_poll_command\b/,
        message: 'missing remote poll command owner orchestration delegation',
      },
      {
        regex: /\bhandle_remote_interaction_command\b/,
        message: 'missing remote interaction command owner orchestration delegation',
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
      'core product tool runtime owner keeps registry assembly and static tool materialization explicit until concrete tools migrate',
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
        regex: /\bproduct_tool_runtime_owner_preserves_registry_contract\b/,
        message: 'missing product runtime owner registry equivalence regression',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/product_runtime/snapshot.rs',
    reason:
      'product runtime snapshot wrapper must stay isolated from registry and catalog ownership',
    patterns: [
      {
        regex: /\bProductSnapshotToolWrapper\b/,
        message: 'missing core product snapshot wrapper adapter',
      },
      {
        regex: /\bimpl SnapshotToolWrapper<dyn Tool> for ProductSnapshotToolWrapper\b/,
        message: 'missing generic snapshot wrapper implementation',
      },
      {
        regex: /\bwrap_tool_for_snapshot_tracking\b/,
        message: 'missing snapshot wrapper boundary',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/product_runtime/catalog.rs',
    reason:
      'product runtime catalog owner keeps manifest, snapshot, readonly, and GetToolSpec product facades explicit',
    patterns: [
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
        regex: /\bresolve_product_resolved_tool_manifest\b/,
        message: 'missing product resolved manifest compatibility facade',
      },
      {
        regex: /\bresolve_product_resolved_visible_tools\b/,
        message: 'missing product resolved visible-tools compatibility facade',
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
        message: 'missing product runtime collapsed-tool unlock state source',
      },
      {
        regex: /\bproduct_catalog_provider_default_get_tool_spec_catalog_matches_registry\b/,
        message: 'missing product catalog provider collapsed catalog regression',
      },
      {
        regex: /\bproduct_resolved_manifest_owner_matches_legacy_shape\b/,
        message: 'missing product resolved manifest compatibility regression',
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
        regex: /\bresolve_product_resolved_visible_tools\b/,
        message: 'missing core product visible-tools facade delegation',
      },
      {
        regex: /\bresolve_product_resolved_tool_manifest\b/,
        message: 'missing core product manifest facade delegation',
      },
      {
        regex: /\bcollapsed_tool_names\b/,
        message: 'missing collapsed-tool name tracking',
      },
      {
        regex: /\bmanifest_resolver_facade_preserves_product_owner_output\b/,
        message: 'missing manifest resolver facade parity regression',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/product_runtime/get_tool_spec_tool.rs',
    reason:
      'product runtime must own the GetToolSpec Tool adapter while delegating generic runtime surface to agent-tools',
    patterns: [
      {
        regex: /\bpub(?:\(crate\))? struct GetToolSpecTool\b/,
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
        regex: /\bbuild_collapsed_tools_context_section\b/,
        message: 'missing core collapsed-tool request-context section renderer',
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
      'core tool framework must keep compatibility re-exports while ToolUseContext is owned by tool_context_runtime',
    patterns: [
      {
        regex: /\bToolExposure\b/,
        message: 'missing ToolExposure compatibility re-export',
      },
      {
        regex: /\bpub use crate::agentic::tools::tool_context_runtime::ToolUseContext\b/,
        message: 'missing ToolUseContext compatibility re-export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/tool_context_runtime.rs',
    reason:
      'core must keep ToolUseContext runtime/service bindings centralized while ToolUseContext and concrete tools remain core-owned',
    patterns: [
      {
        regex: /\bpub struct ToolUseContext\b/,
        message: 'missing ToolUseContext owner type',
      },
      {
        regex: /\bto_tool_context_facts\b/,
        message: 'missing portable ToolUseContext facts projection',
      },
      {
        regex: /\bimpl PortableToolContextProvider for ToolUseContext\b/,
        message: 'missing portable ToolUseContext facts provider impl',
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
      {
        regex: /\bimpl ToolUseContext\b/,
        message: 'missing ToolUseContext runtime binding owner impl',
      },
      {
        regex: /\brecord_light_checkpoint\b/,
        message: 'missing Deep Review checkpoint binding',
      },
      {
        regex: /\bcall_with_tool_runtime_hooks\b/,
        message: 'missing tool-call cancellation/post-call hook binding',
      },
      {
        regex: /\bcall_tool_with_runtime_hooks\b/,
        message: 'missing unified Tool::call runtime hook facade',
      },
      {
        regex: /\bcall_records_deep_review_read_file_measurement_without_touching_result\b/,
        message: 'missing Deep Review post-call hook regression in runtime owner',
      },
      {
        regex: /\bbuild_tool_use_context_for_task\b/,
        message: 'missing tool pipeline context materialization binding',
      },
      {
        regex: /\bbuild_tool_description_context\b/,
        message: 'missing tool manifest description context materialization binding',
      },
      {
        regex: /\bbuild_write_preflight_context\b/,
        message: 'missing write preflight context materialization binding',
      },
      {
        regex: /\bensure_current_workspace_runtime\b/,
        message: 'missing workspace runtime ensure binding',
      },
      {
        regex: /\bresolve_tool_path\b/,
        message: 'missing tool path resolution binding',
      },
      {
        regex: /\benforce_path_operation\b/,
        message: 'missing runtime path policy binding',
      },
      {
        regex: /\bis_tool_path_allowed_by_resolved_roots\b/,
        message: 'missing path policy owner delegation to agent-tools',
      },
      {
        regex: /\bbuild_tool_path_policy_denial_message\b/,
        message: 'missing shared path policy denial contract',
      },
      {
        regex: /\bresolve_tool_path_with_context\b/,
        message: 'missing shared tool path resolution owner delegation',
      },
      {
        regex: /\btool_path_is_effectively_absolute\b/,
        message: 'missing shared tool path absolute owner delegation',
      },
      {
        regex: /\bbuild_tool_runtime_artifact_reference\b/,
        message: 'missing runtime artifact reference owner delegation',
      },
      {
        regex: /\bbuild_tool_session_runtime_artifact_reference\b/,
        message: 'missing session runtime artifact reference owner delegation',
      },
      {
        regex: /\bworkspace_path_resolution_rejects_absolute_paths_outside_remote_workspace\b/,
        message: 'missing remote workspace containment regression',
      },
      {
        regex: /\bruntime_uri_resolution_rejects_different_workspace_scope\b/,
        message: 'missing runtime artifact scope regression',
      },
      {
        regex: /\bpath_policy_allows_only_configured_local_roots\b/,
        message: 'missing path policy enforcement regression',
      },
      {
        regex: /\btool_call_runtime_hook_returns_cancelled_before_impl_completes\b/,
        message: 'missing tool-call cancellation regression',
      },
      {
        regex: /\btool_task_context_materialization_preserves_runtime_fields\b/,
        message: 'missing tool task context materialization regression',
      },
      {
        regex: /\btool_description_context_preserves_manifest_custom_data_shape\b/,
        message: 'missing tool description context regression',
      },
      {
        regex: /\bwrite_preflight_context_preserves_minimal_runtime_fields\b/,
        message: 'missing write preflight context regression',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs',
    reason:
      'core must continue carrying collapsed-tool unlock state while delegating provider-neutral execution gate policy to agent-tools',
    patterns: [
      {
        regex: /\bvalidate_tool_execution_admission\b/,
        message: 'missing provider-neutral tool execution admission gate delegation',
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
      {
        regex: /\brender_tool_result_for_assistant\b/,
        message: 'missing tool result presentation owner delegation',
      },
      {
        regex: /\bbuild_tool_execution_error_presentation\b/,
        message: 'missing tool execution error presentation owner delegation',
      },
      {
        regex: /\bbuild_user_steering_interrupted_presentation\b/,
        message: 'missing steering-interrupted presentation owner delegation',
      },
      {
        regex: /\bbuild_invalid_tool_call_error_message\b/,
        message: 'missing invalid tool call presentation owner delegation',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/execution/execution_engine.rs',
    reason:
      'core execution must pass collapsed-tool unlock state through product runtime owner and keep DeepResearch post-turn hooks',
    patterns: [
      {
        regex: /\bcollect_product_unlocked_collapsed_tools\b/,
        message: 'missing product runtime collapsed-tool unlock state handoff',
      },
      {
        regex: /\bunlocked_collapsed_tools\b/,
        message: 'missing collapsed-tool unlock propagation into round context',
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
    path: 'src/crates/core/src/agentic/tools/product_runtime/unlock_state.rs',
    reason:
      'product runtime owns collapsed-tool unlock observation adaptation while preserving generic agent-tools policy',
    patterns: [
      {
        regex: /\bcollect_product_unlocked_collapsed_tools\b/,
        message: 'missing product runtime collapsed-tool unlock collector',
      },
      {
        regex: /\bGetToolSpecLoadObservation\b/,
        message: 'missing GetToolSpec load observation adapter',
      },
      {
        regex: /\bcollect_loaded_collapsed_tool_names\b/,
        message: 'missing generic collapsed-tool load collector delegation',
      },
      {
        regex: /\bproduct_unlock_state_dedupes_and_filters_runtime_unlocks\b/,
        message: 'missing collapsed-tool unlock filtering regression',
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
      'core Task tool must continue owning fork-aware background subagent launch semantics until a reviewed agent-runtime port preserves delivery behavior',
    patterns: [
      {
        regex: /\bfork_context\b/,
        message: 'missing Task fork_context schema and validation surface',
      },
      {
        regex: /\bSubagentContextMode::Fork\b/,
        message: 'missing forked subagent context mode path',
      },
      {
        regex: /delegation_policy\(\)\.spawn_child\(\)/,
        message: 'missing child delegation policy propagation',
      },
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
        regex: /Background \{\} started successfully/,
        message: 'missing assistant-visible background start acknowledgement',
      },
      {
        regex: /<background_task status=\\"started\\"/,
        message: 'missing structured background task start acknowledgement',
      },
      {
        regex: /\bbackground_subagent_start_acknowledgement_keeps_structured_task_marker\b/,
        message: 'missing background task start acknowledgement regression',
      },
    ],
  },
  {
    path: 'src/crates/core/src/agentic/coordination/scheduler.rs',
    reason:
      'core scheduler must continue owning background subagent result delivery until running-turn and idle-session routing equivalence tests exist',
    patterns: [
      {
        regex: /\bdeliver_background_result\b/,
        message: 'missing background subagent delivery entry point',
      },
      {
        regex: /RoundInjectionKind::BackgroundResult/,
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
    path: 'src/crates/core/src/service/search/mod.rs',
    reason:
      'remote workspace search must route to the real implementation only when ssh-remote is enabled',
    patterns: [
      {
        regex: /#\[cfg\(feature = "ssh-remote"\)\]\s*mod remote\b/s,
        message: 'missing ssh-remote gate for real remote search implementation',
      },
      {
        regex: /#\[cfg\(not\(feature = "ssh-remote"\)\)\]\s*mod remote_disabled\b/s,
        message: 'missing disabled remote search implementation for no-default builds',
      },
      {
        regex: /#\[cfg\(not\(feature = "ssh-remote"\)\)\]\s*pub use remote_disabled/s,
        message: 'missing disabled remote search export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/service/search/remote_disabled.rs',
    reason:
      'no-default core builds must keep remote search unavailable with an explicit diagnostic',
    patterns: [
      {
        regex: /Remote SSH search is disabled; enable the `ssh-remote` feature/,
        message: 'missing explicit disabled remote search diagnostic',
      },
      {
        regex: /\bpub struct RemoteWorkspaceSearchService\b/,
        message: 'missing disabled remote workspace search service surface',
      },
      {
        regex: /\bremote_workspace_search_service_for_path\b/,
        message: 'missing disabled remote workspace search resolver',
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
        regex: /\bbuild_builtin_seed_meta\b/,
        message: 'missing product-domain built-in MiniApp seed meta helper use',
      },
      {
        regex: /\bpreserved_builtin_created_at\b/,
        message: 'missing product-domain built-in MiniApp timestamp preservation helper use',
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
        regex: /\bsplit_host_method\b/,
        message: 'missing product-domain MiniApp host method split use',
      },
      {
        regex: /\basync fn dispatch_fs\b/,
        message: 'missing MiniApp fs host dispatch',
      },
      {
        regex: /\bfs_method_access_mode\b/,
        message: 'missing product-domain MiniApp fs access-mode policy use',
      },
      {
        regex: /\bfs_policy_scopes\b/,
        message: 'missing product-domain MiniApp fs scope extraction policy use',
      },
      {
        regex: /\bfs_resolved_path_allowed\b/,
        message: 'missing product-domain MiniApp fs resolved path policy use',
      },
      {
        regex: /\basync fn dispatch_shell\b/,
        message: 'missing MiniApp shell host dispatch',
      },
      {
        regex: /\bshell_exec_first_token\b/,
        message: 'missing product-domain MiniApp shell token policy use',
      },
      {
        regex: /\bshell_exec_input_is_empty\b/,
        message: 'missing product-domain MiniApp shell empty-input policy use',
      },
      {
        regex: /\bshell_exec_cwd\b/,
        message: 'missing product-domain MiniApp shell cwd policy use',
      },
      {
        regex: /\bshell_exec_timeout_ms\b/,
        message: 'missing product-domain MiniApp shell timeout policy use',
      },
      {
        regex: /\bshell_exec_default_env\b/,
        message: 'missing product-domain MiniApp shell env policy use',
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
        regex: /\bpub struct MiniAppCreateInput\b/,
        message: 'missing MiniApp create input contract',
      },
      {
        regex: /\bpub struct MiniAppUpdatePatch\b/,
        message: 'missing MiniApp update patch contract',
      },
      {
        regex: /\bpub fn build_created_app\b/,
        message: 'missing MiniApp create state helper',
      },
      {
        regex: /\bpub fn apply_update_patch\b/,
        message: 'missing MiniApp update state helper',
      },
      {
        regex: /\bpub fn prepare_draft_app\b/,
        message: 'missing MiniApp draft prepare state helper',
      },
      {
        regex: /\bpub fn apply_draft_source_sync_result\b/,
        message: 'missing MiniApp draft source-sync state helper',
      },
      {
        regex: /\bpub fn apply_draft_permission_update_result\b/,
        message: 'missing MiniApp draft permission-update state helper',
      },
      {
        regex: /\bpub fn apply_draft_to_active\b/,
        message: 'missing MiniApp draft apply state helper',
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
      {
        regex: /\bpub fn prepare_imported_meta\b/,
        message: 'missing MiniApp imported metadata identity/timestamp helper',
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
      'product-domains owns MiniApp runtime detection, including the reviewed concrete PATH/fs/version probe',
    patterns: [
      {
        regex: /\bpub fn detect_runtime\b/,
        message: 'missing MiniApp concrete runtime detector',
      },
      {
        regex: /\bstruct DefaultMiniAppRuntimeProbe\b/,
        message: 'missing MiniApp default runtime probe owner',
      },
      {
        regex: /\bwhich::which\b/,
        message: 'missing MiniApp PATH lookup owner',
      },
      {
        regex: /\bstd::fs::read_dir\b/,
        message: 'missing MiniApp version-manager directory scan owner',
      },
      {
        regex: /\bcreate_version_command\b/,
        message: 'missing MiniApp version process command owner',
      },
      {
        regex: /\bpub fn runtime_lookup_order\b/,
        message: 'missing MiniApp runtime lookup order contract',
      },
      {
        regex: /\bpub trait MiniAppRuntimeProbe\b/,
        message: 'missing MiniApp runtime probe contract',
      },
      {
        regex: /\bpub fn detect_runtime_with_probe\b/,
        message: 'missing MiniApp runtime detector facade',
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
    path: 'src/crates/product-domains/src/miniapp/worker.rs',
    reason:
      'product-domains owns MiniApp worker pool policy and install-deps planning while core keeps worker process execution',
    patterns: [
      {
        regex: /\bpub enum InstallDepsPlan\b/,
        message: 'missing MiniApp install-deps plan contract',
      },
      {
        regex: /\bpub fn plan_install_deps\b/,
        message: 'missing MiniApp install-deps planning helper',
      },
      {
        regex: /\bpub fn worker_pool_capacity\b/,
        message: 'missing MiniApp worker pool capacity policy helper',
      },
      {
        regex: /\bpub fn worker_idle_timeout_ms\b/,
        message: 'missing MiniApp worker idle timeout policy helper',
      },
      {
        regex: /\bpub fn worker_is_idle\b/,
        message: 'missing MiniApp worker idle policy helper',
      },
      {
        regex: /\bpub fn select_lru_worker\b/,
        message: 'missing MiniApp worker LRU selection helper',
      },
      {
        regex: /\binstall_deps_plan_preserves_no_package_noop_and_runtime_commands\b/,
        message: 'missing MiniApp install-deps planning regression test',
      },
      {
        regex: /\bworker_pool_policy_keeps_existing_capacity_and_idle_timeout_contract\b/,
        message: 'missing MiniApp worker pool policy regression test',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/miniapp/host_routing.rs',
    reason:
      'product-domains owns MiniApp host-routing and allowlist decision policy while core keeps host execution',
    patterns: [
      {
        regex: /\bpub fn split_host_method\b/,
        message: 'missing MiniApp host method split helper',
      },
      {
        regex: /\bpub enum FsAccessMode\b/,
        message: 'missing MiniApp fs access mode contract',
      },
      {
        regex: /\bpub fn fs_method_access_mode\b/,
        message: 'missing MiniApp fs access mode helper',
      },
      {
        regex: /\bpub fn fs_policy_scopes\b/,
        message: 'missing MiniApp fs policy scope helper',
      },
      {
        regex: /\bpub fn fs_resolved_path_allowed\b/,
        message: 'missing MiniApp fs resolved path helper',
      },
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
      {
        regex: /\bpub fn shell_exec_first_token\b/,
        message: 'missing MiniApp shell first-token policy helper',
      },
      {
        regex: /\bpub fn shell_exec_input_is_empty\b/,
        message: 'missing MiniApp shell empty-input policy helper',
      },
      {
        regex: /\bpub fn shell_exec_cwd\b/,
        message: 'missing MiniApp shell cwd policy helper',
      },
      {
        regex: /\bpub fn shell_exec_timeout_ms\b/,
        message: 'missing MiniApp shell timeout policy helper',
      },
      {
        regex: /\bpub fn shell_exec_default_env\b/,
        message: 'missing MiniApp shell env policy helper',
      },
      {
        regex: /\bfs_method_access_mode_preserves_access_bypass_and_default_read_contract\b/,
        message: 'missing MiniApp fs access mode regression test',
      },
      {
        regex: /\bfs_policy_scopes_and_resolved_prefix_check_preserve_path_boundary\b/,
        message: 'missing MiniApp fs path policy regression test',
      },
      {
        regex: /\bshell_exec_first_token_prefers_argv_over_shell_command_text\b/,
        message: 'missing MiniApp shell first-token regression test',
      },
      {
        regex: /\bshell_exec_plan_helpers_preserve_defaults_and_precedence\b/,
        message: 'missing MiniApp shell plan regression test',
      },
    ],
  },
  {
    path: 'src/crates/product-domains/src/miniapp/exporter.rs',
    reason:
      'product-domains owns MiniApp export check result policy while core keeps runtime detection',
    patterns: [
      {
        regex: /\bpub const MISSING_JS_RUNTIME_MESSAGE\b/,
        message: 'missing MiniApp export missing-runtime message contract',
      },
      {
        regex: /\bpub fn export_runtime_label\b/,
        message: 'missing MiniApp export runtime label helper',
      },
      {
        regex: /\bpub fn build_export_check_result\b/,
        message: 'missing MiniApp export check result helper',
      },
    ],
  },
  {
    path: 'src/crates/core/src/miniapp/exporter.rs',
    reason:
      'core MiniApp exporter must delegate export check result policy while retaining runtime detection and export skeleton',
    patterns: [
      {
        regex: /\bdetect_runtime\b/,
        message: 'missing core-owned MiniApp export runtime detection',
      },
      {
        regex: /\bbuild_export_check_result\b/,
        message: 'missing product-domain MiniApp export check helper use',
      },
      {
        regex: /Export not yet implemented \(skeleton\)/,
        message: 'missing core-owned MiniApp export skeleton behavior',
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
        regex: /\bbuild_created_app\b/,
        message: 'missing product-domain MiniApp create lifecycle helper use',
      },
      {
        regex: /\bapply_update_patch\b/,
        message: 'missing product-domain MiniApp update lifecycle helper use',
      },
      {
        regex: /\bprepare_draft_app\b/,
        message: 'missing product-domain MiniApp draft prepare lifecycle helper use',
      },
      {
        regex: /\bapply_draft_to_active\b/,
        message: 'missing product-domain MiniApp draft apply lifecycle helper use',
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
        regex: /\bprepare_imported_meta\b/,
        message: 'missing product-domain MiniApp imported metadata helper use',
      },
      {
        regex: /\bpersist_import_runtime_state\b/,
        message: 'missing product-domain MiniApp import runtime-state facade delegation',
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
      {
        regex: /\bpersist_import_runtime_state\b/,
        message: 'missing MiniApp import runtime-state facade path',
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
      {
        regex: /\bpub fn preserved_builtin_created_at\b/,
        message: 'missing built-in MiniApp created-at preservation helper',
      },
      {
        regex: /\bpub fn build_builtin_seed_meta\b/,
        message: 'missing built-in MiniApp seed meta helper',
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
      'core MiniApp runtime detection must be a compatibility facade over product-domain runtime detection',
    patterns: [
      {
        regex: /\bpub use bitfun_product_domains::miniapp::runtime::\{/,
        message: 'missing product-domain MiniApp runtime facade re-export',
      },
      {
        regex: /\bdetect_runtime\b/,
        message: 'missing product-domain detect_runtime facade export',
      },
    ],
  },
  {
    path: 'src/crates/core/src/miniapp/js_worker_pool.rs',
    reason:
      'core must continue owning MiniApp worker runtime adapter until process/runtime migration is reviewed',
    patterns: [
      {
        regex: /\bplan_install_deps\b/,
        message: 'missing product-domain install-deps plan use',
      },
      {
        regex: /\bworker_is_idle\b/,
        message: 'missing product-domain worker idle policy use',
      },
      {
        regex: /\bworker_pool_at_capacity\b/,
        message: 'missing product-domain worker capacity policy use',
      },
      {
        regex: /\bselect_lru_worker\b/,
        message: 'missing product-domain worker LRU policy use',
      },
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
      currentInline.text.push(trimmed);
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
            text: [trimmed],
          };
          deps.push(currentTable);
          break;
        }
      }
      return;
    }

    if (currentTable) {
      currentTable.text.push(trimmed);
      if (/\boptional\s*=\s*true\b/.test(trimmed)) {
        currentTable.optional = true;
      }
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
        text: [trimmed],
      });
      if (trimmed.includes('{') && !trimmed.includes('}')) {
        currentInline = deps[deps.length - 1];
      }
      return;
    }

  });

  return deps;
}

function manifestDependencyText(dep) {
  return dep?.text?.join('\n') ?? '';
}

function manifestDependencyDisablesDefaultFeatures(dep) {
  return /\bdefault-features\s*=\s*false\b/.test(manifestDependencyText(dep));
}

function parseManifestDependencyFeatureNames(dep) {
  const features = new Set();
  const text = manifestDependencyText(dep);
  for (const match of text.matchAll(/\bfeatures\s*=\s*\[([\s\S]*?)\]/g)) {
    for (const featureMatch of match[1].matchAll(/"([^"]+)"/g)) {
      features.add(featureMatch[1]);
    }
  }
  return features;
}

function collectProductCoreDependencyManifestPaths(manifestEntries) {
  return manifestEntries
    .filter((entry) => {
      const deps = parseManifestDependencies(entry.text.split(/\r?\n/));
      return deps.some((dep) => dep.name === 'bitfun-core');
    })
    .map((entry) => entry.manifestPath)
    .sort();
}

function collectProductCoreDependencyManifests(scanRoots = productCoreFeatureAssemblyScanRoots) {
  const manifestEntries = [];
  for (const repoDir of scanRoots) {
    const dir = join(ROOT, ...repoDir.split('/'));
    walkFiles(dir, (path) => {
      if (!path.endsWith('Cargo.toml')) {
        return;
      }
      manifestEntries.push({
        manifestPath: toRepoPath(path),
        text: readText(path),
      });
    });
  }
  return collectProductCoreDependencyManifestPaths(manifestEntries);
}

function parseManifestFeatures(lines) {
  const features = new Map();
  let inFeatures = false;
  let currentFeature = null;

  const appendRefs = (feature, text) => {
    const refs = [...text.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
    feature.refs.push(...refs);
  };

  lines.forEach((line, index) => {
    const trimmed = line.trim();
    if (trimmed.startsWith('#') || trimmed === '') {
      return;
    }

    const headerMatch = trimmed.match(/^\[(.+)]$/);
    if (headerMatch) {
      inFeatures = trimmed === '[features]';
      currentFeature = null;
      return;
    }

    if (!inFeatures) {
      return;
    }

    if (currentFeature) {
      appendRefs(currentFeature, trimmed);
      if (trimmed.includes(']')) {
        currentFeature = null;
      }
      return;
    }

    const featureMatch = trimmed.match(/^([A-Za-z0-9_-]+)\s*=\s*(.*)$/);
    if (!featureMatch) {
      return;
    }

    const feature = {
      name: featureMatch[1],
      line: index + 1,
      refs: [],
    };
    appendRefs(feature, featureMatch[2]);
    features.set(feature.name, feature);
    if (featureMatch[2].includes('[') && !featureMatch[2].includes(']')) {
      currentFeature = feature;
    }
  });

  return features;
}

function collectKnownDependencyNames() {
  return Array.from(
    new Set([
      'bitfun-core',
      ...lightweightBoundaryRules.flatMap((rule) => rule.forbiddenDeps),
      ...dependencyProfileRules.flatMap((rule) => rule.forbiddenNonOptionalDeps),
      ...optionalDependencyFeatureOwnerRules.flatMap((rule) =>
        rule.dependencies.map((dependency) => dependency.depName),
      ),
      ...productCoreFeatureAssemblyRules.map((rule) => rule.dependencyName),
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
    'bitfun-core = { path = "../core", default-features = false, features = ["product-full"] }',
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
  const parsedCoreDep = parsedByName.get('bitfun-core');
  if (!manifestDependencyDisablesDefaultFeatures(parsedCoreDep)) {
    throw new Error('dependency profile parser must detect default-features = false');
  }
  if (!parseManifestDependencyFeatureNames(parsedCoreDep).has('product-full')) {
    throw new Error('dependency profile parser must detect inline dependency features');
  }
  const parsedCoreTableDeps = parseManifestDependencies([
    '[dependencies."bitfun-core"]',
    'path = "../core"',
    'default-features = false',
    'features = [',
    '  "product-full",',
    '  "ssh-remote",',
    ']',
  ]);
  const parsedCoreTableDep = parsedCoreTableDeps.find((dep) => dep.name === 'bitfun-core');
  if (!manifestDependencyDisablesDefaultFeatures(parsedCoreTableDep)) {
    throw new Error('dependency profile parser must detect table default-features = false');
  }
  if (!parseManifestDependencyFeatureNames(parsedCoreTableDep).has('ssh-remote')) {
    throw new Error('dependency profile parser must detect table dependency features');
  }
  if (parsedByName.has('image')) {
    throw new Error('dependency profile parser must ignore feature entries named like dependencies');
  }

  const productCoreRulePaths = new Set(
    productCoreFeatureAssemblyRules.map((rule) => rule.manifestPath),
  );
  for (const manifestPath of [
    'src/apps/desktop/Cargo.toml',
    'src/apps/cli/Cargo.toml',
    'src/crates/acp/Cargo.toml',
  ]) {
    if (!productCoreRulePaths.has(manifestPath)) {
      throw new Error(`product core feature assembly rule must cover ${manifestPath}`);
    }
  }
  for (const rule of productCoreFeatureAssemblyRules) {
    if (!rule.requiredFeatures.includes('product-full')) {
      throw new Error(`${rule.manifestPath} must require bitfun-core product-full`);
    }
  }
  for (const featureName of ['ssh-remote', 'product-domains', 'service-integrations', 'tool-packs']) {
    if (!coreProductFullFeatureAssemblyRule.requiredFeatureRefs.includes(featureName)) {
      throw new Error(`core product-full assembly rule must require ${featureName}`);
    }
  }
  const discoveredProductCoreManifests = collectProductCoreDependencyManifestPaths([
    {
      manifestPath: 'src/apps/desktop/Cargo.toml',
      text:
        '[dependencies]\nbitfun-core = { path = "../../crates/core", default-features = false, features = ["product-full"] }',
    },
    {
      manifestPath: 'src/apps/server/Cargo.toml',
      text: '[dependencies]\naxum = { workspace = true }',
    },
    {
      manifestPath: 'src/crates/acp/Cargo.toml',
      text: '[dependencies."bitfun-core"]\npath = "../core"\ndefault-features = false\nfeatures = ["product-full"]',
    },
  ]);
  if (discoveredProductCoreManifests.join(',') !== 'src/apps/desktop/Cargo.toml,src/crates/acp/Cargo.toml') {
    throw new Error('product core dependency scanner must discover only manifests that depend on bitfun-core');
  }
  const ownerFeatureRulePaths = new Set(
    ownerCrateFeatureAssemblyRules.map((rule) => rule.manifestPath),
  );
  for (const manifestPath of [
    'src/crates/tool-packs/Cargo.toml',
    'src/crates/services-integrations/Cargo.toml',
    'src/crates/product-domains/Cargo.toml',
  ]) {
    if (!ownerFeatureRulePaths.has(manifestPath)) {
      throw new Error(`owner crate feature assembly rule must cover ${manifestPath}`);
    }
  }
  for (const rule of ownerCrateFeatureAssemblyRules) {
    const declaredFeatures = new Set(rule.requiredProductFullFeatures);
    if (declaredFeatures.size !== rule.requiredProductFullFeatures.length) {
      throw new Error(`${rule.manifestPath} product-full guard must not duplicate feature groups`);
    }
    if (rule.requiredProductFullFeatures.some((featureName) => featureName.startsWith('dep:'))) {
      throw new Error(`${rule.manifestPath} product-full guard must track owner feature groups only`);
    }
  }

  const parsedFeatures = parseManifestFeatures([
    '[package]',
    'name = "example"',
    '[features]',
    'default = ["product-full"]',
    'product-full = [',
    '    "dep:tool-runtime",',
    '    "service-integrations",',
    ']',
    'service-integrations = ["dep:git2", "dep:rmcp"]',
    'ssh-remote = [',
    '    "russh",',
    '    "russh-sftp",',
    ']',
    '[dependencies]',
    'git2 = { workspace = true, optional = true }',
  ]);
  if (!parsedFeatures.get('default')?.refs.includes('product-full')) {
    throw new Error('feature parser must detect inline feature references');
  }
  if (!parsedFeatures.get('product-full')?.refs.includes('dep:tool-runtime')) {
    throw new Error('feature parser must detect multiline dependency feature references');
  }
  if (!parsedFeatures.get('service-integrations')?.refs.includes('dep:rmcp')) {
    throw new Error('feature parser must detect inline dependency feature references');
  }
  if (!parsedFeatures.get('ssh-remote')?.refs.includes('russh-sftp')) {
    throw new Error('feature parser must detect implicit optional dependency feature references');
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
    'get_global_coordinator',
    'GitService',
    'get_workspace_runtime_service_arc',
    'remote_workspace_runtime_root',
    'get_path_manager_arc',
    'post_call_hooks::record_successful_tool_call',
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
    'normalize_absolute_posix_path',
  ];
  const coreToolRestrictionRuleText = coreToolRestrictionRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of coreToolRestrictionContracts) {
    if (!coreToolRestrictionRuleText.includes(contract)) {
      throw new Error(`core tool restrictions boundary rule must forbid contract: ${contract}`);
    }
  }
  const agentToolsFrameworkRule = requiredContentRules.find(
    (rule) => rule.path === 'src/crates/agent-tools/src/framework.rs',
  );
  if (!agentToolsFrameworkRule) {
    throw new Error('missing agent-tools framework boundary rule');
  }
  const agentToolsFrameworkContracts = [
    'is_tool_path_allowed_by_resolved_roots',
    'build_tool_path_policy_denial_message',
    'resolve_tool_path_with_context',
    'tool_path_is_effectively_absolute',
    'build_tool_runtime_artifact_reference',
    'build_tool_session_runtime_artifact_reference',
  ];
  const agentToolsFrameworkRuleText = agentToolsFrameworkRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of agentToolsFrameworkContracts) {
    if (!agentToolsFrameworkRuleText.includes(contract)) {
      throw new Error(`agent-tools framework boundary rule must require contract: ${contract}`);
    }
  }
  const coreWorkspacePathRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/agentic/tools/workspace_paths.rs',
  );
  if (!coreWorkspacePathRule) {
    throw new Error('missing core workspace path boundary rule');
  }
  const coreWorkspacePathContracts = [
    'BITFUN_RUNTIME_URI_PREFIX',
    'ParsedBitFunRuntimeUri',
    'posix_normalize_components',
    'Component::ParentDir',
  ];
  const coreWorkspacePathRuleText = coreWorkspacePathRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of coreWorkspacePathContracts) {
    if (!coreWorkspacePathRuleText.includes(contract)) {
      throw new Error(`core workspace path boundary rule must forbid contract: ${contract}`);
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
  const coreSubagentRuntimeRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/agentic/subagent_runtime/mod.rs',
  );
  if (!coreSubagentRuntimeRule) {
    throw new Error('missing core subagent runtime boundary rule');
  }
  const coreSubagentRuntimeRuleText = coreSubagentRuntimeRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of ['DelegationPolicy', 'SubagentContextMode']) {
    if (!coreSubagentRuntimeRuleText.includes(contract)) {
      throw new Error(`core subagent runtime boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreCoordinatorRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/agentic/coordination/coordinator.rs',
  );
  if (!coreCoordinatorRule) {
    throw new Error('missing core coordinator boundary rule');
  }
  const coreCoordinatorRuleText = coreCoordinatorRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  if (!coreCoordinatorRuleText.includes('DialogTriggerSource')) {
    throw new Error('core coordinator boundary rule must forbid DialogTriggerSource redefinition');
  }
  const coreSchedulerRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/agentic/coordination/scheduler.rs',
  );
  if (!coreSchedulerRule) {
    throw new Error('missing core scheduler boundary rule');
  }
  const coreSchedulerRuleText = coreSchedulerRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of [
    'DialogQueuePriority',
    'DialogSubmissionPolicy',
    'DialogSubmitOutcome',
    'AgentSessionReplyRoute',
    'DialogSteerOutcome',
  ]) {
    if (!coreSchedulerRuleText.includes(contract)) {
      throw new Error(`core scheduler boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreRoundPreemptRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/agentic/round_preempt.rs',
  );
  if (!coreRoundPreemptRule) {
    throw new Error('missing core round preempt boundary rule');
  }
  const coreRoundPreemptRuleText = coreRoundPreemptRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of [
    'DialogRoundPreemptSource',
    'RoundInjection',
    'DialogRoundInjectionSource',
    'RoundInjectionKind',
    'RoundInjectionTarget',
  ]) {
    if (!coreRoundPreemptRuleText.includes(contract)) {
      throw new Error(`core round preempt boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreGoalModeTypesRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/agentic/goal_mode/mod.rs',
  );
  if (!coreGoalModeTypesRule) {
    throw new Error('missing core goal mode types boundary rule');
  }
  const coreGoalModeTypesRuleText = coreGoalModeTypesRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of [
    'ThreadGoal',
    'ThreadGoalStatus',
    'SetThreadGoalResult',
    'ThreadGoalContinuationPlan',
    'ThreadGoalToolResponse',
  ]) {
    if (!coreGoalModeTypesRuleText.includes(contract)) {
      throw new Error(`core goal mode types boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreMessageRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/agentic/core/message.rs',
  );
  if (!coreMessageRule) {
    throw new Error('missing core message boundary rule');
  }
  const coreMessageRuleText = coreMessageRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of ['CompressionContract', 'CompressionContractItem']) {
    if (!coreMessageRuleText.includes(contract)) {
      throw new Error(`core message boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreWorkspaceRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/core/src/service/workspace/manager.rs',
  );
  if (!coreWorkspaceRule) {
    throw new Error('missing core workspace manager boundary rule');
  }
  if (
    !coreWorkspaceRule.patterns
      .map((pattern) => pattern.regex.source)
      .join('\n')
      .includes('RelatedPath')
  ) {
    throw new Error('core workspace manager boundary rule must forbid contract: RelatedPath');
  }
  const coreSubagentRuntimeOwnerPathRule = forbiddenContentUnderRules.find(
    (rule) => rule.path === 'src/crates/core/src',
  );
  if (!coreSubagentRuntimeOwnerPathRule) {
    throw new Error('missing core subagent runtime owner-path boundary rule');
  }
  const coreSubagentRuntimeOwnerPathRuleText = coreSubagentRuntimeOwnerPathRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of ['DelegationPolicy', 'SubagentContextMode']) {
    if (!coreSubagentRuntimeOwnerPathRuleText.includes(contract)) {
      throw new Error(
        `core subagent runtime owner-path rule must forbid compatibility import: ${contract}`,
      );
    }
  }

  const productDomainProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'product-domains',
  );
  for (const dep of ['dirs', 'log', 'sha2', 'which']) {
    if (!productDomainProfile?.forbiddenNonOptionalDeps.includes(dep)) {
      throw new Error(`product-domains default profile must forbid non-optional ${dep}`);
    }
  }
  const servicesIntegrationsDefaultProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'services-integrations',
  );
  if (!servicesIntegrationsDefaultProfile?.forbiddenNonOptionalDeps.includes('uuid')) {
    throw new Error('services-integrations default profile must forbid non-optional uuid');
  }
  const coreProfile = dependencyProfileRules.find((rule) => rule.crateName === 'core');
  for (const dep of ['git2', 'rmcp', 'image', 'tool-runtime', 'bitfun-relay-server']) {
    if (!coreProfile?.forbiddenNonOptionalDeps.includes(dep)) {
      throw new Error(`core no-default profile must forbid non-optional ${dep}`);
    }
  }
  const coreOptionalOwnerRule = optionalDependencyFeatureOwnerRules.find(
    (rule) => rule.crateName === 'core',
  );
  const coreOptionalOwnerDeps = new Set(
    coreOptionalOwnerRule?.dependencies.map((dependency) => dependency.depName) ?? [],
  );
  for (const dep of coreProfile?.forbiddenNonOptionalDeps ?? []) {
    if (!coreOptionalOwnerDeps.has(dep)) {
      throw new Error(`core optional dependency owner rule must cover forbidden dependency ${dep}`);
    }
  }
  for (const dep of ['git2', 'rmcp', 'image', 'tool-runtime', 'bitfun-relay-server']) {
    if (!coreOptionalOwnerDeps.has(dep)) {
      throw new Error(`core optional dependency owner rule must cover ${dep}`);
    }
  }
  const coreGit2Owner = coreOptionalOwnerRule?.dependencies.find(
    (dependency) => dependency.depName === 'git2',
  );
  if (!coreGit2Owner?.ownerFeatures.includes('service-integrations')) {
    throw new Error('core optional dependency owner rule must keep git2 under service-integrations');
  }
  const servicesOptionalOwnerRule = optionalDependencyFeatureOwnerRules.find(
    (rule) => rule.crateName === 'services-integrations',
  );
  for (const dep of ['bitfun-runtime-ports', 'git2', 'notify', 'rmcp']) {
    if (!servicesOptionalOwnerRule?.dependencies.some((dependency) => dependency.depName === dep)) {
      throw new Error(`services-integrations optional dependency owner rule must cover ${dep}`);
    }
  }
  const productDomainsOptionalOwnerRule = optionalDependencyFeatureOwnerRules.find(
    (rule) => rule.crateName === 'product-domains',
  );
  for (const dep of ['dirs', 'log', 'sha2']) {
    if (!productDomainsOptionalOwnerRule?.dependencies.some((dependency) => dependency.depName === dep)) {
      throw new Error(`product-domains optional dependency owner rule must cover ${dep}`);
    }
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
  const productDomainCommandRule = productDomainRuntimeRule.patterns.find((pattern) =>
    pattern.regex.source.includes('Command::new'),
  );
  if (
    !productDomainCommandRule?.allowPaths?.includes(
      'src/crates/product-domains/src/miniapp/runtime.rs',
    )
  ) {
    throw new Error('product-domains Command::new exception must stay scoped to MiniApp runtime detection');
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
  const runtimeServicesRule = lightweightBoundaryRules.find(
    (rule) => rule.crateName === 'runtime-services',
  );
  if (!runtimeServicesRule?.forbiddenDeps.includes('bitfun-core')) {
    throw new Error('runtime-services lightweight boundary must forbid bitfun-core');
  }
  if (!runtimeServicesRule?.forbiddenDeps.includes('bitfun-services-integrations')) {
    throw new Error('runtime-services lightweight boundary must forbid concrete service integrations');
  }
  const runtimeServicesProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'runtime-services',
  );
  if (!runtimeServicesProfile?.forbiddenNonOptionalDeps.includes('tool-runtime')) {
    throw new Error('runtime-services dependency profile must forbid tool runtime implementations');
  }
  const agentRuntimeRule = lightweightBoundaryRules.find(
    (rule) => rule.crateName === 'agent-runtime',
  );
  if (!agentRuntimeRule?.forbiddenDeps.includes('bitfun-core')) {
    throw new Error('agent-runtime lightweight boundary must forbid bitfun-core');
  }
  if (!agentRuntimeRule?.forbiddenDeps.includes('bitfun-services-integrations')) {
    throw new Error('agent-runtime lightweight boundary must forbid concrete service integrations');
  }
  const agentRuntimeProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'agent-runtime',
  );
  if (!agentRuntimeProfile?.forbiddenNonOptionalDeps.includes('tauri')) {
    throw new Error('agent-runtime dependency profile must forbid product surface dependencies');
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
        'RemoteWorkspaceFacts',
        'RemoteWorkspaceRuntimeHost',
        'RemoteWorkspacePort',
        'RemoteWorkspaceFileRuntimeHost',
        'RemoteProjectionPort',
        'RemoteInitialSyncRuntimeHost',
        'remote_workspace_contracts_preserve_workspace_and_session_facts',
        'remote_projection_contract_preserves_file_chunk_identity',
        'remote_image',
        'DialogTriggerSource',
        'dialog_trigger_source_reuses_agent_submission_source_contract',
        'DialogQueuePriority',
        'DialogSubmissionPolicy',
        'dialog_submission_policy_preserves_current_surface_queue_defaults',
        'DialogSubmitOutcome',
        'dialog_submit_outcome_preserves_started_and_queued_fields',
        'DialogSessionStateFact',
        'DialogSubmitQueueFacts',
        'DialogSubmitQueueAction',
        'dialog_policy_may_preempt',
        'resolve_dialog_submit_queue_action',
        'dialog_submit_queue_action_preserves_current_scheduler_routing_policy',
        'should_suppress_agent_session_cancelled_reply',
        'DialogTurnOutcomeKind',
        'should_skip_agent_session_reply',
        'agent_session_reply_decisions_preserve_cancel_suppression_boundary',
        'AgentSessionReplyRoute',
        'agent_session_reply_route_keeps_requester_fields',
        'DialogSteerOutcome',
        'dialog_steer_outcome_preserves_buffered_fields',
        'RoundInjectionKind',
        'RoundInjectionTarget',
        'RoundInjection',
        'DialogRoundPreemptSource',
        'DialogRoundInjectionSource',
        'round_injection_contract_keeps_kind_and_target_identity',
        'round_injection_source_contract_drains_portable_injections',
        'ThreadGoalStatus',
        'ThreadGoal',
        'SetThreadGoalResult',
        'ThreadGoalContinuationPlan',
        'ThreadGoalToolResponse',
        'thread_goal_active_status_includes_budget_limited',
        'thread_goal_tool_response_serializes_optional_fields',
        'CompressionContract',
        'CompressionContractItem',
        'compression_contract_renders_model_visible_fields',
        'RelatedPath',
        'related_path_serializes_as_request_context_fact',
        'WorkspaceFileSystem',
        'WorkspaceShell',
        'WorkspaceServices',
        'WorkspaceCommandOptions',
        'WorkspaceCommandResult',
        'WorkspaceDirEntry',
        'workspace_services_contract_is_runtime_port_owned',
        'DelegationPolicy',
        'SubagentContextMode',
        'delegation_policy_child_blocks_recursive_spawn_without_losing_depth',
        'subagent_context_mode_preserves_fork_wire_value',
      ],
    },
    {
      path: 'src/crates/runtime-services/src/lib.rs',
      contracts: [
        'RuntimeServices',
        'RuntimeServicesBuilder',
        'CapabilityAvailability',
        'RuntimeServicesProvider',
        'RuntimeServicesRegistry',
        'CapabilityMismatch',
        'require_capability',
      ],
    },
    {
      path: 'src/crates/runtime-services/tests/runtime_services_contracts.rs',
      contracts: [
        'builder_requires_mandatory_runtime_services',
        'fake_provider_registers_required_and_remote_services_through_registry',
        'missing_optional_capability_returns_typed_unsupported_error',
        'capability_availability_reports_optional_service_status_without_side_effects',
        'builder_rejects_port_registered_under_the_wrong_capability',
        'registered_remote_ports_expose_owner_contract_methods',
      ],
    },
    {
      path: 'src/crates/agent-runtime/src/scheduler.rs',
      contracts: [
        'BackgroundDeliveryFacts',
        'BackgroundDeliveryAction',
        'resolve_background_delivery_action',
        'follow_up_submission_policy',
        'SubmitAgentSessionFollowUp',
        'InjectIntoRunningTurn',
      ],
    },
    {
      path: 'src/crates/agent-runtime/tests/scheduler_contracts.rs',
      contracts: [
        'background_delivery_injects_when_session_is_processing',
        'background_delivery_starts_agent_session_follow_up_when_session_is_not_processing',
        'background_delivery_follow_up_uses_agent_session_source_semantics',
        'background_delivery_injection_does_not_expose_follow_up_policy',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/subagent_runtime/mod.rs',
      contracts: [
        'bitfun_runtime_ports',
        'DelegationPolicy',
        'SubagentContextMode',
        'queue_timing',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/session/session_manager.rs',
      contracts: [
        'clone_prompt_cache',
        'start_dialog_turn_with_existing_context',
        'start_dialog_turn_with_existing_context_persists_turn_and_snapshot',
        'clone_prompt_cache_copies_runtime_and_persisted_entries',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs',
      contracts: [
        'build_truncation_recovery_notice',
        'truncation_notice_for_interactive_tools_does_not_claim_file_write',
        'truncation_notice_for_write_tools_keeps_write_continuation_guidance',
        'denied_tool_messages',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/tools/restrictions.rs',
      contracts: ['denied_tool_messages', 'custom_deny_message_overrides_generic_runtime_error'],
    },
    {
      path: 'src/crates/core/src/agentic/tools/tool_result_storage.rs',
      contracts: ['write_once', 'file\\.flush\\(\\)\\.await'],
    },
    {
      path: 'src/crates/services-integrations/src/mcp/server/connection.rs',
      contracts: [
        'send_request_with_id',
        'initialize_timeout',
        'notifications/initialized',
        'pending\\.clear\\(\\)',
        'local_tool_calls_do_not_inherit_initialize_timeout',
        'local_initialize_uses_initialize_timeout',
      ],
    },
    {
      path: 'src/crates/services-integrations/src/mcp/protocol/transport.rs',
      contracts: ['send_request_with_id', '\\.flush\\(\\)\\s*\\.await'],
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
        'CollapsedToolUsageError',
        'ToolExecutionAccessError',
        'validate_tool_allowed_by_list',
        'validate_collapsed_tool_usage',
        'sort_tool_manifest_definitions',
        'is_tool_collapsed',
        'get_collapsed_tool_names',
      ],
    },
    {
      path: 'src/crates/agent-tools/src/file_guidance.rs',
      contracts: [
        'FILE_TOOL_GUIDANCE_PREFIX',
        'file_tool_guidance_message',
        'is_file_tool_guidance_message',
      ],
    },
    {
      path: 'src/crates/agent-tools/src/file_read_freshness.rs',
      contracts: [
        'FileReadFreshnessFacts',
        'normalize_tool_file_content',
        'file_read_facts_content_matches',
        'file_read_facts_are_fresh',
      ],
    },
    {
      path: 'src/crates/agent-tools/src/tool_result_storage.rs',
      contracts: [
        'ToolResultStoragePolicy',
        'PersistedToolOutput',
        'ToolResultPersistenceCandidate',
        'select_tool_result_indices_for_persistence',
        'sanitize_tool_result_file_component',
        'generate_tool_result_preview',
        'count_tool_result_lines',
        'tool_result_is_persisted_output',
        'build_persisted_tool_output_message',
      ],
    },
    {
      path: 'src/crates/agent-tools/src/tool_execution_presentation.rs',
      contracts: [
        'TOOL_ERROR_ARGUMENTS_PREVIEW_BYTES',
        'USER_STEERING_INTERRUPTED_MESSAGE',
        'ToolExecutionErrorPresentation',
        'render_tool_result_for_assistant',
        'truncate_tool_arguments_preview',
        'truncate_raw_tool_arguments_preview',
        'build_tool_execution_error_presentation',
        'build_user_steering_interrupted_presentation',
        'build_invalid_tool_call_error_message',
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
        'DialogTriggerSource',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/coordination/scheduler.rs',
      contracts: [
        'AgentSessionReplyRoute',
        'DialogQueuePriority',
        'DialogSessionStateFact',
        'DialogSteerOutcome',
        'DialogSubmissionPolicy',
        'DialogSubmitOutcome',
        'DialogSubmitQueueAction',
        'DialogSubmitQueueFacts',
        'DialogTurnOutcomeKind',
        'dialog_policy_may_preempt',
        'resolve_dialog_submit_queue_action',
        'should_skip_agent_session_reply_contract',
        'should_suppress_agent_session_cancelled_reply_contract',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/round_preempt.rs',
      contracts: [
        'bitfun_runtime_ports',
        'DialogRoundInjectionSource',
        'DialogRoundPreemptSource',
        'RoundInjection',
        'RoundInjectionKind',
        'RoundInjectionTarget',
        'SessionRoundInjectionBuffer',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/goal_mode/mod.rs',
      contracts: [
        'bitfun_runtime_ports',
        'SetThreadGoalResult',
        'ThreadGoal',
        'ThreadGoalContinuationPlan',
        'ThreadGoalStatus',
        'ThreadGoalToolResponse',
        'THREAD_GOAL_METADATA_KEY',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/core/message.rs',
      contracts: ['bitfun_runtime_ports', 'CompressionContract', 'CompressionContractItem'],
    },
    {
      path: 'src/crates/core/src/service/workspace/manager.rs',
      contracts: ['bitfun_runtime_ports', 'RelatedPath'],
    },
    {
      path: 'src/crates/core/src/service_agent_runtime.rs',
      contracts: [
        'CoreServiceAgentRuntime',
        'remote_dialog_host',
        'remote_cancel_host',
        'remote_image_context',
        'load_remote_model_catalog',
        'RemoteModelCatalogFacts',
        'RemoteModelCapabilityFact',
        'RemoteReasoningModeFact',
        'build_remote_model_catalog',
        'update_remote_session_model',
        'normalize_remote_session_model_id',
        'normalize_remote_session_model_id_contract',
        'normalize_remote_model_selection',
        'normalize_remote_model_selection_contract',
        'remote_chat_messages_from_turns',
        'RemoteDialogSchedulerOutcomeFact',
        'remote_dialog_submit_outcome_from_scheduler',
        'RemoteChatHistoryTurn',
        'build_remote_chat_messages',
        'strip_remote_user_input_tags',
        'compress_remote_chat_data_url_for_mobile',
        'load_remote_chat_messages',
        'agent_submission_port',
        'agent_turn_cancellation_port',
        'remote_control_state_port',
        'CoreRemoteDialogRuntimeHost',
        'CoreRemoteCancelRuntimeHost',
        'CoreRemoteWorkspaceFileRuntimeHost',
        'CoreRemoteWorkspaceRuntimeHost',
        'CoreRemoteSessionRuntimeHost',
        'CoreRemotePollRuntimeHost',
        'CoreRemoteInteractionRuntimeHost',
        'CoreRemoteSessionTrackerHost',
        'RemoteExecutionDispatcher',
        'ImageContextData',
        'RemoteImageContextAdapter',
        'AgentSubmissionPort',
        'AgentTurnCancellationPort',
        'RemoteControlStatePort',
        'SessionTranscriptReader',
        'core_service_agent_runtime_owner_keeps_coordinator_port_contracts',
        'core_service_agent_runtime_owner_normalizes_remote_session_model_ids',
        'core_service_agent_runtime_owner_normalizes_remote_model_selection_aliases',
        'core_service_agent_runtime_owner_preserves_remote_chat_history_shape',
        'core_service_agent_runtime_owner_skips_in_progress_remote_assistant_history',
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
        'RemoteCancelTaskRequest',
        'RemoteCancelRuntimeHost',
        'cancel_remote_task',
        'RemoteChatHistoryTurn',
        'RemoteChatHistoryRound',
        'RemoteChatHistoryToolItem',
        'build_remote_chat_messages',
        'REMOTE_FILE_MAX_READ_BYTES',
        'REMOTE_FILE_MAX_CHUNK_BYTES',
        'resolve_remote_file_chunk_range',
        'remote_file_display_name',
        'RemoteWorkspaceFacts',
        'RemoteSessionMetadata',
        'remote_workspace_info_response',
        'remote_recent_workspaces_response',
        'remote_assistant_list_response',
        'RemoteWorkspaceRuntimeHost',
        'handle_remote_workspace_command',
        'remote_workspace_handler_preserves_response_shapes',
        'RemoteInitialSyncRuntimeHost',
        'generate_remote_initial_sync',
        'remote_session_info',
        'remote_session_list_response',
        'remote_initial_sync_response',
        'remote_messages_response',
        'RemoteSessionRuntimeHost',
        'handle_remote_session_command',
        'remote_session_handler_preserves_list_and_create_policy',
        'remote_session_handler_removes_tracker_after_delete_success',
        'RemotePollRuntimeHost',
        'handle_remote_poll_command',
        'remote_poll_handler_preserves_missing_workspace_error',
        'RemoteInteractionRuntimeHost',
        'handle_remote_interaction_command',
        'remote_interaction_handler_preserves_default_reject_reason',
        'RemoteDefaultModelsConfig',
        'RemoteModelConfig',
        'RemoteModelCatalog',
        'RemoteModelCapabilityFact',
        'RemoteReasoningModeFact',
        'RemoteModelFacts',
        'RemoteModelCatalogFacts',
        'build_remote_model_catalog',
        'RemoteModelCatalogPollDelta',
        'normalize_remote_session_model_id',
        'normalize_remote_model_selection',
        'remote_model_selection_needs_config',
        'RemoteDialogSchedulerOutcomeFact',
        'remote_dialog_submit_outcome_from_scheduler',
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
        'remote_connect_model_catalog_builder_preserves_config_shape',
        'remote_connect_model_selection_policy_owns_alias_and_config_reference_rules',
        'remote_connect_poll_helpers_preserve_delta_and_completion_policy',
        'remote_connect_image_context_policy_preserves_legacy_fallback_shape',
        'remote_connect_image_context_policy_prefers_explicit_contexts',
        'remote_connect_cancel_and_restore_policy_preserve_runtime_decisions',
        'remote_connect_dialog_submit_outcome_builder_preserves_scheduler_shape',
        'remote_chat_history_assembly_preserves_message_shape_and_item_order',
        'remote_chat_history_assembly_skips_in_progress_assistant_history',
        'remote_connect_file_transfer_policy_preserves_limits_and_chunk_ranges',
        'remote_connect_file_transfer_policy_preserves_name_fallback',
        'remote_connect_tracker_keeps_finished_turn_snapshot_until_persistence_finalizes',
        'remote_connect_tracker_registry_owns_lifecycle_without_core_state',
        'remote_connect_tracker_ignores_unrelated_direct_session_events',
        'remote_connect_tool_preview_slimming_keeps_short_fields_and_drops_large_strings',
        'remote_connect_workspace_response_helpers_own_wire_shape',
        'remote_connect_session_response_helpers_own_pagination_and_timestamps',
      ],
    },
    {
      path: 'src/crates/core/src/service/remote_connect/remote_server.rs',
      contracts: [
        'CoreServiceAgentRuntime',
        'remote_image_context',
        'handle_remote_workspace_command',
        'handle_remote_session_command',
        'generate_remote_initial_sync',
        'handle_remote_poll_command',
        'handle_remote_interaction_command',
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
      path: 'src/crates/core/src/agentic/tools/product_runtime/get_tool_spec_tool.rs',
      contracts: [
        'GetToolSpecTool',
        'build_collapsed_tools_context_section',
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
        'pub use crate::agentic::tools::tool_context_runtime::ToolUseContext',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/tools/tool_context_runtime.rs',
      contracts: [
        'pub struct ToolUseContext',
        'to_tool_context_facts',
        'impl PortableToolContextProvider for ToolUseContext',
        'tool_context_facts_omit_runtime_owner_fields_even_when_context_is_populated',
        'customData',
        'cancellationToken',
        'unlocked_collapsed_tools',
        'impl ToolUseContext',
        'record_light_checkpoint',
        'call_with_tool_runtime_hooks',
        'call_tool_with_runtime_hooks',
        'call_records_deep_review_read_file_measurement_without_touching_result',
        'build_tool_use_context_for_task',
        'build_tool_description_context',
        'build_write_preflight_context',
        'ensure_current_workspace_runtime',
        'resolve_tool_path',
        'enforce_path_operation',
        'workspace_path_resolution_rejects_absolute_paths_outside_remote_workspace',
        'runtime_uri_resolution_rejects_different_workspace_scope',
        'path_policy_allows_only_configured_local_roots',
        'tool_call_runtime_hook_returns_cancelled_before_impl_completes',
        'tool_task_context_materialization_preserves_runtime_fields',
        'tool_description_context_preserves_manifest_custom_data_shape',
        'write_preflight_context_preserves_minimal_runtime_fields',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs',
      contracts: [
        'validate_collapsed_tool_usage',
        'unlocked_collapsed_tools',
        'GetToolSpec',
        'render_tool_result_for_assistant',
        'build_tool_execution_error_presentation',
        'build_user_steering_interrupted_presentation',
        'build_invalid_tool_call_error_message',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/execution/execution_engine.rs',
      contracts: [
        'collect_product_unlocked_collapsed_tools',
        'unlocked_collapsed_tools',
        'collapsed_tool_names',
        'GetToolSpec',
        'citation_renumber',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/tools/product_runtime/unlock_state.rs',
      contracts: [
        'collect_product_unlocked_collapsed_tools',
        'GetToolSpecLoadObservation',
        'collect_loaded_collapsed_tool_names',
        'product_unlock_state_dedupes_and_filters_runtime_unlocks',
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
      contracts: [
        'fork_context',
        'SubagentContextMode::Fork',
        'delegation_policy\\(\\)\\.spawn_child\\(\\)',
        'run_in_background',
        'start_background_subagent',
        'background_task_id',
        'Background \\{\\} started successfully',
        '<background_task status=\\\\"started\\\\"',
        'background_subagent_start_acknowledgement_keeps_structured_task_marker',
      ],
    },
    {
      path: 'src/crates/core/src/agentic/coordination/scheduler.rs',
      contracts: [
        'deliver_background_result',
        'BackgroundResult',
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
      path: 'src/crates/core/src/service/search/mod.rs',
      contracts: ['mod remote_disabled', 'feature = "ssh-remote"', 'pub use remote_disabled'],
    },
    {
      path: 'src/crates/core/src/service/search/remote_disabled.rs',
      contracts: ['Remote SSH search is disabled', 'RemoteWorkspaceSearchService', 'remote_workspace_search_service_for_path'],
    },
    {
      path: 'src/crates/core/Cargo.toml',
      contracts: [
        'bitfun-tool-packs = \\{ path = "\\.\\.\\/tool-packs", default-features = false, optional = true \\}',
        'bitfun-services-integrations = \\{ path = "\\.\\.\\/services-integrations", default-features = false, features = \\["remote-ssh"\\] \\}',
        'bitfun-product-domains = \\{ path = "\\.\\.\\/product-domains", default-features = false, optional = true \\}',
        'dep:bitfun-tool-packs',
        'bitfun-tool-packs\\/product-full',
        'bitfun-services-integrations\\/product-full',
        'dep:bitfun-product-domains',
        'bitfun-product-domains\\/product-full',
      ],
    },
    {
      path: 'src/crates/core/src/lib.rs',
      contracts: [
        'feature = "product-full"',
        'pub mod agentic',
        'feature = "product-domains"',
        'pub mod function_agents',
        'pub mod miniapp',
        'feature = "service-integrations"',
        'service_agent_runtime',
      ],
    },
    {
      path: 'src/crates/core/src/service/mod.rs',
      contracts: [
        'feature = "service-integrations"',
        'pub mod git',
        'pub mod mcp',
        'pub mod remote_connect',
        'pub mod review_platform',
        'feature = "product-full"',
        'pub mod snapshot',
      ],
    },
    {
      path: 'src/crates/core/src/service/config/mod.rs',
      contracts: ['feature = "product-full"', 'mode_config_canonicalizer'],
    },
    {
      path: 'src/crates/core/src/service/workspace/manager.rs',
      contracts: ['feature = "service-integrations"', 'GitService', 'return None'],
    },
    {
      path: 'src/crates/core/src/service/workspace_runtime/service.rs',
      contracts: ['feature = "product-full"', 'WorkspaceBinding', 'ensure_runtime_for_workspace_binding'],
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
        'build_builtin_seed_meta',
        'preserved_builtin_created_at',
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
        'preserved_builtin_created_at',
        'build_builtin_seed_meta',
      ],
    },
    {
      path: 'src/crates/core/src/miniapp/host_dispatch.rs',
      contracts: [
        'dispatch_host',
        'split_host_method',
        'dispatch_fs',
        'fs_method_access_mode',
        'fs_policy_scopes',
        'fs_resolved_path_allowed',
        'dispatch_shell',
        'shell_exec_first_token',
        'shell_exec_input_is_empty',
        'shell_exec_cwd',
        'shell_exec_timeout_ms',
        'shell_exec_default_env',
        'command_basename_allowed',
        'host_allowed_by_allowlist',
      ],
    },
    {
      path: 'src/crates/core/src/miniapp/js_worker_pool.rs',
      contracts: [
        'MiniAppRuntimePort',
        'plan_install_deps',
        'worker_is_idle',
        'worker_pool_at_capacity',
        'select_lru_worker',
      ],
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
      path: 'src/crates/core/src/service/remote_ssh/mod.rs',
      contracts: ['mod disabled', 'pub mod manager', 'pub mod remote_fs', 'pub mod remote_terminal', 'pub mod workspace_state'],
    },
    {
      path: 'src/crates/core/src/service/remote_ssh/disabled.rs',
      contracts: ['Remote SSH support is disabled', 'SSHConnectionManager', 'RemoteFileService', 'RemoteTerminalManager'],
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
        'persist_import_runtime_state',
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
        'MiniAppCreateInput',
        'MiniAppUpdatePatch',
        'build_created_app',
        'apply_update_patch',
        'prepare_draft_app',
        'apply_draft_source_sync_result',
        'apply_draft_permission_update_result',
        'apply_draft_to_active',
        'mark_deps_installed_state',
        'clear_worker_restart_required_state',
        'prepare_rollback_app',
        'apply_recompile_result',
        'apply_sync_from_fs_result',
        'apply_import_runtime_state',
        'prepare_imported_meta',
      ],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/draft.rs',
      contracts: ['MiniAppDraftManifest', 'MiniAppDraft', 'build_draft_manifest', 'build_draft_response'],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/runtime.rs',
      contracts: [
        'runtime_lookup_order',
        'detect_runtime',
        'DefaultMiniAppRuntimeProbe',
        'MiniAppRuntimeProbe',
        'detect_runtime_with_probe',
        'which::which',
        'std::fs::read_dir',
        'create_version_command',
        'candidate_executable_path',
        'versioned_executable_candidate',
      ],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/worker.rs',
      contracts: [
        'InstallDepsPlan',
        'plan_install_deps',
        'worker_pool_capacity',
        'worker_idle_timeout_ms',
        'worker_is_idle',
        'select_lru_worker',
      ],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/host_routing.rs',
      contracts: [
        'split_host_method',
        'FsAccessMode',
        'fs_method_access_mode',
        'fs_policy_scopes',
        'fs_resolved_path_allowed',
        'command_basename_for_allowlist',
        'command_basename_allowed',
        'host_allowed_by_allowlist',
        'shell_exec_first_token',
        'shell_exec_input_is_empty',
        'shell_exec_cwd',
        'shell_exec_timeout_ms',
        'shell_exec_default_env',
      ],
    },
    {
      path: 'src/crates/product-domains/src/miniapp/exporter.rs',
      contracts: ['MISSING_JS_RUNTIME_MESSAGE', 'export_runtime_label', 'build_export_check_result'],
    },
    {
      path: 'src/crates/core/src/miniapp/exporter.rs',
      contracts: ['detect_runtime', 'build_export_check_result', 'Export not yet implemented'],
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
        'build_created_app',
        'apply_update_patch',
        'prepare_draft_app',
        'apply_draft_to_active',
        'persist_sync_from_fs_result_for_app',
        'compile_source',
        'REQUIRED_SOURCE_FILES',
        'MiniAppImportLayout',
        'build_import_fallbacks',
        'prepare_imported_meta',
        'persist_import_runtime_state',
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
      contracts: ['pub use bitfun_product_domains::miniapp::runtime::{', 'detect_runtime'],
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
    'RemoteCancelTaskRequest',
    'RemoteCancelRuntimeHost',
    'cancel_remote_task',
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
    'remote_file_content_response',
    'remote_file_chunk_response',
    'remote_file_info_response',
    'handle_remote_workspace_file_command',
    'general_purpose::STANDARD\\.encode',
    'remote_dialog_submit_response',
    'remote_task_cancel_response',
    'remote_interaction_accepted_response',
    'remote_answer_question_response',
    'remote_workspace_info_response',
    'remote_recent_workspaces_response',
    'remote_assistant_list_response',
    'remote_workspace_updated_response',
    'remote_assistant_updated_response',
    'remote_session_info',
    'remote_session_list_response',
    'remote_initial_sync_response',
    'remote_session_created_response',
    'remote_session_model_updated_response',
    'remote_messages_response',
    'remote_session_deleted_response',
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

function featureReferencesDependency(feature, depName) {
  if (!feature) {
    return false;
  }
  return feature.refs.includes(`dep:${depName}`) || feature.refs.includes(depName);
}

function featureReferencesFeature(feature, featureName) {
  if (!feature) {
    return false;
  }
  return feature.refs.includes(featureName);
}

function checkOptionalDependencyFeatureOwners(crateDir, rule) {
  const manifestPath = join(crateDir, 'Cargo.toml');
  const lines = readText(manifestPath).split(/\r?\n/);
  const deps = parseManifestDependencies(lines);
  const depsByName = new Map(deps.map((dep) => [dep.name, dep]));
  const features = parseManifestFeatures(lines);
  const declaredOwnerDeps = new Set(rule.dependencies.map((dependency) => dependency.depName));

  for (const dependency of rule.dependencies) {
    const dep = depsByName.get(dependency.depName);
    if (!dep) {
      failures.push({
        path: manifestPath,
        line: 1,
        message: `${rule.reason}; missing optional dependency: ${dependency.depName}`,
      });
      continue;
    }
    if (!dep.optional) {
      failures.push({
        path: manifestPath,
        line: dep.line,
        message: `${rule.reason}; dependency must be optional: ${dependency.depName}`,
      });
    }
    for (const featureName of dependency.ownerFeatures) {
      const feature = features.get(featureName);
      if (!feature) {
        failures.push({
          path: manifestPath,
          line: dep?.line ?? 1,
          message: `${rule.reason}; missing owner feature ${featureName} for ${dependency.depName}`,
        });
        continue;
      }
      if (!featureReferencesDependency(feature, dependency.depName)) {
        failures.push({
          path: manifestPath,
          line: feature.line,
          message: `${rule.reason}; ${featureName} must explicitly enable ${dependency.depName}`,
        });
      }
    }
  }

  const profileRule = dependencyProfileRules.find((profile) => profile.crateName === rule.crateName);
  const depsRequiringOwner = new Set(profileRule?.forbiddenNonOptionalDeps ?? []);
  const uncoveredDeps = new Map();
  for (const dep of deps) {
    if (!dep.optional || !depsRequiringOwner.has(dep.name) || declaredOwnerDeps.has(dep.name)) {
      continue;
    }
    if (!uncoveredDeps.has(dep.name)) {
      uncoveredDeps.set(dep.name, dep);
    }
  }
  for (const [depName, dep] of uncoveredDeps.entries()) {
    failures.push({
      path: manifestPath,
      line: dep.line,
      message: `${rule.reason}; optional runtime dependency must declare owner feature coverage: ${depName}`,
    });
  }
}

function checkProductCoreFeatureAssembly(rule) {
  const manifestPath = join(ROOT, ...rule.manifestPath.split('/'));
  const deps = parseManifestDependencies(readText(manifestPath).split(/\r?\n/));
  const dep = deps.find((candidate) => candidate.name === rule.dependencyName);
  if (!dep) {
    failures.push({
      path: manifestPath,
      line: 1,
      message: `${rule.reason}; missing dependency: ${rule.dependencyName}`,
    });
    return;
  }

  if (!manifestDependencyDisablesDefaultFeatures(dep)) {
    failures.push({
      path: manifestPath,
      line: dep.line,
      message: `${rule.reason}; ${rule.dependencyName} must set default-features = false`,
    });
  }

  const enabledFeatures = parseManifestDependencyFeatureNames(dep);
  for (const featureName of rule.requiredFeatures) {
    if (!enabledFeatures.has(featureName)) {
      failures.push({
        path: manifestPath,
        line: dep.line,
        message: `${rule.reason}; ${rule.dependencyName} must enable feature ${featureName}`,
      });
    }
  }
}

function checkProductCoreFeatureAssemblyCoverage() {
  const rulePaths = new Set(productCoreFeatureAssemblyRules.map((rule) => rule.manifestPath));
  for (const manifestPath of collectProductCoreDependencyManifests()) {
    if (!rulePaths.has(manifestPath)) {
      failures.push({
        path: join(ROOT, ...manifestPath.split('/')),
        line: 1,
        message:
          'product entry crate depends on bitfun-core but is not covered by product-full assembly rules',
      });
    }
  }
}

function checkCoreDefaultProductFullFeature() {
  const manifestPath = join(ROOT, 'src', 'crates', 'core', 'Cargo.toml');
  const features = parseManifestFeatures(readText(manifestPath).split(/\r?\n/));
  if (!featureReferencesFeature(features.get('default'), 'product-full')) {
    failures.push({
      path: manifestPath,
      line: features.get('default')?.line ?? 1,
      message:
        'bitfun-core default feature must remain product-full until a separate product matrix review changes it',
    });
  }
}

function checkCoreProductFullFeatureAssembly(rule) {
  const manifestPath = join(ROOT, ...rule.manifestPath.split('/'));
  const features = parseManifestFeatures(readText(manifestPath).split(/\r?\n/));
  const productFull = features.get(rule.featureName);
  if (!productFull) {
    failures.push({
      path: manifestPath,
      line: 1,
      message: `${rule.reason}; missing ${rule.featureName} feature declaration`,
    });
    return;
  }
  for (const featureName of rule.requiredFeatureRefs) {
    if (!featureReferencesFeature(productFull, featureName)) {
      failures.push({
        path: manifestPath,
        line: productFull.line,
        message: `${rule.reason}; ${rule.featureName} must explicitly enable ${featureName}`,
      });
    }
  }
}

function checkOwnerCrateFeatureAssembly(rule) {
  const manifestPath = join(ROOT, ...rule.manifestPath.split('/'));
  const features = parseManifestFeatures(readText(manifestPath).split(/\r?\n/));
  const allowedProductFullFeatures = new Set(rule.requiredProductFullFeatures);
  const defaultFeature = features.get('default');
  if (!defaultFeature) {
    failures.push({
      path: manifestPath,
      line: 1,
      message: `${rule.reason}; missing default feature declaration`,
    });
  } else if (defaultFeature.refs.length > 0) {
    failures.push({
      path: manifestPath,
      line: defaultFeature.line,
      message: `${rule.reason}; default feature must remain empty`,
    });
  }

  const productFull = features.get('product-full');
  if (!productFull) {
    failures.push({
      path: manifestPath,
      line: 1,
      message: `${rule.reason}; missing product-full feature declaration`,
    });
    return;
  }

  for (const featureName of rule.requiredProductFullFeatures) {
    if (!featureReferencesFeature(productFull, featureName)) {
      failures.push({
        path: manifestPath,
        line: productFull.line,
        message: `${rule.reason}; product-full must explicitly enable ${featureName}`,
      });
    }
  }
  for (const featureName of productFull.refs) {
    if (!allowedProductFullFeatures.has(featureName)) {
      failures.push({
        path: manifestPath,
        line: productFull.line,
        message: `${rule.reason}; product-full must not include undeclared feature group ${featureName}`,
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
    const repoPath = toRepoPath(path);
    const lines = readText(path).split(/\r?\n/);
    lines.forEach((line, index) => {
      for (const pattern of patterns) {
        if (pattern.allowPaths?.includes(repoPath)) {
          continue;
        }
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

for (const rule of optionalDependencyFeatureOwnerRules) {
  const crateDir = join(ROOT, 'src', 'crates', rule.crateName);
  checkOptionalDependencyFeatureOwners(crateDir, rule);
}

for (const rule of productCoreFeatureAssemblyRules) {
  checkProductCoreFeatureAssembly(rule);
}
checkProductCoreFeatureAssemblyCoverage();
checkCoreDefaultProductFullFeature();
checkCoreProductFullFeatureAssembly(coreProductFullFeatureAssemblyRule);
for (const rule of ownerCrateFeatureAssemblyRules) {
  checkOwnerCrateFeatureAssembly(rule);
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
