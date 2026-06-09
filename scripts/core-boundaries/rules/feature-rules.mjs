// Boundary rules for feature assembly and optional dependency ownership.

export const optionalDependencyFeatureOwnerRules = [
  {
    crateName: 'core',
    reason:
      'bitfun-core product/runtime optional dependencies must stay owned by explicit feature gates',
    dependencies: [
      { depName: 'aes', ownerFeatures: ['service-integrations'] },
      { depName: 'aes-gcm', ownerFeatures: ['service-integrations'] },
      { depName: 'axum', ownerFeatures: ['service-integrations'] },
      { depName: 'bitfun-ai-adapters', ownerFeatures: ['ai-adapter-runtime'] },
      { depName: 'bitfun-product-capabilities', ownerFeatures: ['product-capabilities'] },
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
      { depName: 'rand', ownerFeatures: ['service-integrations'] },
      { depName: 'reqwest', ownerFeatures: ['ai-adapter-runtime', 'service-integrations'] },
      { depName: 'rmcp', ownerFeatures: ['service-integrations'] },
      { depName: 'russh', ownerFeatures: ['ssh-remote'] },
      { depName: 'similar', ownerFeatures: ['product-full'] },
      { depName: 'sse-stream', ownerFeatures: ['service-integrations'] },
      { depName: 'tokio-tungstenite', ownerFeatures: ['service-integrations'] },
      { depName: 'tower-http', ownerFeatures: ['service-integrations'] },
      { depName: 'tool-runtime', ownerFeatures: ['product-full'] },
      { depName: 'x25519-dalek', ownerFeatures: ['service-integrations'] },
    ],
  },
  {
    crateName: 'services-integrations',
    reason:
      'services-integrations optional runtime dependencies must stay owned by explicit integration features',
    dependencies: [
      { depName: 'aes-gcm', ownerFeatures: ['mcp', 'remote-ssh-concrete'] },
      { depName: 'anyhow', ownerFeatures: ['mcp', 'remote-ssh-concrete'] },
      {
        depName: 'async-trait',
        ownerFeatures: ['mcp', 'remote-connect', 'remote-ssh-concrete', 'workspace-search'],
      },
      {
        depName: 'base64',
        ownerFeatures: ['mcp', 'miniapp-runtime', 'remote-connect', 'remote-ssh-concrete'],
      },
      { depName: 'bitfun-agent-runtime', ownerFeatures: ['deep-research'] },
      { depName: 'bitfun-product-domains', ownerFeatures: ['function-agents', 'miniapp-runtime'] },
      { depName: 'bitfun-runtime-ports', ownerFeatures: ['remote-connect'] },
      {
        depName: 'bitfun-services-core',
        ownerFeatures: ['git', 'mcp', 'miniapp-runtime', 'workspace-search'],
      },
      { depName: 'chrono', ownerFeatures: ['git', 'remote-ssh-concrete'] },
      { depName: 'dirs', ownerFeatures: ['miniapp-runtime', 'remote-ssh-concrete'] },
      { depName: 'dunce', ownerFeatures: ['remote-ssh', 'workspace-search'] },
      { depName: 'futures', ownerFeatures: ['mcp'] },
      { depName: 'git2', ownerFeatures: ['git'] },
      { depName: 'notify', ownerFeatures: ['file-watch'] },
      { depName: 'rand', ownerFeatures: ['mcp', 'remote-ssh-concrete'] },
      { depName: 'reqwest', ownerFeatures: ['mcp', 'miniapp-runtime'] },
      { depName: 'rmcp', ownerFeatures: ['mcp'] },
      { depName: 'russh', ownerFeatures: ['remote-ssh-concrete'] },
      { depName: 'russh-keys', ownerFeatures: ['remote-ssh-concrete'] },
      { depName: 'russh-sftp', ownerFeatures: ['remote-ssh-concrete'] },
      { depName: 'sha2', ownerFeatures: ['remote-ssh'] },
      { depName: 'shellexpand', ownerFeatures: ['remote-ssh-concrete'] },
      { depName: 'sse-stream', ownerFeatures: ['mcp'] },
      { depName: 'ssh_config', ownerFeatures: ['remote-ssh-concrete', 'ssh_config'] },
      { depName: 'terminal-core', ownerFeatures: ['remote-ssh-concrete'] },
      { depName: 'thiserror', ownerFeatures: ['git', 'remote-ssh-concrete', 'workspace-search'] },
      { depName: 'tokio-util', ownerFeatures: ['remote-ssh'] },
      { depName: 'uuid', ownerFeatures: ['miniapp-runtime', 'remote-connect', 'remote-ssh-concrete'] },
      { depName: 'which', ownerFeatures: ['miniapp-runtime', 'workspace-search'] },
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

export const productCoreFeatureAssemblyRules = [
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
    manifestPath: 'src/crates/interfaces/acp/Cargo.toml',
    dependencyName: 'bitfun-core',
    requiredFeatures: ['product-full'],
    reason: 'ACP must explicitly assemble the full bitfun-core product runtime',
  },
];

export const productCoreFeatureAssemblyScanRoots = [
  'src/apps',
  'src/crates/interfaces/acp',
];

export const coreProductFullFeatureAssemblyRule = {
  manifestPath: 'src/crates/assembly/core/Cargo.toml',
  featureName: 'product-full',
  requiredFeatureRefs: [
    'ssh-remote',
    'product-capabilities',
    'product-domains',
    'service-integrations',
    'tool-packs',
  ],
  reason: 'bitfun-core product-full must explicitly assemble current owner feature groups',
};

export const ownerCrateFeatureAssemblyRules = [
  {
    manifestPath: 'src/crates/execution/tool-provider-groups/Cargo.toml',
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
    manifestPath: 'src/crates/services/services-integrations/Cargo.toml',
    reason: 'services-integrations must keep integration feature groups explicit and default-light',
    requiredProductFullFeatures: [
      'announcement',
      'deep-research',
      'file-watch',
      'function-agents',
      'git',
      'miniapp-runtime',
      'mcp',
      'remote-connect',
      'remote-ssh',
      'remote-ssh-concrete',
      'workspace-search',
    ],
  },
  {
    manifestPath: 'src/crates/contracts/product-domains/Cargo.toml',
    reason: 'product-domains must keep product domain feature groups explicit and default-light',
    requiredProductFullFeatures: ['miniapp', 'function-agents'],
  },
];
