// Physical crate layout rules. Package names remain stable; this file only
// owns where workspace crates live under src/crates.

export const crateLayoutRules = [
  { crateName: 'core-types', layer: 'contracts', path: 'src/crates/contracts/core-types' },
  { crateName: 'events', layer: 'contracts', path: 'src/crates/contracts/events' },
  { crateName: 'runtime-ports', layer: 'contracts', path: 'src/crates/contracts/runtime-ports' },

  { crateName: 'agent-runtime', layer: 'execution', path: 'src/crates/execution/agent-runtime' },
  { crateName: 'agent-stream', layer: 'execution', path: 'src/crates/execution/agent-stream' },
  { crateName: 'agent-tools', layer: 'execution', path: 'src/crates/execution/agent-tools' },
  { crateName: 'harness', layer: 'execution', path: 'src/crates/execution/harness' },
  { crateName: 'runtime-services', layer: 'execution', path: 'src/crates/execution/runtime-services' },
  { crateName: 'tool-packs', layer: 'execution', path: 'src/crates/execution/tool-packs' },
  { crateName: 'tool-runtime', layer: 'execution', path: 'src/crates/execution/tool-runtime' },

  { crateName: 'product-capabilities', layer: 'product', path: 'src/crates/product/product-capabilities' },
  { crateName: 'product-domains', layer: 'product', path: 'src/crates/product/product-domains' },

  { crateName: 'services-core', layer: 'services', path: 'src/crates/services/services-core' },
  { crateName: 'services-integrations', layer: 'services', path: 'src/crates/services/services-integrations' },
  { crateName: 'terminal', layer: 'services', path: 'src/crates/services/terminal' },

  { crateName: 'acp', layer: 'surfaces', path: 'src/crates/surfaces/acp' },
  { crateName: 'ai-adapters', layer: 'integrations', path: 'src/crates/integrations/ai-adapters' },
  { crateName: 'api-layer', layer: 'integrations', path: 'src/crates/integrations/api-layer' },
  { crateName: 'transport', layer: 'integrations', path: 'src/crates/integrations/transport' },
  { crateName: 'webdriver', layer: 'integrations', path: 'src/crates/integrations/webdriver' },

  { crateName: 'core', layer: 'facade', path: 'src/crates/facade/core' },
];

export const crateLayoutLayerNames = [
  'surfaces',
  'facade',
  'integrations',
  'services',
  'product',
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
