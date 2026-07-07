// Physical crate layout rules. Package names remain stable; this file only
// owns where workspace crates live under src/crates.

export const crateLayoutRules = [
  { crateName: 'core-types', layer: 'contracts', path: 'src/crates/contracts/core-types' },
  { crateName: 'events', layer: 'contracts', path: 'src/crates/contracts/events' },
  { crateName: 'product-domains', layer: 'contracts', path: 'src/crates/contracts/product-domains' },
  { crateName: 'runtime-ports', layer: 'contracts', path: 'src/crates/contracts/runtime-ports' },

  { crateName: 'agent-runtime', layer: 'execution', path: 'src/crates/execution/agent-runtime' },
  { crateName: 'agent-stream', layer: 'execution', path: 'src/crates/execution/agent-stream' },
  { crateName: 'agent-tools', layer: 'execution', path: 'src/crates/execution/tool-contracts' },
  { crateName: 'harness', layer: 'execution', path: 'src/crates/execution/harness' },
  { crateName: 'runtime-services', layer: 'execution', path: 'src/crates/execution/runtime-services' },
  { crateName: 'tool-packs', layer: 'execution', path: 'src/crates/execution/tool-provider-groups' },
  { crateName: 'tool-runtime', layer: 'execution', path: 'src/crates/execution/tool-execution' },

  { crateName: 'product-capabilities', layer: 'assembly', path: 'src/crates/assembly/product-capabilities' },

  { crateName: 'services-core', layer: 'services', path: 'src/crates/services/services-core' },
  { crateName: 'services-integrations', layer: 'services', path: 'src/crates/services/services-integrations' },
  { crateName: 'terminal', layer: 'services', path: 'src/crates/services/terminal' },

  { crateName: 'acp', layer: 'interfaces', path: 'src/crates/interfaces/acp' },
  { crateName: 'ai-adapters', layer: 'adapters', path: 'src/crates/adapters/ai-adapters' },
  { crateName: 'api-layer', layer: 'adapters', path: 'src/crates/adapters/api-layer' },
  { crateName: 'opencode-adapter', layer: 'adapters', path: 'src/crates/adapters/opencode-adapter' },
  { crateName: 'transport', layer: 'adapters', path: 'src/crates/adapters/transport' },
  { crateName: 'webdriver', layer: 'adapters', path: 'src/crates/adapters/webdriver' },

  { crateName: 'core', layer: 'assembly', path: 'src/crates/assembly/core' },
];

export const crateLayoutLayerNames = [
  'interfaces',
  'assembly',
  'adapters',
  'services',
  'execution',
  'contracts',
];

const crateLayoutByName = new Map(crateLayoutRules.map((rule) => [rule.crateName, rule]));

export function crateLayoutRuleForName(crateName) {
  return crateLayoutByName.get(crateName);
}

export function cratePathForName(crateName) {
  return crateLayoutRuleForName(crateName)?.path;
}
