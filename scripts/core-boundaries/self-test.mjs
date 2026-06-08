// Self-tests for the core boundary checker configuration and parsers.

export function runManifestParserSelfTest({
  isManifestDependencyDeclaration,
  parseManifestDependencies,
  manifestDependencyDisablesDefaultFeatures,
  parseManifestDependencyFeatureNames,
  productCoreFeatureAssemblyRules,
  coreProductFullFeatureAssemblyRule,
  collectProductCoreDependencyManifestPaths,
  ownerCrateFeatureAssemblyRules,
  parseManifestFeatures,
  optionalDependencyFeatureOwnerRules,
  lightweightBoundaryRules,
  dependencyProfileRules,
  noCoreDependencyCrates,
  requiredContentRules,
  forbiddenContentRules,
  forbiddenContentUnderRules,
  facadeOnlyFiles,
  forbiddenRuleTextForPath,
  regexSourceContainsContract,
  createFacadeLineChecker,
  escapeRegex,
}) {
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
    'src/crates/interfaces/acp/Cargo.toml',
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
  for (const featureName of [
    'ssh-remote',
    'product-capabilities',
    'product-domains',
    'service-integrations',
    'tool-packs',
  ]) {
    if (!coreProductFullFeatureAssemblyRule.requiredFeatureRefs.includes(featureName)) {
      throw new Error(`core product-full assembly rule must require ${featureName}`);
    }
  }
  const discoveredProductCoreManifests = collectProductCoreDependencyManifestPaths([
    {
      manifestPath: 'src/apps/desktop/Cargo.toml',
      text:
        '[dependencies]\nbitfun-core = { path = "../../crates/assembly/core", default-features = false, features = ["product-full"] }',
    },
    {
      manifestPath: 'src/apps/server/Cargo.toml',
      text: '[dependencies]\naxum = { workspace = true }',
    },
    {
      manifestPath: 'src/crates/interfaces/acp/Cargo.toml',
      text: '[dependencies."bitfun-core"]\npath = "../../assembly/core"\ndefault-features = false\nfeatures = ["product-full"]',
    },
  ]);
  if (discoveredProductCoreManifests.join(',') !== 'src/apps/desktop/Cargo.toml,src/crates/interfaces/acp/Cargo.toml') {
    throw new Error('product core dependency scanner must discover only manifests that depend on bitfun-core');
  }
  const ownerFeatureRulePaths = new Set(
    ownerCrateFeatureAssemblyRules.map((rule) => rule.manifestPath),
  );
  for (const manifestPath of [
    'src/crates/execution/tool-provider-groups/Cargo.toml',
    'src/crates/services/services-integrations/Cargo.toml',
    'src/crates/contracts/product-domains/Cargo.toml',
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
  const coreToolFrameworkRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/framework.rs',
  );
  if (!coreToolFrameworkRuleText) {
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
  for (const contract of coreToolFrameworkContracts) {
    if (!coreToolFrameworkRuleText.includes(contract)) {
      throw new Error(`core tool framework boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreToolRestrictionRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/restrictions.rs',
  );
  if (!coreToolRestrictionRuleText) {
    throw new Error('missing core tool restrictions boundary rule');
  }
  const coreToolRestrictionContracts = [
    'ToolPathOperation',
    'ToolPathPolicy',
    'ToolRuntimeRestrictions',
    'normalize_absolute_posix_path',
  ];
  for (const contract of coreToolRestrictionContracts) {
    if (!coreToolRestrictionRuleText.includes(contract)) {
      throw new Error(`core tool restrictions boundary rule must forbid contract: ${contract}`);
    }
  }
  const agentToolsFrameworkRule = requiredContentRules.find(
    (rule) => rule.path === 'src/crates/execution/tool-contracts/src/framework.rs',
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
  const coreWorkspacePathRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/workspace_paths.rs',
  );
  if (!coreWorkspacePathRuleText) {
    throw new Error('missing core workspace path boundary rule');
  }
  const coreWorkspacePathContracts = [
    'BITFUN_RUNTIME_URI_PREFIX',
    'ParsedBitFunRuntimeUri',
    'posix_normalize_components',
    'Component::ParentDir',
  ];
  for (const contract of coreWorkspacePathContracts) {
    if (!coreWorkspacePathRuleText.includes(contract)) {
      throw new Error(`core workspace path boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreToolRegistryRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/registry.rs',
  );
  if (!coreToolRegistryRuleText) {
    throw new Error('missing core tool registry boundary rule');
  }
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
  const coreSubagentRuntimeRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/subagent_runtime/mod.rs',
  );
  if (!coreSubagentRuntimeRuleText) {
    throw new Error('missing core subagent runtime boundary rule');
  }
  for (const contract of ['DelegationPolicy', 'SubagentContextMode']) {
    if (!coreSubagentRuntimeRuleText.includes(contract)) {
      throw new Error(`core subagent runtime boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreCoordinatorRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/coordination/coordinator.rs',
  );
  if (!coreCoordinatorRuleText) {
    throw new Error('missing core coordinator boundary rule');
  }
  if (!coreCoordinatorRuleText.includes('DialogTriggerSource')) {
    throw new Error('core coordinator boundary rule must forbid DialogTriggerSource redefinition');
  }
  const coreSchedulerRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/coordination/scheduler.rs',
  );
  if (!coreSchedulerRuleText) {
    throw new Error('missing core scheduler boundary rule');
  }
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
  const coreRoundPreemptRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/round_preempt.rs',
  );
  if (!coreRoundPreemptRuleText) {
    throw new Error('missing core round preempt boundary rule');
  }
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
  const coreGoalModeTypesRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/goal_mode/mod.rs',
  );
  if (!coreGoalModeTypesRuleText) {
    throw new Error('missing core goal mode types boundary rule');
  }
  for (const contract of [
    'GOAL_MODE_METADATA_KEY',
    'MAX_GOAL_CONTINUATIONS',
    'ThreadGoal',
    'ThreadGoalStatus',
    'ThreadGoalToolResponse',
    'ThreadGoalRuntime',
    'build_thread_goal_continuation_plan',
    'goal_tool_response',
  ]) {
    if (!coreGoalModeTypesRuleText.includes(contract)) {
      throw new Error(`core goal mode types boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreMessageRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/core/message.rs',
  );
  if (!coreMessageRuleText) {
    throw new Error('missing core message boundary rule');
  }
  for (const contract of ['CompressionContract', 'CompressionContractItem']) {
    if (!coreMessageRuleText.includes(contract)) {
      throw new Error(`core message boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreWorkspaceRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/service/workspace/manager.rs',
  );
  if (!coreWorkspaceRuleText) {
    throw new Error('missing core workspace manager boundary rule');
  }
  if (!coreWorkspaceRuleText.includes('RelatedPath')) {
    throw new Error('core workspace manager boundary rule must forbid contract: RelatedPath');
  }
  const coreSubagentRuntimeOwnerPathRule = forbiddenContentUnderRules.find(
    (rule) => rule.path === 'src/crates/assembly/core/src',
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
    (rule) => rule.path === 'src/crates/contracts/product-domains/src',
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
      'src/crates/contracts/product-domains/src/miniapp/runtime.rs',
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
  const productCapabilitiesRule = lightweightBoundaryRules.find(
    (rule) => rule.crateName === 'product-capabilities',
  );
  if (!productCapabilitiesRule?.forbiddenDeps.includes('bitfun-core')) {
    throw new Error('product-capabilities lightweight boundary must forbid bitfun-core');
  }
  if (!productCapabilitiesRule?.forbiddenDeps.includes('bitfun-product-domains')) {
    throw new Error(
      'product-capabilities lightweight boundary must forbid product-domain implementations',
    );
  }
  if (!productCapabilitiesRule?.forbiddenDeps.includes('tool-runtime')) {
    throw new Error('product-capabilities lightweight boundary must forbid tool-runtime');
  }
  const productCapabilitiesProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'product-capabilities',
  );
  if (!productCapabilitiesProfile?.forbiddenNonOptionalDeps.includes('bitfun-core')) {
    throw new Error('product-capabilities dependency profile must forbid bitfun-core');
  }
  const agentToolsManifestRule = forbiddenContentUnderRules.find(
    (rule) => rule.path === 'src/crates/execution/tool-contracts/src',
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
    (rule) => rule.path === 'src/crates/execution/tool-provider-groups/src',
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
      path: 'src/crates/contracts/runtime-ports/src/lib.rs',
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
      path: 'src/crates/execution/runtime-services/src/lib.rs',
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
      path: 'src/crates/execution/runtime-services/tests/runtime_services_contracts.rs',
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
      path: 'src/crates/execution/agent-runtime/src/agents.rs',
      contracts: [
        'SubagentQueryContext',
        'SubagentListScope',
        'SubagentVisibilityPolicy',
        'resolve_subagent_default_enabled',
        'resolve_subagent_availability',
        'SubagentOverrideLayers',
        'SubagentStateReason',
        'SHARED_CODING_MODE_CONFIG_PROFILE_ID',
        'resolve_mode_config_profile_id',
        'mode_config_profile_member_mode_ids',
        'mode_presentation_rank',
        'shared_coding_mode_user_context_policy',
        'SubAgentSource',
        'subagent_source_kind',
        'subagent_source_presentation_rank',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/agent_registry_contracts.rs',
      contracts: [
        'visibility_policy_supports_public_restricted_hidden_and_denied_parents',
        'availability_preserves_builtin_project_and_user_override_layering',
        'default_enabled_uses_visibility_only_for_builtin_subagents',
        'shared_coding_modes_resolve_to_the_same_config_profile',
        'subagent_source_contract_preserves_runtime_kind_and_presentation_order',
        'mode_presentation_and_shared_context_policy_match_existing_mode_contract',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/custom_subagent.rs',
      contracts: [
        'CustomSubagentKind',
        'CustomSubagentDiscoveryRoots',
        'CustomSubagentLoadReport',
        'CustomSubagentDefinition',
        'CustomSubagentDefinitionError',
        'DEFAULT_CUSTOM_SUBAGENT_TOOLS',
        'custom_subagent_tools_from_front_matter',
        'custom_subagent_tools_to_front_matter',
        'custom_subagent_readonly_should_save',
        'custom_subagent_review_should_save',
        'custom_subagent_model_should_save',
        'custom_subagent_read_markdown_file',
        'custom_subagent_save_markdown_parts',
        'custom_subagent_possible_dirs',
        'load_custom_subagent_definitions',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/custom_subagent_discovery_contracts.rs',
      contracts: [
        'custom_subagent_discovery_preserves_directory_priority_and_deduplication',
        'custom_subagent_discovery_reports_parse_errors_without_dropping_valid_files',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/custom_subagent_contracts.rs',
      contracts: [
        'custom_subagent_defaults_match_existing_front_matter_contract',
        'custom_subagent_tool_front_matter_keeps_existing_comma_format',
        'custom_subagent_default_fields_are_omitted_when_saved',
        'custom_subagent_definition_from_front_matter_preserves_schema_and_defaults',
        'custom_subagent_definition_reports_legacy_missing_field_errors',
        'custom_subagent_markdown_io_preserves_legacy_front_matter_shape',
        'custom_subagent_markdown_parse_errors_match_legacy_prefixes',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/post_call_hooks.rs',
      contracts: [
        'PostCallHookKind',
        'successful_tool_post_call_hooks',
        'SuccessfulToolPostCallHookExecutor',
        'run_successful_tool_post_call_hooks',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/post_call_hook_contracts.rs',
      contracts: ['successful_tool_call_routes_to_shared_context_measurement_hook'],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/post_call_hook_execution_contracts.rs',
      contracts: ['successful_tool_post_call_executor_runs_deep_review_measurement_route'],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/tool_confirmation.rs',
      contracts: [
        'ToolConfirmationRequestFacts',
        'ToolConfirmationPlan',
        'ToolConfirmationOutcome',
        'ToolConfirmationWaitResult',
        'ConfirmationFailureKind',
        'resolve_tool_confirmation_plan',
        'resolve_confirmation_failure',
        'resolve_confirmation_wait_result',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/tool_confirmation_contracts.rs',
      contracts: [
        'confirmation_plan_requires_permission_only_when_both_flags_are_true',
        'confirmation_plan_preserves_legacy_no_timeout_one_year_deadline',
        'confirmation_failure_mapping_preserves_legacy_reasons_and_errors',
        'confirmation_wait_result_mapping_preserves_legacy_timeout_and_rejection',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/checkpoint.rs',
      contracts: [
        'LightCheckpoint',
        'LightCheckpointWorkspaceFacts',
        'GitStatusCheckpointFacts',
        'build_light_checkpoint',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/scheduler.rs',
      contracts: [
        'DEFAULT_MAX_DIALOG_QUEUE_DEPTH',
        'ActiveDialogTurn',
        'ActiveDialogTurnStore',
        'AgentSessionReplyAction',
        'AgentSessionReplyPlan',
        'BackgroundDeliveryFacts',
        'BackgroundDeliveryAction',
        'BackgroundInjectionKind',
        'DialogReplySuppressionSet',
        'DialogSteeringAction',
        'DialogTurnQueue',
        'SessionAbortFlags',
        'resolve_agent_session_reply_action',
        'resolve_background_delivery_action',
        'resolve_background_delivery_injection',
        'resolve_dialog_steering_action',
        'follow_up_submission_policy',
        'SubmitAgentSessionFollowUp',
        'InjectIntoRunningTurn',
        'SessionRoundYieldFlags',
        'SessionRoundInjectionBuffer',
        'TurnOutcome',
        'TurnOutcomeQueueAction',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/scheduler_contracts.rs',
      contracts: [
        'background_delivery_injects_when_session_is_processing',
        'background_delivery_starts_agent_session_follow_up_when_session_is_not_processing',
        'background_delivery_follow_up_uses_agent_session_source_semantics',
        'background_delivery_injection_does_not_expose_follow_up_policy',
        'background_delivery_injection_builds_thread_goal_current_turn_message',
        'background_delivery_injection_builds_background_result_with_display_fallback',
        'dialog_turn_queue_preserves_priority_order_and_fifo_within_priority',
        'dialog_turn_queue_rejects_overflow_and_preserves_current_error_shape',
        'dialog_turn_queue_requeued_turn_keeps_original_priority_for_later_ordering',
        'active_dialog_turn_owns_agent_session_reply_suppression_facts',
        'active_dialog_turn_store_owns_suppression_key_resolution_and_removal',
        'reply_suppression_set_marks_takes_and_clears_turn_keys',
        'session_abort_flags_are_session_scoped',
        'agent_session_reply_action_forwards_completed_outcome_with_legacy_reminder_text',
        'agent_session_reply_action_suppresses_cancelled_auto_reply_when_requested',
        'agent_session_reply_action_ignores_non_agent_session_turns',
        'dialog_steering_action_buffers_exact_running_turn_with_display_fallback',
        'dialog_steering_action_rejects_when_target_turn_is_not_running',
        'round_yield_flags_are_session_scoped_and_clearable',
        'round_injection_buffer_drains_only_messages_for_the_active_turn',
        'turn_outcome_status_reply_and_queue_policy_are_portable',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/thread_goal.rs',
      contracts: [
        'ThreadGoalRuntime',
        'SetThreadGoalRequest',
        'build_set_thread_goal_result',
        'continuation_after_turn',
        'ThreadGoalContinuationOutcome',
        'goal_tool_response',
        'should_skip_goal_for_turn',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/thread_goal_contracts.rs',
      contracts: [
        'set_thread_goal_creates_new_active_goal_with_trimmed_objective',
        'continuation_outcome_increments_active_goal_and_builds_plan',
        'continuation_outcome_marks_active_goal_blocked_at_limit',
        'continuation_outcome_reports_budget_limit_once_when_tokens_cross_budget',
        'prompt_and_tool_response_contracts_match_thread_goal_wire_shape',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/prompt.rs',
      contracts: [
        'UserContextSection',
        'UserContextPolicy',
        'ToolListingSections',
        'PrependedPromptReminders',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/prompt_cache.rs',
      contracts: [
        'PROMPT_CACHE_SCHEMA_VERSION',
        'PromptCachePolicy',
        'prompt_cache_scope_key',
        'SessionPromptCacheStore',
        'PromptCacheLookup',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/prompt_cache_contracts.rs',
      contracts: [
        'prompt_cache_policy_keeps_existing_default_persistence_ttl',
        'prompt_cache_lookup_preserves_identity_and_expiry_semantics',
        'prompt_cache_scope_key_preserves_legacy_mode_switch_shape',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/prompt_contracts.rs',
      contracts: [
        'user_context_policy_preserves_order_and_deduplicates_sections',
        'tool_listing_sections_render_only_present_sections',
        'prepended_prompt_reminders_keep_runtime_injection_order',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/events.rs',
      contracts: ['FinishReason', 'session_state_label', 'turn_outcome_kind'],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/events_contracts.rs',
      contracts: [
        'finish_reason_display_preserves_wire_labels',
        'session_state_labels_match_existing_event_wire_values',
        'turn_outcome_kind_matches_existing_reply_policy_contract',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/scheduled_job.rs',
      contracts: [
        'ScheduledJobRuntimeState',
        'ScheduledJobRunStatus',
        'DEFAULT_SCHEDULED_JOB_RETRY_DELAY_MS',
        'mark_manual_trigger',
        'apply_due_scheduled_trigger',
        'mark_enqueued',
        'mark_enqueue_failed',
        'recover_interrupted_turn_after_restart',
        'pending_is_due',
        'next_wakeup_at_ms',
        'clear_pending_trigger',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/scheduled_job_contracts.rs',
      contracts: [
        'manual_trigger_coalesces_existing_pending_run',
        'due_scheduled_trigger_coalesces_when_active_or_pending',
        'pending_wakeup_prefers_retry_time_when_present',
        'disabled_and_config_clear_remove_pending_retry_without_touching_history',
        'enqueue_success_sets_active_turn_and_disables_one_shot_next_run',
        'enqueue_failure_preserves_retry_and_missing_session_disable_semantics',
        'restart_recovery_marks_active_turn_error',
        'serde_shape_preserves_legacy_cron_state_wire_contract',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/service/cron/types.rs',
      contracts: [
        'ScheduledJobRuntimeState as CronJobState',
        'ScheduledJobRunStatus as CronJobRunStatus',
        'DEFAULT_SCHEDULED_JOB_RETRY_DELAY_MS',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/service/cron/service.rs',
      contracts: [
        'mark_manual_trigger',
        'apply_due_scheduled_trigger',
        'mark_enqueued',
        'mark_enqueue_failed',
        'recover_interrupted_turn_after_restart',
        'pending_is_due',
        'next_wakeup_at_ms',
        'clear_pending_trigger',
        'ScheduledJobEnqueueFailureAction',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/execution/types.rs',
      contracts: ['bitfun_agent_runtime::events::FinishReason'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/events/types.rs',
      contracts: ['bitfun_agent_runtime::events::session_state_label'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/agents/prompt_builder/user_context.rs',
      contracts: ['bitfun_agent_runtime::prompt'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/subagent_runtime/mod.rs',
      contracts: [
        'bitfun_runtime_ports',
        'DelegationPolicy',
        'SubagentContextMode',
        'queue_timing',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/session/session_manager.rs',
      contracts: [
        'clone_prompt_cache',
        'start_dialog_turn_with_existing_context',
        'start_dialog_turn_with_existing_context_persists_turn_and_snapshot',
        'clone_prompt_cache_copies_runtime_and_persisted_entries',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/pipeline/tool_pipeline.rs',
      contracts: [
        'resolve_tool_confirmation_plan',
        'resolve_confirmation_failure',
        'resolve_confirmation_wait_result',
        'ToolConfirmationPlan::Await',
        'should_retry_tool_attempt',
        'retry_delay_ms',
        'build_tool_call_truncation_recovery_notice',
        'truncation_notice_for_interactive_tools_does_not_claim_file_write',
        'truncation_notice_for_write_tools_keeps_write_continuation_guidance',
        'denied_tool_messages',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/restrictions.rs',
      contracts: ['denied_tool_messages', 'custom_deny_message_overrides_generic_runtime_error'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/tool_result_storage.rs',
      contracts: ['write_once', 'file\\.flush\\(\\)\\.await'],
    },
    {
      path: 'src/crates/execution/tool-execution/src/pipeline.rs',
      contracts: [
        'ToolBatch',
        'partition_tool_batches',
        'ToolExecutionErrorClass',
        'ToolRetryAttemptFacts',
        'should_retry_tool_attempt',
        'retry_delay_ms',
      ],
    },
    {
      path: 'src/crates/execution/tool-execution/tests/tool_pipeline_planning.rs',
      contracts: [
        'partitions_consecutive_concurrency_safe_tools_into_parallel_batches',
        'retry_policy_preserves_attempt_limit_and_error_class_contract',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/src/mcp/server/connection.rs',
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
      path: 'src/crates/services/services-integrations/src/mcp/protocol/transport.rs',
      contracts: ['send_request_with_id', '\\.flush\\(\\)\\s*\\.await'],
    },
    {
      path: 'src/crates/execution/tool-contracts/src/framework.rs',
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
      path: 'src/crates/execution/tool-contracts/src/file_guidance.rs',
      contracts: [
        'FILE_TOOL_GUIDANCE_PREFIX',
        'file_tool_guidance_message',
        'is_file_tool_guidance_message',
      ],
    },
    {
      path: 'src/crates/execution/tool-contracts/src/file_read_freshness.rs',
      contracts: [
        'FileReadFreshnessFacts',
        'normalize_tool_file_content',
        'file_read_facts_content_matches',
        'file_read_facts_are_fresh',
      ],
    },
    {
      path: 'src/crates/execution/tool-contracts/src/tool_result_storage.rs',
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
      path: 'src/crates/execution/tool-contracts/src/tool_execution_presentation.rs',
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
      path: 'src/crates/assembly/core/src/agentic/coordination/coordinator.rs',
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
      path: 'src/crates/assembly/core/src/agentic/coordination/scheduler.rs',
      contracts: [
        'AgentSessionReplyRoute',
        'DialogQueuePriority',
        'DialogSessionStateFact',
        'DialogSteerOutcome',
        'DialogSubmissionPolicy',
        'DialogSubmitOutcome',
        'DialogSubmitQueueAction',
        'DialogSubmitQueueFacts',
        'ActiveDialogTurnStore',
        'AgentSessionReplyAction',
        'AgentSessionReplyPlan',
        'BackgroundInjectionKind',
        'DialogReplySuppressionSet',
        'DialogSteeringAction',
        'DialogTurnQueue',
        'SessionAbortFlags',
        'dialog_policy_may_preempt',
        'resolve_agent_session_reply_action',
        'resolve_background_delivery_injection',
        'resolve_dialog_submit_queue_action',
        'resolve_dialog_steering_action',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/round_preempt.rs',
      contracts: [
        'bitfun_agent_runtime',
        'bitfun_runtime_ports',
        'DialogRoundInjectionSource',
        'DialogRoundPreemptSource',
        'RoundInjection',
        'RoundInjectionKind',
        'RoundInjectionTarget',
        'SessionRoundInjectionBuffer',
        'SessionRoundYieldFlags',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/goal_mode/mod.rs',
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
      path: 'src/crates/assembly/core/src/agentic/core/message.rs',
      contracts: ['bitfun_runtime_ports', 'CompressionContract', 'CompressionContractItem'],
    },
    {
      path: 'src/crates/assembly/core/src/service/workspace/manager.rs',
      contracts: ['bitfun_runtime_ports', 'RelatedPath'],
    },
    {
      path: 'src/crates/assembly/core/src/service_agent_runtime.rs',
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
      path: 'src/crates/services/services-integrations/src/remote_connect.rs',
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
      path: 'src/crates/services/services-integrations/tests/remote_connect_contracts.rs',
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
      path: 'src/crates/assembly/core/src/service/remote_connect/remote_server.rs',
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
      path: 'src/crates/assembly/core/src/agentic/coordination/scheduler.rs',
      contracts: ['remote_queue_policy_preserves_interactive_preempt_and_confirmation_boundary'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/registry.rs',
      contracts: [
        'from_inner',
        'ProductToolDecoratorRef',
        'ProductToolRuntime',
        'get_collapsed_tool_names',
        'resolve_product_readonly_enabled_tools',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/product_runtime.rs',
      contracts: [
        'ProductToolRuntime',
        'SnapshotToolDecorator',
        'create_product_tool_registry_from_plan',
        'product_assembly_plan_for_profile',
        'product_tool_runtime_owner_preserves_registry_contract',
        'product_tool_runtime_registry_preserves_provider_plan_order',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/snapshot.rs',
      contracts: [
        'ProductSnapshotToolWrapper',
        'SnapshotToolWrapper',
        'wrap_tool_for_snapshot_tracking',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/catalog.rs',
      contracts: [
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
        'product_resolved_manifest_owner_matches_legacy_shape',
        'GetToolSpec requires agent type context',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/materialization.rs',
      contracts: [
        'ProductConcreteToolFactory',
        'StaticToolProviderFactory',
        'ProductToolProviderPlanAdapter',
        'StaticToolProviderPlan',
        'create_registry_from_static_provider_plans',
        'create_product_tool_registry_from_plan',
        'materialize_tool',
        'GetToolSpecTool',
      ],
    },
    {
      path: 'src/crates/execution/tool-contracts/src/framework.rs',
      contracts: [
        'ToolContextFacts',
        'PortableToolContextProvider',
        'ToolWorkspaceKind',
        'StaticToolProvider',
        'StaticToolProviderGroup',
        'StaticToolProviderPlan',
        'StaticToolProviderFactory',
        'StaticToolMaterializationError',
        'materialize_static_tool_provider_groups',
        'ToolRuntimeAssembly',
        'create_registry_from_static_provider_plans',
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
      path: 'src/crates/execution/tool-provider-groups/src/lib.rs',
      contracts: [
        'ToolPackFeatureGroup',
        'ToolProviderGroupPlan',
        'all_feature_groups',
        'enabled_feature_groups',
        'product_tool_provider_group_plan',
        'ToolProviderGroupPlanSelectionError',
        'try_product_tool_provider_group_plan_for_ids',
        'product_provider_group_plan_selector_rejects_unknown_provider_ids',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/tool_adapter.rs',
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
      path: 'src/crates/assembly/core/src/agentic/tools/manifest_resolver.rs',
      contracts: [
        'resolve_tool_manifest',
        'GET_TOOL_SPEC_TOOL_NAME',
        'resolve_product_resolved_visible_tools',
        'resolve_product_resolved_tool_manifest',
        'collapsed_tool_names',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/get_tool_spec_tool.rs',
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
      path: 'src/crates/assembly/core/src/agentic/tools/framework.rs',
      contracts: [
        'ToolExposure',
        'ToolUseContext',
        'pub use crate::agentic::tools::tool_context_runtime::ToolUseContext',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/tool_context_runtime.rs',
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
        'build_runtime_light_checkpoint',
        'LightCheckpointWorkspaceFacts::LocalWorkspace',
        'call_with_tool_runtime_hooks',
        'call_tool_with_runtime_hooks',
        'call_records_deep_review_read_file_measurement_without_touching_result',
        'build_tool_use_context_for_task',
        'build_tool_description_context',
        'ensure_current_workspace_runtime',
        'resolve_tool_path',
        'enforce_path_operation',
        'workspace_path_resolution_rejects_absolute_paths_outside_remote_workspace',
        'runtime_uri_resolution_rejects_different_workspace_scope',
        'path_policy_allows_only_configured_local_roots',
        'tool_call_runtime_hook_returns_cancelled_before_impl_completes',
        'tool_task_context_materialization_preserves_runtime_fields',
        'tool_description_context_preserves_manifest_custom_data_shape',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/pipeline/tool_pipeline.rs',
      contracts: [
        'validate_tool_execution_admission',
        'unlocked_collapsed_tools',
        'GetToolSpec',
        'render_tool_result_for_assistant',
        'build_tool_execution_error_presentation',
        'build_user_steering_interrupted_presentation',
        'build_invalid_tool_call_error_message',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/execution/execution_engine.rs',
      contracts: [
        'collect_product_unlocked_collapsed_tools',
        'unlocked_collapsed_tools',
        'collapsed_tool_names',
        'GetToolSpec',
        'citation_renumber',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/product_runtime/unlock_state.rs',
      contracts: [
        'collect_product_unlocked_collapsed_tools',
        'GetToolSpecLoadObservation',
        'collect_loaded_collapsed_tool_names',
        'product_unlock_state_dedupes_and_filters_runtime_unlocks',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/agents/registry/availability.rs',
      contracts: [
        'resolve_availability',
        'resolve_override_layers',
        'AgentSubagentOverrideState',
        'resolve_subagent_availability',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/agents/registry/types.rs',
      contracts: ['SubagentQueryContext', 'SubagentListScope', 'default_enabled', 'effective_enabled', 'SubagentStateReason'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/agents/definitions/modes/mod.rs',
      contracts: ['mod multitask', 'MultitaskMode'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/agents/definitions/subagents/mod.rs',
      contracts: ['mod general_purpose', 'GeneralPurposeAgent'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/agents/registry/builtin.rs',
      contracts: ['builtin_agent_specs', 'runtime_agents::default_model_id_for_builtin_agent'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/task_tool.rs',
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
      path: 'src/crates/assembly/core/src/agentic/coordination/scheduler.rs',
      contracts: [
        'deliver_background_result',
        'BackgroundResult',
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
      path: 'src/crates/assembly/core/src/agentic/agents/citation_renumber.rs',
      contracts: ['run_for_session_workspace', 'try_renumber_research_report', 'display_map', 'REJECTED'],
    },
    {
      path: 'src/crates/assembly/core/src/service/workspace/service.rs',
      contracts: ['prepare_startup_restored_workspaces', 'WorkspaceKind::Remote', 'ensure_remote_workspace_runtime', 'sshHost'],
    },
    {
      path: 'src/crates/services/services-integrations/src/workspace_search/mod.rs',
      contracts: ['flashgrep'],
    },
    {
      path: 'src/crates/services/services-integrations/src/workspace_search/service.rs',
      contracts: ['WorkspaceSearchRepoConfig', 'with_scan_fallback'],
    },
    {
      path: 'src/crates/services/services-integrations/src/workspace_search/result_mapping.rs',
      contracts: ['convert_hits_to_file_search_results', 'split_preview', 'preview_inside'],
    },
    {
      path: 'src/crates/assembly/core/src/service/search/service.rs',
      contracts: ['owner::WorkspaceSearchService::new_with_hooks', 'CoreWorkspaceSearchRuntimeHooks', 'WorkspaceSearchRepoConfig', 'get_global_config_service', 'ensure_workspace_gitignore_ignores_bitfun'],
    },
    {
      path: 'src/crates/assembly/core/src/service/search/remote.rs',
      contracts: ['ServiceRemoteWorkspaceSearchService', 'impl RemoteWorkspaceSearchProvider for CoreRemoteWorkspaceSearchProvider', 'lookup_remote_connection_with_hint', 'open_exec_channel', 'RemoteWorkspaceSearchStdioProtocol'],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_ssh/workspace_search/mod.rs',
      contracts: [
        'build_remote_scope',
        'shell_escape',
        'should_retry_remote_scan_fallback_as_files_with_matches',
        'remote_workspace_search_paths_preserve_current_contract',
        'remote_scan_fallback_retry_policy_preserves_current_contract',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_ssh/workspace_search/service.rs',
      contracts: ['RemoteWorkspaceSearchProvider', 'RemoteWorkspaceSearchService', 'RemoteWorkspaceSearchStdioProtocol', 'REMOTE_STDIO_SESSIONS', 'ensure_remote_search_context', 'allow_scan_fallback', 'fallback_query', 'remote_search_rejects_non_linux_before_stdio_open'],
    },
    {
      path: 'src/crates/assembly/core/src/service/search/mod.rs',
      contracts: ['mod remote_disabled', 'feature = "ssh-remote"', 'pub use remote_disabled'],
    },
    {
      path: 'src/crates/assembly/core/src/service/search/remote_disabled.rs',
      contracts: ['Remote SSH search is disabled', 'RemoteWorkspaceSearchService', 'remote_workspace_search_service_for_path'],
    },
    {
      path: 'src/crates/assembly/core/Cargo.toml',
      contracts: [
        'bitfun-product-capabilities = \\{ path = "\\.\\.\\/product-capabilities", default-features = false, optional = true \\}',
        'bitfun-tool-packs = \\{ path = "\\.\\.\\/\\.\\.\\/execution\\/tool-provider-groups", default-features = false, optional = true \\}',
        'bitfun-services-integrations = \\{ path = "\\.\\.\\/\\.\\.\\/services\\/services-integrations", default-features = false, features = \\["remote-ssh", "workspace-search"\\] \\}',
        'bitfun-product-domains = \\{ path = "\\.\\.\\/\\.\\.\\/contracts\\/product-domains", default-features = false, optional = true \\}',
        'dep:bitfun-product-capabilities',
        'dep:bitfun-tool-packs',
        'bitfun-tool-packs\\/product-full',
        'bitfun-services-integrations\\/product-full',
        'dep:bitfun-product-domains',
        'bitfun-product-domains\\/product-full',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/lib.rs',
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
      path: 'src/crates/assembly/core/src/service/mod.rs',
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
      path: 'src/crates/assembly/core/src/service/config/mod.rs',
      contracts: ['feature = "product-full"', 'mode_config_canonicalizer'],
    },
    {
      path: 'src/crates/assembly/core/src/service/workspace/manager.rs',
      contracts: ['feature = "service-integrations"', 'GitService', 'return None'],
    },
    {
      path: 'src/crates/assembly/core/src/service/workspace_runtime/service.rs',
      contracts: ['feature = "product-full"', 'WorkspaceBinding', 'ensure_runtime_for_workspace_binding'],
    },
    {
      path: 'src/crates/interfaces/acp/src/client/manager.rs',
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
      path: 'src/crates/assembly/core/src/miniapp/storage.rs',
      contracts: ['MiniAppStoragePort'],
    },
    {
      path: 'src/crates/assembly/core/src/miniapp/builtin/mod.rs',
      contracts: [
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
      path: 'src/crates/contracts/product-domains/src/miniapp/builtin.rs',
      contracts: [
        'builtin-pr-review',
        'BUILTIN_APPS',
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
      path: 'src/crates/assembly/core/src/miniapp/host_dispatch.rs',
      contracts: [
        'dispatch_host',
        'split_host_method',
        'dispatch_fs',
        'plan_fs_legacy_path_check',
        'plan_fs_host_call',
        'fs_policy_scopes',
        'fs_resolved_path_allowed',
        'dispatch_shell',
        'plan_shell_host_call',
        'shell_exec_default_env',
        'command_basename_allowed',
        'host_allowed_by_allowlist',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/miniapp/js_worker_pool.rs',
      contracts: [
        'MiniAppRuntimePort',
        'plan_install_deps',
        'worker_is_idle',
        'worker_pool_at_capacity',
        'select_lru_worker',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/function_agents/port_adapters.rs',
      contracts: [
        'CoreFunctionAgentGitAdapter',
        'FunctionAgentGitPort',
        'FunctionAgentGitService::git_commit_snapshot',
        'CoreFunctionAgentAiAdapter',
        'FunctionAgentAiPort',
        'git_adapter_commit_snapshot_keeps_staged_diff_and_unstaged_count_separate',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/service/remote_connect/bot/command_router.rs',
      contracts: [
        'CoreServiceAgentRuntime',
        'agent_submission_port',
        'build_remote_session_create_request',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/product_domain_runtime.rs',
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
      path: 'src/crates/assembly/core/src/service/remote_ssh/mod.rs',
      contracts: ['mod disabled', 'pub mod manager', 'pub mod remote_fs', 'pub mod remote_terminal', 'pub mod workspace_state'],
    },
    {
      path: 'src/crates/assembly/core/src/service/remote_ssh/disabled.rs',
      contracts: ['Remote SSH support is disabled', 'SSHConnectionManager', 'RemoteFileService', 'RemoteTerminalManager'],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_ssh/paths.rs',
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
      path: 'src/crates/contracts/product-domains/src/miniapp/ports.rs',
      contracts: [
        'MiniAppRuntimeFacade',
        'mark_deps_installed_state',
        'persist_sync_from_fs_result_for_app',
        'persist_import_runtime_state',
      ],
    },
    {
      path: 'src/crates/contracts/product-domains/src/miniapp/storage.rs',
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
      path: 'src/crates/contracts/product-domains/src/miniapp/lifecycle.rs',
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
      path: 'src/crates/contracts/product-domains/src/miniapp/draft.rs',
      contracts: ['MiniAppDraftManifest', 'MiniAppDraft', 'build_draft_manifest', 'build_draft_response'],
    },
    {
      path: 'src/crates/contracts/product-domains/src/miniapp/runtime.rs',
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
      path: 'src/crates/contracts/product-domains/src/miniapp/worker.rs',
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
      path: 'src/crates/contracts/product-domains/src/miniapp/host_routing.rs',
      contracts: [
        'split_host_method',
        'FsAccessMode',
        'fs_method_access_mode',
        'MiniAppFsHostCallPlan',
        'plan_fs_host_call',
        'plan_fs_legacy_path_check',
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
        'MiniAppShellHostCallPlan',
        'plan_shell_host_call',
      ],
    },
    {
      path: 'src/crates/contracts/product-domains/src/miniapp/exporter.rs',
      contracts: ['MISSING_JS_RUNTIME_MESSAGE', 'export_runtime_label', 'build_export_check_result'],
    },
    {
      path: 'src/crates/assembly/core/src/miniapp/exporter.rs',
      contracts: ['detect_runtime', 'build_export_check_result', 'Export not yet implemented'],
    },
    {
      path: 'src/crates/contracts/product-domains/src/miniapp/customization.rs',
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
      path: 'src/crates/assembly/core/src/miniapp/manager.rs',
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
      path: 'src/crates/assembly/core/src/function_agents/runtime_services.rs',
      contracts: [
        'prepare_commit_ai_prompt',
        'parse_commit_ai_response',
        'build_work_state_analysis_prompt',
        'parse_work_state_analysis_response',
        'send_message',
        'AgentError::internal_error',
        'CoreCommitAiAnalysisService',
        'CoreWorkStateAiAnalysisService',
        'parse_commit_response_preserves_product_domain_response_policy',
        'parse_complete_analysis_preserves_product_domain_response_policy',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/src/function_agents.rs',
      contracts: [
        'FunctionAgentGitService',
        'git_commit_snapshot',
        'startchat_git_snapshot',
        'startchat_time_snapshot',
        'process_manager::create_command("git")',
        'git_unpushed_commits',
        'git_ahead_behind',
        'git_last_commit_timestamp',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/tests/function_agent_contracts.rs',
      contracts: [
        'git_service_builds_commit_snapshot_from_staged_diff_without_unstaged_content',
        'git_service_startchat_snapshot_preserves_no_head_and_non_git_fallback',
        'git_service_time_snapshot_uses_last_commit_timestamp',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/function_agents/git-func-agent/ai_service.rs',
      contracts: ['CoreCommitAiAnalysisService as AIAnalysisService'],
    },
    {
      path: 'src/crates/assembly/core/src/function_agents/startchat-func-agent/ai_service.rs',
      contracts: ['CoreWorkStateAiAnalysisService as AIWorkStateService'],
    },
    {
      path: 'src/crates/assembly/core/src/function_agents/git-func-agent/commit_generator.rs',
      contracts: ['CoreProductDomainRuntime', 'generate_function_agent_commit_message'],
    },
    {
      path: 'src/crates/assembly/core/src/function_agents/startchat-func-agent/work_state_analyzer.rs',
      contracts: ['CoreProductDomainRuntime', 'analyze_function_agent_work_state'],
    },
    {
      path: 'src/crates/contracts/product-domains/src/function_agents/ports.rs',
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
      path: 'src/crates/contracts/product-domains/src/function_agents/common.rs',
      contracts: ['extract_json_from_ai_response', 'try_repair_json'],
    },
    {
      path: 'src/crates/contracts/product-domains/src/function_agents/startchat_func_agent/utils.rs',
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
      path: 'src/crates/contracts/product-domains/src/function_agents/git_func_agent/utils.rs',
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
      path: 'src/crates/assembly/core/src/miniapp/runtime_detect.rs',
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
      if (!regexSourceContainsContract(ruleText, contract)) {
        throw new Error(`owner content anchor rule for ${path} must require: ${contract}`);
      }
    }
  }

  const remoteWorkspaceRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/assembly/core/src/service/remote_ssh/workspace_state.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/announcement/state_store.rs',
  );
  if (!announcementStateStoreRule) {
    throw new Error('missing announcement state store boundary rule');
  }

  const mcpProcessRule = forbiddenContentRules.find(
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/server/process.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/server/manager/mod.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/server/manager/reconnect.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/server/manager/interaction.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/adapter/tool.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/adapter/context.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/config/json_config.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/config/service.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/auth.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/protocol/transport_remote.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/protocol/jsonrpc.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/mcp/server/config.rs',
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
    (rule) => rule.path === 'src/crates/assembly/core/src/service/remote_connect/remote_server.rs',
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
    'src/crates/assembly/core/src/service/mcp/protocol/transport.rs',
    'src/crates/assembly/core/src/service/mcp/protocol/transport_remote.rs',
    'src/crates/assembly/core/src/service/mcp/server/connection.rs',
  ]) {
    if (!facadePaths.has(path)) {
      throw new Error(`missing MCP runtime facade-only rule for ${path}`);
    }
  }
}
