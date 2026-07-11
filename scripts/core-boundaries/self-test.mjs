// Self-tests for the core boundary checker configuration and parsers.

import { crateLayoutRules } from './rules/crate-layout.mjs';

export function runManifestParserSelfTest({
  isManifestDependencyDeclaration,
  parseManifestDependencies,
  manifestDependencyMatches,
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
  forbiddenManifestDependencyRules,
  noCoreDependencyCrates,
  requiredContentRules,
  forbiddenContentRules,
  forbiddenContentUnderRules,
  publicApiAllowlistRules,
  publicApiContractSlices,
  facadeOnlyFiles,
  forbiddenRuleTextForPath,
  regexSourceContainsContract,
  collectTopLevelRustPublicSymbols,
  collectPluginRootReexports,
  hasPluginWildcardReexport,
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
    'single-quoted-opencode = { package = \'bitfun-opencode-adapter\', path = "../adapters/opencode-adapter" }',
    '[dependencies.git2]',
    'workspace = true',
    'optional = true',
    '[target.\'cfg(windows)\'.dependencies."bitfun-cli"]',
    'path = "../../apps/cli"',
    '[dependencies.renamed-opencode]',
    'package = "bitfun-opencode-adapter"',
    'path = "../adapters/opencode-adapter"',
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
  if (parsedByName.get('renamed-opencode')?.optional !== false) {
    throw new Error('dependency profile parser must detect renamed dependency tables');
  }
  if (
    !manifestDependencyMatches(
      parsedByName.get('single-quoted-opencode'),
      'bitfun-opencode-adapter',
    )
  ) {
    throw new Error('dependency profile parser must detect single-quoted package aliases');
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
  const parsedWorkspaceDeps = parseManifestDependencies(
    [
      '[workspace.dependencies]',
      'opencode-fixture = { path = "src/crates/adapters/opencode-adapter", package = "bitfun-opencode-adapter" }',
      'opencode-fixture-single = { path = "src/crates/adapters/opencode-adapter", package = \'bitfun-opencode-adapter\' }',
      '[workspace.dependencies.renamed-opencode-workspace]',
      'package = "bitfun-opencode-adapter"',
      'path = "src/crates/adapters/opencode-adapter"',
    ],
    { includeWorkspace: true },
  );
  const workspaceDepsByName = new Map(parsedWorkspaceDeps.map((dep) => [dep.name, dep]));
  if (
    !manifestDependencyMatches(
      workspaceDepsByName.get('opencode-fixture'),
      'bitfun-opencode-adapter',
    ) ||
    !manifestDependencyMatches(
      workspaceDepsByName.get('opencode-fixture-single'),
      'bitfun-opencode-adapter',
    ) ||
    !manifestDependencyMatches(
      workspaceDepsByName.get('renamed-opencode-workspace'),
      'bitfun-opencode-adapter',
    )
  ) {
    throw new Error('manifest parser must detect workspace aliases to forbidden packages');
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
    '    "bitfun-services-integrations/remote-ssh-concrete",',
    '    "russh",',
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
  if (!parsedFeatures.get('ssh-remote')?.refs.includes('russh')) {
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
  const coreDeepReviewTaskAdapterRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/deep_review/task_adapter.rs',
  );
  if (!coreDeepReviewTaskAdapterRuleText) {
    throw new Error('missing core DeepReview task adapter boundary rule');
  }
  const coreDeepReviewTaskAdapterContracts = [
    'QueueWaitTimer',
    'decide_provider_capacity_queue_step',
    'decide_blocked_reviewer_admission_queue_step',
    '<partial_result status=',
    'completed successfully with result:',
    'Retries used:',
    'DeepReview automatic retry elapsed guard exceeded',
    'cancelled coverage',
  ];
  for (const contract of coreDeepReviewTaskAdapterContracts) {
    if (!coreDeepReviewTaskAdapterRuleText.includes(contract)) {
      throw new Error(`core DeepReview task adapter boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreTaskToolRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/task/execution.rs',
  );
  if (!coreTaskToolRuleText) {
    throw new Error('missing core TaskTool DeepReview boundary rule');
  }
  const coreTaskToolContracts = [
    '<partial_result status=',
    'completed successfully with result:',
    'Retries used:',
    'DeepReview automatic retry elapsed guard exceeded',
    'cancelled coverage',
    'provider_capacity_retry_attempts',
    'provider_capacity_queue_elapsed_ms',
    'DEEP_REVIEW_PROVIDER_CAPACITY_MAX_RETRY_ATTEMPTS',
  ];
  for (const contract of coreTaskToolContracts) {
    if (!coreTaskToolRuleText.includes(contract)) {
      throw new Error(`core TaskTool DeepReview boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreCodeReviewToolRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/code_review_tool.rs',
  );
  if (!coreCodeReviewToolRuleText) {
    throw new Error('missing core CodeReviewTool DeepReview report boundary rule');
  }
  if (!coreCodeReviewToolRuleText.includes('"kind"')) {
    throw new Error('core CodeReviewTool DeepReview boundary rule must be anchored to kind');
  }
  for (const contract of ['"cache_hit"', '"cache_miss"']) {
    if (!coreCodeReviewToolRuleText.includes(contract)) {
      throw new Error(
        `core CodeReviewTool DeepReview boundary rule must forbid contract: ${contract}`,
      );
    }
  }
  for (const contract of ['DeepReviewIncrementalCache', 'deepReviewCache']) {
    if (!coreTaskToolRuleText.includes(contract)) {
      throw new Error(`core TaskTool DeepReview boundary rule must forbid contract: ${contract}`);
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
  const coreSessionStateManagerRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/coordination/state_manager.rs',
  );
  if (!coreSessionStateManagerRuleText) {
    throw new Error('missing core session state manager boundary rule');
  }
  for (const contract of ['SessionStateManager', 'DashMap', 'AgenticEvent::SessionStateChanged']) {
    if (!coreSessionStateManagerRuleText.includes(contract)) {
      throw new Error(`core session state manager boundary rule must forbid contract: ${contract}`);
    }
  }
  const coreRoundPreemptRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/round_preempt.rs',
  );
  if (!coreRoundPreemptRuleText) {
    throw new Error('missing core round preempt boundary rule');
  }
  for (const contract of [
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
  for (const dep of [
    'git2',
    'rmcp',
    'image',
    'tool-runtime',
    'bitfun-relay-server',
    'htmd',
    'legible',
    'readability-js',
  ]) {
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
  const coreFullyMigratedDeps = new Set([
    'aes',
    'hostname',
    'htmd',
    'legible',
    'local-ip-address',
    'mac_address',
    'qrcode',
    'readability-js',
    'x25519-dalek',
  ]);
  for (const dep of coreProfile?.forbiddenNonOptionalDeps ?? []) {
    if (coreFullyMigratedDeps.has(dep)) {
      continue;
    }
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
  const servicesOptionalOwnerDeps = new Set(
    servicesOptionalOwnerRule?.dependencies.map((dependency) => dependency.depName) ?? [],
  );
  const servicesIntegrationsDefaultOnlyGuardDeps = new Set(['bitfun-relay-server']);
  for (const dep of servicesIntegrationsDefaultProfile?.forbiddenNonOptionalDeps ?? []) {
    if (servicesIntegrationsDefaultOnlyGuardDeps.has(dep)) {
      continue;
    }
    if (!servicesOptionalOwnerDeps.has(dep)) {
      throw new Error(
        `services-integrations optional dependency owner rule must cover forbidden dependency ${dep}`,
      );
    }
  }
  for (const dep of [
    'aes',
    'bitfun-services-core',
    'bitfun-product-domains',
    'dunce',
    'fs2',
    'bitfun-runtime-ports',
    'git2',
    'hex',
    'hostname',
    'libc',
    'local-ip-address',
    'mac_address',
    'md5',
    'notify',
    'qrcode',
    'reqwest',
    'rmcp',
    'tokio-tungstenite',
    'which',
    'windows',
    'x25519-dalek',
  ]) {
    if (!servicesOptionalOwnerRule?.dependencies.some((dependency) => dependency.depName === dep)) {
      throw new Error(`services-integrations optional dependency owner rule must cover ${dep}`);
    }
  }
  for (const dep of ['bitfun-product-domains', 'dunce', 'fs2', 'hex', 'libc', 'sha2', 'thiserror', 'uuid', 'windows']) {
    const owner = servicesOptionalOwnerRule?.dependencies.find(
      (dependency) => dependency.depName === dep,
    );
    if (!owner?.ownerFeatures.includes('plugin-source')) {
      throw new Error(`services-integrations plugin-source must own optional dependency ${dep}`);
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
  const coreBrowserLauncherFacadeRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/browser_control/browser_launcher.rs',
  );
  for (const contract of ['process_manager::', 'Command::new', 'reqwest::']) {
    if (!coreBrowserLauncherFacadeRuleText.includes(contract)) {
      throw new Error(`core browser launcher facade boundary rule must forbid: ${contract}`);
    }
  }
  const coreReviewPlatformFacadeRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/service/review_platform/mod.rs',
  );
  for (const contract of [
    'reqwest::',
    'tokio::fs',
    'process_manager::',
    'execute_git_command',
    'serde_json::',
    'ReviewProvider',
  ]) {
    if (!coreReviewPlatformFacadeRuleText.includes(contract)) {
      throw new Error(`core review platform facade boundary rule must forbid: ${contract}`);
    }
  }
  const coreComputerUseSystemFacadeRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/computer_use_actions.rs',
  );
  for (const contract of [
    'process_manager::',
    'tokio::process::',
    'script_invocation',
    'platform_open_attempts',
  ]) {
    if (!coreComputerUseSystemFacadeRuleText.includes(contract)) {
      throw new Error(`core computer-use system facade boundary rule must forbid: ${contract}`);
    }
  }
  const coreLocalShellFacadeRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/local_shell.rs',
  );
  for (const contract of ['process_manager::', 'Command::new', 'std::env::var', 'target_os']) {
    if (!coreLocalShellFacadeRuleText.includes(contract)) {
      throw new Error(`core local shell facade boundary rule must forbid: ${contract}`);
    }
  }
  const coreWeixinFacadeRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/service/remote_connect/bot/weixin.rs',
  );
  for (const contract of ['reqwest::', 'aes::', 'hex::', 'md5::', 'sync_buf_path']) {
    if (!coreWeixinFacadeRuleText.includes(contract)) {
      throw new Error(`core Weixin bot facade boundary rule must forbid: ${contract}`);
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
  const coreTypesAiRuleText = forbiddenRuleTextForPath(
    'src/crates/contracts/core-types/src/ai.rs',
  );
  for (const contract of ['resolve_request_url', 'chat\\/completions', 'v1\\/messages']) {
    if (!coreTypesAiRuleText.includes(contract)) {
      throw new Error(`core-types AI DTO boundary rule must forbid: ${contract}`);
    }
  }
  const runtimePortsProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'runtime-ports',
  );
  if (!runtimePortsProfile?.forbiddenNonOptionalDeps.includes('bitfun-services-core')) {
    throw new Error('runtime-ports dependency profile must forbid service implementations');
  }
  const pluginRuntimeContractRule = requiredContentRules.find(
    (rule) => rule.path === 'src/crates/contracts/runtime-ports/src/plugin.rs',
  );
  if (!pluginRuntimeContractRule) {
    throw new Error('plugin runtime contracts must have a module-local owner rule');
  }
  const pluginRuntimeContractRuleText = pluginRuntimeContractRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const contract of [
    'PluginRuntimeClient',
    'PluginRuntimeBinding',
    'read_plugins',
    'PluginQuarantineState',
  ]) {
    if (!pluginRuntimeContractRuleText.includes(contract)) {
      throw new Error(`plugin runtime contract owner rule must require: ${contract}`);
    }
  }
  const pluginRuntimeForbiddenRuleText = forbiddenRuleTextForPath(
    'src/crates/contracts/runtime-ports/src/plugin.rs',
  );
  for (const forbiddenContract of [
    'serde_json::Value',
    'accepted',
    'product-full',
    'requires_permission',
    'permission_prompt',
    'PluginMaterializeCondition',
  ]) {
    if (!pluginRuntimeForbiddenRuleText.includes(forbiddenContract)) {
      throw new Error(`plugin runtime boundary rule must forbid: ${forbiddenContract}`);
    }
  }
  const pluginPublicApiRule = publicApiAllowlistRules.find(
    (rule) => rule.path === 'src/crates/contracts/runtime-ports/src/plugin.rs',
  );
  const pluginRootReexportRule = publicApiAllowlistRules.find(
    (rule) => rule.path === 'src/crates/contracts/runtime-ports/src/lib.rs',
  );
  const opencodeAdapterPublicApiRule = publicApiAllowlistRules.find(
    (rule) => rule.path === 'src/crates/adapters/opencode-adapter/src/lib.rs',
  );
  const parsedPluginReexports = collectPluginRootReexports(`
    pub use plugin::{PluginDispatchEnvelope, PluginResponseEnvelope};
    pub use self::plugin::{
      PluginRuntimeReadRequest,
      PluginRuntimeReadResponse as RuntimeReadResponse,
    };
    pub use crate::plugin::PluginRuntimeClient;
    pub use plugin::*;
  `);
  if (
    parsedPluginReexports.join(',') !==
    'PluginDispatchEnvelope,PluginResponseEnvelope,PluginRuntimeReadRequest,RuntimeReadResponse,PluginRuntimeClient,*'
  ) {
    throw new Error('plugin root re-export parser must collect plugin path variants, aliases, and wildcard markers');
  }
  for (const wildcardReexport of [
    'pub use plugin::*;',
    'pub use crate::plugin::*;',
    'pub use self::plugin::*;',
  ]) {
    if (!hasPluginWildcardReexport(wildcardReexport)) {
      throw new Error(`plugin wildcard guard must reject path variant: ${wildcardReexport}`);
    }
  }
  for (const nonWildcardReexport of [
    'pub use plugin::PluginDispatchEnvelope;',
    'pub use crate::not_plugin::*;',
  ]) {
    if (hasPluginWildcardReexport(nonWildcardReexport)) {
      throw new Error(`plugin wildcard guard must not reject non-plugin wildcard: ${nonWildcardReexport}`);
    }
  }
  const parsedPluginSymbols = collectTopLevelRustPublicSymbols(`
    pub enum TopLevelEnum { Value }
    impl TopLevelEnum { pub fn hidden_method(&self) {} }
    pub use plugin::PluginDispatchEnvelope;
    pub use crate::hidden::{HiddenType, InternalName as PublicName};
    pub use crate::multi::{
      MultiLineType,
      InternalMultiLine as PublicMultiLine,
    };
    pub mod host;
    pub const CONTRACT_VERSION: u16 = 1;
  `);
  if (
    parsedPluginSymbols.join(',') !==
    'TopLevelEnum,PluginDispatchEnvelope,HiddenType,PublicName,MultiLineType,PublicMultiLine,host,CONTRACT_VERSION'
  ) {
    throw new Error('public API parser must collect top-level items and re-exports without impl methods');
  }
  const pluginPublicApiSymbols = (pluginPublicApiRule?.allowedSymbolEntries || []).map(
    (entry) => entry.symbol,
  );
  const pluginRootReexportSymbols = (pluginRootReexportRule?.allowedPluginReexportEntries || []).map(
    (entry) => entry.symbol,
  );
  if (!pluginPublicApiSymbols.includes('PluginDispatchEnvelope')) {
    throw new Error('plugin runtime public API allowlist must include dispatch envelope');
  }
  if (!pluginPublicApiSymbols.includes('PluginPermissionGate')) {
    throw new Error('plugin runtime public API allowlist must include permission gate');
  }
  if (pluginPublicApiSymbols.some((symbol) => symbol.startsWith('PluginRecoveryAction'))) {
    throw new Error('plugin runtime P0-B public API allowlist must not include recovery action API');
  }
  if (!pluginPublicApiSymbols.includes('PluginRuntimeReadRequest')) {
    throw new Error('plugin runtime public API allowlist must include read request');
  }
  if (!pluginPublicApiSymbols.includes('PluginStatusSnapshot')) {
    throw new Error('plugin runtime public API allowlist must include plugin status snapshot');
  }
  for (const entry of pluginPublicApiRule.allowedSymbolEntries) {
    for (const field of ['owner', 'consumer', 'verification', 'p0', 'contractSlice', 'rationale', 'exit']) {
      if (!entry[field]) {
        throw new Error(`plugin runtime public API entry must declare ${field}: ${entry.symbol}`);
      }
    }
    if (!publicApiContractSlices.includes(entry.contractSlice)) {
      throw new Error(`plugin runtime public API entry uses unknown contractSlice: ${entry.symbol}`);
    }
    if (typeof entry.wireImpact !== 'boolean') {
      throw new Error(`plugin runtime public API entry must declare wireImpact: ${entry.symbol}`);
    }
  }
  if (!pluginRootReexportSymbols.includes('PluginDispatchEnvelope')) {
    throw new Error('runtime-ports root re-export allowlist must include dispatch envelope');
  }
  if (pluginRootReexportSymbols.length !== pluginPublicApiSymbols.length) {
    throw new Error('plugin root re-export allowlist must match plugin module public budget');
  }
  if (!opencodeAdapterPublicApiRule) {
    throw new Error('OpenCode adapter must have a public API budget rule');
  }
  const opencodeAdapterPublicApiSymbols = (
    opencodeAdapterPublicApiRule.allowedSymbolEntries || []
  ).map((entry) => entry.symbol);
  if (
    opencodeAdapterPublicApiSymbols.join(',') !==
    'load_opencode_workspace_adapter'
  ) {
    throw new Error('OpenCode adapter public API budget must stay limited to source adapter loading');
  }
  for (const entry of opencodeAdapterPublicApiRule.allowedSymbolEntries) {
    for (const field of ['owner', 'consumer', 'verification', 'p0', 'contractSlice', 'rationale', 'exit']) {
      if (!entry[field]) {
        throw new Error(`OpenCode adapter public API entry must declare ${field}: ${entry.symbol}`);
      }
    }
    if (entry.contractSlice !== 'opencode-adapter-boundary') {
      throw new Error(`OpenCode adapter public API entry uses wrong contractSlice: ${entry.symbol}`);
    }
    if (entry.wireImpact !== false) {
      throw new Error(`OpenCode adapter public API entry must not claim wire impact: ${entry.symbol}`);
    }
  }
  const appHostAbiRule = forbiddenContentUnderRules.find((rule) => rule.path === 'src/apps');
  if (!appHostAbiRule) {
    throw new Error('product app entrypoints must have a Host ABI import guard');
  }
  const appHostAbiRuleText = appHostAbiRule.patterns
    .map((pattern) => pattern.regex.source)
    .join('\n');
  for (const forbiddenHostAbi of [
    'PluginRuntimeReadResponse',
    'PluginStatusSnapshot',
    'PluginResponseEnvelope',
    'PluginRuntimeBinding',
  ]) {
    if (!appHostAbiRuleText.includes(forbiddenHostAbi)) {
      throw new Error(`product app entrypoint Host ABI guard must forbid: ${forbiddenHostAbi}`);
    }
  }
  const opencodeManifestRule = forbiddenManifestDependencyRules.find((rule) =>
    rule.dependencyNames?.includes('bitfun-opencode-adapter'),
  );
  if (!opencodeManifestRule) {
    throw new Error('OpenCode adapter must have a forbidden manifest dependency rule');
  }
  for (const scanRoot of ['src/apps', 'src/crates', 'BitFun-Installer/src-tauri']) {
    if (!opencodeManifestRule.scanRoots?.includes(scanRoot)) {
      throw new Error(`OpenCode adapter manifest guard must scan ${scanRoot}`);
    }
  }
  if (opencodeManifestRule.workspaceManifestPath !== 'Cargo.toml') {
    throw new Error('OpenCode adapter manifest guard must scan root workspace dependencies');
  }
  if (
    !opencodeManifestRule.allowManifestPaths?.includes(
      'src/crates/adapters/opencode-adapter/Cargo.toml',
    )
  ) {
    throw new Error('OpenCode adapter manifest guard must allow its own manifest');
  }
  const opencodeSourceRules = forbiddenContentUnderRules.filter((rule) =>
    rule.reason.includes('OpenCode adapter must not become a production dependency'),
  );
  for (const scanRoot of ['src', 'BitFun-Installer/src-tauri']) {
    if (!opencodeSourceRules.some((rule) => rule.path === scanRoot)) {
      throw new Error(`OpenCode adapter source guard must scan ${scanRoot}`);
    }
  }
  const opencodeSourceRegex = opencodeSourceRules[0]?.patterns?.[0]?.regex;
  if (
    !opencodeSourceRegex?.test('use bitfun_opencode_adapter as opencode;') ||
    !opencodeSourceRegex?.test('extern crate bitfun_opencode_adapter;') ||
    !opencodeSourceRegex?.test('bitfun_opencode_adapter::OpenCodePluginAdapter')
  ) {
    throw new Error('OpenCode adapter source guard must catch direct, alias, and extern imports');
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
  if (
    crateLayoutRules.find((rule) => rule.crateName === 'plugin-runtime-host')?.path !==
    'src/crates/execution/plugin-runtime-host'
  ) {
    throw new Error('plugin-runtime-host must be registered in the execution crate layout');
  }
  if (!noCoreDependencyCrates.includes('plugin-runtime-host')) {
    throw new Error('plugin-runtime-host must be covered by the no-core dependency guard');
  }
  const pluginRuntimeHostRule = lightweightBoundaryRules.find(
    (rule) => rule.crateName === 'plugin-runtime-host',
  );
  if (!pluginRuntimeHostRule?.forbiddenDeps.includes('bitfun-core')) {
    throw new Error('plugin-runtime-host lightweight boundary must forbid bitfun-core');
  }
  if (!pluginRuntimeHostRule?.forbiddenDeps.includes('bitfun-opencode-adapter')) {
    throw new Error('plugin-runtime-host must not depend on the OpenCode fixture adapter');
  }
  if (!pluginRuntimeHostRule?.forbiddenDeps.includes('bitfun-services-integrations')) {
    throw new Error('plugin-runtime-host must not depend on concrete service integrations');
  }
  const pluginRuntimeHostProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'plugin-runtime-host',
  );
  if (!pluginRuntimeHostProfile?.forbiddenNonOptionalDeps.includes('tauri')) {
    throw new Error('plugin-runtime-host dependency profile must forbid product surfaces');
  }
  const pluginRuntimeHostPublicApiRule = publicApiAllowlistRules.find(
    (rule) => rule.path === 'src/crates/execution/plugin-runtime-host/src/lib.rs',
  );
  const hostPublicSymbols = (pluginRuntimeHostPublicApiRule?.allowedSymbolEntries || []).map(
    (entry) => entry.symbol,
  );
  if (hostPublicSymbols.join(',') !== 'PluginHostAdapter,PluginRuntimeHost') {
    throw new Error('plugin-runtime-host public API budget must stay narrow');
  }
  const hasPluginRuntimeHostMethodBudgetRule = forbiddenContentRules.some(
    (rule) =>
      rule.path === 'src/crates/execution/plugin-runtime-host/src/lib.rs' &&
      rule.patterns.some((pattern) =>
        pattern.message.includes('unexpected public PluginRuntimeHost method') &&
        pattern.regex.test('pub fn restart(&self)') === false,
      ),
  );
  if (!hasPluginRuntimeHostMethodBudgetRule) {
    throw new Error('plugin-runtime-host public method budget forbidden rule is missing');
  }
  const hasPluginHostAdapterMethodBudgetRule = forbiddenContentRules.some(
    (rule) =>
      rule.path === 'src/crates/execution/plugin-runtime-host/src/adapter.rs' &&
      rule.patterns.some((pattern) =>
        pattern.message.includes('unexpected PluginHostAdapter trait method'),
      ),
  );
  if (!hasPluginHostAdapterMethodBudgetRule) {
    throw new Error('plugin-runtime-host adapter method budget forbidden rule is missing');
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
  if (!agentRuntimeRule?.forbiddenDeps.includes('bitfun-product-capabilities')) {
    throw new Error('agent-runtime lightweight boundary must forbid product assembly facts');
  }
  if (!agentRuntimeRule?.forbiddenDeps.includes('tool-runtime')) {
    throw new Error('agent-runtime lightweight boundary must forbid concrete tool runtime');
  }
  const agentRuntimeProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'agent-runtime',
  );
  if (!agentRuntimeProfile?.forbiddenNonOptionalDeps.includes('tauri')) {
    throw new Error('agent-runtime dependency profile must forbid product surface dependencies');
  }
  if (!agentRuntimeProfile?.forbiddenNonOptionalDeps.includes('bitfun-product-capabilities')) {
    throw new Error('agent-runtime dependency profile must forbid product assembly facts');
  }
  if (!agentRuntimeProfile?.forbiddenNonOptionalDeps.includes('tool-runtime')) {
    throw new Error('agent-runtime dependency profile must forbid concrete tool runtime');
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
  const serviceAgentRuntimeRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/service_agent_runtime.rs',
  );
  if (!serviceAgentRuntimeRuleText.includes('self\\.scheduler')) {
    throw new Error('service agent runtime boundary rule must forbid direct scheduler submit');
  }
  if (!serviceAgentRuntimeRuleText.includes('strip_remote_user_input_tags')) {
    throw new Error('service agent runtime boundary rule must forbid remote display cleanup ownership');
  }
  if (!serviceAgentRuntimeRuleText.includes('compress_remote_chat_data_url_for_mobile')) {
    throw new Error('service agent runtime boundary rule must forbid remote thumbnail compression ownership');
  }
  if (!serviceAgentRuntimeRuleText.includes('normalize_remote_session_model_id')) {
    throw new Error('service agent runtime boundary rule must forbid session model normalization ownership');
  }
  const serviceAgentRuntimeOwnerFeatureContracts = [
    'images',
    'image::load_from_memory',
    'JpegEncoder',
    "User's question",
    'AgentInputAttachment',
    'remote_image',
  ];
  for (const contract of serviceAgentRuntimeOwnerFeatureContracts) {
    if (!serviceAgentRuntimeRuleText.includes(contract)) {
      throw new Error(`service agent runtime boundary rule must forbid remote chat owner feature: ${contract}`);
    }
  }
  const sessionMessageRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/session_message_tool.rs',
  );
  if (!sessionMessageRuleText.includes('submit_with_prepended_messages')) {
    throw new Error('SessionMessage boundary rule must forbid direct scheduler submit');
  }
  const sessionMessageLegacySessionAccessContracts = [
    'resolve_session_workspace_binding',
    'list_sessions',
  ];
  for (const contract of sessionMessageLegacySessionAccessContracts) {
    if (!sessionMessageRuleText.includes(contract)) {
      throw new Error(`SessionMessage boundary rule must forbid direct coordinator ${contract}`);
    }
  }
  const sessionControlForbiddenRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/session_control_tool.rs',
  );
  if (!sessionControlForbiddenRuleText.includes('cancel_active_turn_for_session_from_requester')) {
    throw new Error('SessionControl boundary rule must forbid direct scheduler cancellation');
  }
  const sessionControlLegacySessionAccessContracts = [
    'resolve_session_workspace_binding',
    'list_sessions',
    'delete_session',
  ];
  for (const contract of sessionControlLegacySessionAccessContracts) {
    if (!sessionControlForbiddenRuleText.includes(contract)) {
      throw new Error(`SessionControl boundary rule must forbid direct coordinator ${contract}`);
    }
  }
  const cronServiceRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/service/cron/service.rs',
  );
  if (!cronServiceRuleText.includes('submit_with_prepended_messages')) {
    throw new Error('cron service boundary rule must forbid direct scheduler submit');
  }
  const cronToolRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/cron_tool.rs',
  );
  const cronToolLegacySessionAccessContracts = [
    'resolve_session_workspace_binding',
    'list_sessions',
  ];
  for (const contract of cronToolLegacySessionAccessContracts) {
    if (!cronToolRuleText.includes(contract)) {
      throw new Error(`CronTool boundary rule must forbid direct coordinator ${contract}`);
    }
  }
  const bashToolRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/bash_tool.rs',
  );
  if (!bashToolRuleText.includes('scheduler') || !bashToolRuleText.includes('deliver_background_result')) {
    throw new Error('Bash boundary rule must forbid direct scheduler background delivery');
  }
  const coordinatorRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/coordination/coordinator.rs',
  );
  if (
    !coordinatorRuleText.includes('scheduler') ||
    !coordinatorRuleText.includes('deliver_thread_goal_') ||
    !coordinatorRuleText.includes('deliver_background_result') ||
    !coordinatorRuleText.includes('CoreRuntimeServicesProvider::terminal_port') ||
    !coordinatorRuleText.includes('TerminalRuntimePort')
  ) {
    throw new Error(
      'Coordinator boundary rule must forbid direct scheduler delivery and terminal provider construction',
    );
  }
  const agenticSystemRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/system.rs',
  );
  if (
    !agenticSystemRuleText.includes('CoreRuntimeServicesProvider::terminal_port') ||
    !agenticSystemRuleText.includes('TerminalRuntimePort')
  ) {
    throw new Error('agentic system boundary rule must forbid terminal provider construction');
  }
  const coreFileReadStateRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/session/file_read_state.rs',
  );
  for (const contract of ['FileReadState', 'FileReadStateStore', 'DashMap']) {
    if (!coreFileReadStateRuleText.includes(contract)) {
      throw new Error(`core file_read_state boundary rule must forbid ${contract}`);
    }
  }
  const coreEvidenceLedgerRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/session/evidence_ledger.rs',
  );
  for (const contract of [
    'EvidenceLedgerTargetKind',
    'EvidenceLedgerEvent',
    'SessionEvidenceLedger',
    'CompressionContract',
    'uuid::Uuid::new_v4',
    'DashMap',
  ]) {
    if (!coreEvidenceLedgerRuleText.includes(contract)) {
      throw new Error(`core evidence_ledger boundary rule must forbid ${contract}`);
    }
  }
  const coreToolContextRuntimeRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/tool_context_runtime.rs',
  );
  for (const contract of ['LightCheckpoint', 'TerminalRuntimePort']) {
    if (!coreToolContextRuntimeRuleText.includes(contract)) {
      throw new Error(`core tool_context_runtime boundary rule must forbid ${contract}`);
    }
  }
  const coreUserInputManagerRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/user_input_manager.rs',
  );
  for (const contract of ['UserInputManager', 'oneshot::Sender', 'DashMap']) {
    if (!coreUserInputManagerRuleText.includes(contract)) {
      throw new Error(`core user_input_manager boundary rule must forbid ${contract}`);
    }
  }
  const coreToolPipelineRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/pipeline/tool_pipeline.rs',
  );
  for (const contract of ['ConfirmationResponse', 'oneshot::Sender', 'CancellationToken']) {
    if (!coreToolPipelineRuleText.includes(contract)) {
      throw new Error(`core tool_pipeline boundary rule must forbid ${contract}`);
    }
  }
  const coreRoundExecutorRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/execution/round_executor.rs',
  );
  for (const contract of ['CancellationToken', 'DashMap']) {
    if (!coreRoundExecutorRuleText.includes(contract)) {
      throw new Error(`core round_executor boundary rule must forbid ${contract}`);
    }
  }
  const coreBackgroundCommandOutputRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/background_command_output.rs',
  );
  for (const contract of [
    'BackgroundCommandOutputCapture',
    'BackgroundCommandOutputStatus',
    'VecDeque',
    'mpsc::UnboundedSender',
    'tokio::spawn',
  ]) {
    if (!coreBackgroundCommandOutputRuleText.includes(contract)) {
      throw new Error(`core background_command_output boundary rule must forbid ${contract}`);
    }
  }
  const coreExecCommandRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/command.rs',
  );
  for (const contract of [
    'POWERSHELL_UTF8_OUTPUT_PREFIX',
    'REMOTE_NON_TTY_INTERRUPT_GRACE_SECONDS',
    'DEFAULT_TOOL_YIELD_TIME_MS',
    '(?:local|remote)_completion_value',
    '(?:local|remote)_completion',
    '(?:local|remote)_background_output_status_for_completion',
    'merged_remote_env',
    'remote_command_env_words',
    'shell_escape',
    'is_plausible_remote_shell_path',
    'getent\\s+passwd',
    'command\\s+-v\\s+bash',
      'REMOTE_SHELL_PROBE_TIMEOUT_MS',
      'get_global_exec_process_manager',
      'get_global_remote_exec_process_manager',
      'SSHConnectionManager',
      'SSHCommandOptions',
      'LocalExecCommandRequest',
      'CoreRuntimeServicesProvider::terminal_port',
      'TerminalRuntimePort',
    ]) {
      if (!coreExecCommandRuleText.includes(contract)) {
        throw new Error(`core exec_command boundary rule must forbid ${contract}`);
      }
    }
  const coreWriteStdinRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/stdin.rs',
  );
  for (const contract of [
    'DEFAULT_TOOL_YIELD_TIME_MS',
    '(?:local|remote)_completion_value',
    '(?:local|remote)_completion',
    '"status"\\s*:\\s*"session_not_found"',
      'No input was sent',
      'get_global_exec_process_manager',
      'get_global_remote_exec_process_manager',
      'RemoteExecError',
      'LocalWriteStdinRequest',
      'CoreRuntimeServicesProvider::terminal_port',
      'TerminalRuntimePort',
    ]) {
      if (!coreWriteStdinRuleText.includes(contract)) {
        throw new Error(`core WriteStdin boundary rule must forbid ${contract}`);
      }
    }
  const coreExecControlRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/control.rs',
  );
  if (!coreExecControlRuleText.includes('exec_command_control_action_name')) {
    throw new Error('core ExecControl boundary rule must forbid action result shaping');
  }
  if (!coreExecControlRuleText.includes('(?:local|remote)_completion')) {
    throw new Error('core ExecControl boundary rule must forbid duplicated completion mapping');
  }
    for (const contract of [
      'get_global_exec_process_manager',
      'get_global_remote_exec_process_manager',
      'RemoteExecError',
      'LocalExecControlRequest',
      'CoreRuntimeServicesProvider::terminal_port',
      'TerminalRuntimePort',
    ]) {
      if (!coreExecControlRuleText.includes(contract)) {
        throw new Error(`core ExecControl boundary rule must forbid ${contract}`);
      }
    }
  const coreExecCommandInputRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/input.rs',
  );
    for (const contract of [
      'get_global_exec_process_manager',
      'get_global_remote_exec_process_manager',
      'LocalSendStdinRequest',
      'CoreRuntimeServicesProvider::terminal_port',
      'TerminalRuntimePort',
    ]) {
      if (!coreExecCommandInputRuleText.includes(contract)) {
        throw new Error(`core ExecCommand input boundary rule must forbid ${contract}`);
      }
    }
  const coreExecCommandEnvSnapshotRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/env_snapshot.rs',
  );
  for (const contract of [
    'ENV_SNAPSHOT_BEGIN',
    'ENV_SNAPSHOT_END',
    'should_import_env_var',
    'is_valid_env_var_name',
    'is_volatile_env_var',
    'shell_escape',
    'ENV_SNAPSHOT_TIMEOUT_MS',
    'ENV_SNAPSHOT_MAX_OUTPUT_CHARS',
    'ENV_SNAPSHOT_TTL',
    'get_global_remote_exec_process_manager',
    'SSHConnectionManager',
  ]) {
    if (!coreExecCommandEnvSnapshotRuleText.includes(contract)) {
      throw new Error(`core exec_command env_snapshot boundary rule must forbid ${contract}`);
    }
  }
  const coreSkillAgentSnapshotRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/skill_agent_snapshot.rs',
  );
  for (const contract of [
    'SkillSnapshotEntry',
    'AgentSnapshotEntry',
    'TurnSkillAgentSnapshot',
    'SkillAgentDiff',
    'diff_skill_agent_snapshot',
    'render_titled_skill_entries',
  ]) {
    if (!coreSkillAgentSnapshotRuleText.includes(contract)) {
      throw new Error(`core skill_agent_snapshot boundary rule must forbid ${contract}`);
    }
  }
  const coreTurnSkillAgentSnapshotStoreRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/session/turn_skill_agent_snapshot_store.rs',
  );
  for (const contract of ['TurnSkillAgentSnapshotStore', 'DashMap', 'BTreeMap']) {
    if (!coreTurnSkillAgentSnapshotStoreRuleText.includes(contract)) {
      throw new Error(`core turn_skill_agent_snapshot_store boundary rule must forbid ${contract}`);
    }
  }

  const requiredContentContracts = [
    {
      path: 'src/crates/contracts/runtime-ports/src/lib.rs',
      contracts: [
        'AgentDialogTurnRequest',
        'AgentDialogPrependedReminder',
        'AgentDialogTurnPort',
        'AgentBackgroundResultRequest',
        'AgentThreadGoalDeliveryKind',
        'AgentThreadGoalDeliveryRequest',
        'AgentLifecycleDeliveryPort',
        'agent_dialog_turn_request_serializes_lifecycle_contract',
        'agent_background_result_request_serializes_lifecycle_contract',
        'agent_thread_goal_delivery_request_serializes_lifecycle_contract',
        'AgentTurnCancellationPort',
        'requester_session_id',
        'AgentSessionListRequest',
        'AgentSessionSummary',
        'AgentSessionDeleteRequest',
        'AgentSessionWorkspaceRequest',
        'AgentSessionManagementPort',
        'agent_session_management_contracts_serialize_stable_shape',
        'RemoteControlStatePort',
        'RuntimeEventSink',
        'AgentSessionCreateResult',
        'session_name',
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
        'RuntimeServiceMarkerPort',
        'RuntimeServicesProvider',
        'RuntimeServicesRegistry',
        'CapabilityMismatch',
        'require_capability',
      ],
    },
    {
      path: 'src/crates/execution/runtime-services/src/backend_events.rs',
      contracts: [
        'BackendEvent',
        'BackendEventSystem',
        'event_name',
        'emit',
        'get_global_event_system',
        'backend_event_names_remain_stable',
      ],
    },
    {
      path: 'src/crates/contracts/events/src/frontend_projection.rs',
      contracts: [
        'AgenticFrontendEvent',
        'project_agentic_frontend_event',
        'legacy_flat_message',
        'deep_review_queue_projection_preserves_camel_case_contract',
        'legacy_flat_message_keeps_projection_type_authoritative',
        'legacy_flat_dialog_turn_started_preserves_existing_shape',
      ],
    },
    {
      path: 'src/crates/contracts/events/src/agentic_projection_manifest.rs',
      contracts: [
        'AGENTIC_EVENT_PROJECTION_MANIFEST',
        'public_agentic_event_projection_manifest',
        'is_legacy_websocket_agentic_event_type',
        'public_event_projection_manifest_describes_projected_events_and_websocket_allowlist',
      ],
    },
    {
      path: 'src/crates/adapters/transport/src/adapters/tauri.rs',
      contracts: ['project_agentic_frontend_event', 'projected.event_name.as_str()'],
    },
    {
      path: 'src/crates/adapters/transport/src/adapters/websocket.rs',
      contracts: [
        'project_agentic_frontend_event',
        'is_legacy_websocket_agentic_event_type',
        'websocket_keeps_legacy_agentic_event_allowlist',
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
        'marker_ports_register_optional_service_availability_without_core_dependency',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/runtime.rs',
      contracts: [
        'AgentRuntime',
        'AgentRuntimeBuilder',
        'AgentSubmissionPort',
        'AgentDialogTurnPort',
        'with_dialog_turn_port',
        'submit_dialog_turn',
        'AgentLifecycleDeliveryPort',
        'with_lifecycle_delivery_port',
        'deliver_background_result',
        'deliver_thread_goal',
        'AgentTurnCancellationPort',
        'AgentSessionManagementPort',
        'with_session_management_port',
        'MissingSessionManagementPort',
        'list_sessions',
        'delete_session',
        'resolve_session_workspace_binding',
        'session_management_delegates_to_registered_port',
        'RuntimeServices',
        'RuntimeEventEnvelope',
        'AgentEventStream',
        'with_event_stream',
        'SessionSelector',
        'AgentRunRequest',
        'AgentRunHandle',
        'run',
        'publish_event',
        'publish_event_uses_runtime_services_event_sink',
        'run_handle_exposes_configured_agent_event_stream',
        'port_errors_remain_typed',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/sdk.rs',
      contracts: [
        'AGENT_RUNTIME_SDK_API_VERSION',
        '#[non_exhaustive]',
        'AgentRuntimeSdkStability',
        'AgentRuntimeSdkCompatibility',
        'impl AgentRuntimeSdkCompatibility',
        'bitfun_agent_tools',
        'bitfun_harness',
        'bitfun_runtime_services',
        'PortResult',
        'RuntimeServicePort',
        'FileSystemPort',
        'RemoteWorkspacePort',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/Cargo.toml',
      contracts: ['[features]', 'default = []'],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/context_profile.rs',
      contracts: [
        'ContextProfile',
        'ModelCapabilityProfile',
        'ContextProfilePolicy',
        'for_subagent_context_and_models',
        'model_capability_weak_for_mini',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/session_state.rs',
      contracts: [
        'SessionState',
        'ProcessingPhase',
        'dialog_state_fact',
        'session_state_label_for_state',
        'processing_state_serialization_stays_compatible',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/session.rs',
      contracts: [
        'Session',
        'SessionConfig',
        'SessionSummary',
        'SessionKind',
        'PersistedSessionStateFile',
        'sanitize_persisted_session_state',
        'persisted_session_state_file_shape_stays_compatible',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/dialog_turn.rs',
      contracts: ['new_turn_id', 'TurnStats'],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/side_question.rs',
      contracts: [
        'SideQuestionRuntime',
        'ActiveBtwTurn',
        'register_btw_turn',
        'registering_same_request_cancels_previous_token',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/sdk_smoke.rs',
      contracts: [
        'sdk_facade_exposes_versioned_preview_compatibility_contract',
        'sdk_facade_runs_with_fake_provider_and_local_event_stream',
        'sdk_facade_accepts_fake_services_tools_harnesses_and_hooks_without_core',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/examples/sdk_minimal.rs',
      contracts: [
        'bitfun_agent_runtime::sdk',
        'AgentRuntimeSdkCompatibility::current',
        'impl AgentSubmissionPort for ExampleAgentProvider',
        'AgentRuntimeBuilder::new',
        'AgentRunRequest::new',
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
      path: 'src/crates/execution/agent-runtime/src/custom_agent.rs',
      contracts: [
        'CustomAgentKind',
        'CustomAgentDiscoveryRoots',
        'CustomAgentLoadReport',
        'CustomAgentDefinition',
        'CustomAgentDefinitionError',
        'DEFAULT_CUSTOM_MODE_TOOLS',
        'DEFAULT_CUSTOM_SUBAGENT_TOOLS',
        'custom_agent_read_markdown_file',
        'custom_agent_save_markdown_file',
        'custom_agent_possible_dirs',
        'load_custom_agent_definitions',
        'CustomAgentValidationContext',
        'CustomAgentValidationReport',
        'CustomAgentModelFallback',
        'validate_custom_agent_definition',
        'custom_agent_review_writable_tools',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/custom_subagent.rs',
      contracts: [
        'pub type CustomSubagentKind = CustomAgentLevel',
        'pub type CustomSubagentDefinition = CustomAgentDefinition',
        'pub type CustomSubagentDiscoveryRoots = CustomAgentDiscoveryRoots',
        'load_custom_subagent_definitions',
        'custom_agent_read_markdown_file',
        'custom_agent_save_markdown_file',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/custom_subagent_discovery_contracts.rs',
      contracts: [
        'custom_subagent_discovery_preserves_bitfun_priority_and_ignores_foreign_agent_dirs',
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
        'custom_subagent_markdown_io_writes_canonical_front_matter',
        'custom_subagent_markdown_parse_errors_match_legacy_prefixes',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/post_call_hooks.rs',
      contracts: [
        'RuntimeHookKind',
        'RuntimeHookErrorPolicy',
        'RuntimeHookPlan',
        'RuntimeHookRegistry',
        'EmptyHookId',
        'InvalidTimeoutMillis',
        'successful_tool_post_call_hooks',
        'SuccessfulToolPostCallHookExecutor',
        'run_successful_tool_post_call_hooks',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/post_call_hook_contracts.rs',
      contracts: [
        'successful_tool_call_routes_to_shared_context_measurement_hook',
        'runtime_hook_registry_preserves_order_timeout_and_error_policy',
        'runtime_hook_registry_rejects_duplicate_ids',
        'runtime_hook_registry_rejects_unstable_ids_and_zero_timeouts',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/post_call_hook_execution_contracts.rs',
      contracts: ['successful_tool_post_call_executor_runs_deep_review_measurement_route'],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/tool_confirmation.rs',
      contracts: [
        'ToolConfirmationRequestFacts',
        'ToolConfirmationGateFacts',
        'ToolConfirmationGatePlan',
        'ToolConfirmationPlan',
        'ToolConfirmationOutcome',
        'ToolConfirmationWaitResult',
        'ToolConfirmationResponse',
        'ToolConfirmationChannelStore',
        'ConfirmationFailureKind',
        'resolve_tool_confirmation_gate',
        'resolve_tool_confirmation_plan',
        'resolve_confirmation_failure',
        'resolve_confirmation_wait_result',
        'confirmation_channel_store_delivers_confirmation_once',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/user_questions.rs',
      contracts: [
        'AskUserQuestionInput',
        'UserInputResponse',
        'UserInputManager',
        'get_user_input_manager',
        'validate_ask_user_question_input',
        'user_input_manager_delivers_answer_and_clears_channel',
        'user_input_manager_cancel_closes_receiver',
      ],
    },
    {
      path: 'src/crates/execution/tool-execution/src/background_command_output.rs',
      contracts: [
        'BackgroundCommandOutputCapture',
        'BackgroundCommandOutputStatus',
        'BackgroundCommandOutputMetadata',
        'background_command_output_capture',
        'BACKGROUND_COMMAND_OUTPUT_CAPTURE_LIMIT_BYTES',
        'background_command_output_reads_snapshot_then_incremental_chunks',
      ],
    },
    {
      path: 'src/crates/execution/tool-execution/src/exec_command.rs',
      contracts: [
        'EXEC_COMMAND_POWERSHELL_UTF8_OUTPUT_PREFIX',
        'EXEC_COMMAND_DEFAULT_YIELD_TIME_MS',
        'ExecCommandShellKind',
        'Custom',
        'ExecCommandRemoteShell',
        'REMOTE_EXEC_SHELL_PROBE_TIMEOUT_MS',
        'remote_exec_shell_probe_command',
        'fallback_remote_exec_shell',
        'ExecCommandRemoteEnvSnapshot',
        'ExecCommandRemoteEnvSnapshotCapturePolicy',
        'ExecCommandRemoteEnvSnapshotCacheKey',
        'ExecCommandRemoteEnvSnapshotCache',
        'remote_exec_env_snapshot_capture_policy',
        'ExecCommandRunInput',
        'WriteStdinInput',
        'ExecCommandControlToolInput',
        'ExecCommandResultFields',
        'ExecCommandShellMetadata',
        'exec_command_argv_for_shell',
        'exec_command_result_value',
        'write_stdin_result_value',
        'write_stdin_session_not_found_result',
        'exec_control_result_value',
        'remote_exec_login_shell_command',
        'remote_exec_non_tty_control_wrapper',
        'parse_remote_exec_env_snapshot_output',
        'ExecCommandLifecycleStatus',
        'exec_command_lifecycle_background_output_status',
        'remote_login_shell_command_applies_snapshot_then_tool_env',
        'remote_shell_probe_and_env_snapshot_are_provider_neutral',
        'exec_command_input_policy_applies_defaults_without_trimming_command',
        'write_stdin_input_policy_applies_poll_defaults',
        'exec_control_input_policy_keeps_wait_optional',
        'exec_command_result_builder_preserves_existing_wire_shape',
        'write_stdin_and_control_result_builders_preserve_remote_shape',
        'remote_env_snapshot_capture_policy_keeps_existing_bounds',
        'remote_env_snapshot_cache_owns_key_and_ttl_policy',
        'remote_shell_probe_skips_non_posix_shells',
        'lifecycle_status_has_provider_neutral_names_and_background_statuses',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/completion.rs',
      contracts: [
        'exec_command_local_completion',
        'exec_command_remote_completion',
        'ExecCommandCompletionStatus::Interrupted',
        'ExecCommandCompletionSource::OutOfBandControl',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/command.rs',
      contracts: [
        'exec_command_argv_for_shell',
        'parse_remote_exec_shell_probe_output',
        'remote_exec_shell_probe_command',
        'fallback_remote_exec_shell',
        'remote_exec_login_shell_command',
        'remote_exec_non_tty_control_wrapper',
        'exec_command_lifecycle_status_name',
        'exec_command_lifecycle_background_output_status',
        'exec_command_run_input_from_input',
        'exec_command_result_value',
        'exec_command_local_completion',
        'exec_command_remote_completion',
        'ExecCommandShellMetadata',
        'remote_shell_probe_preserves_unknown_shell_metadata',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/stdin.rs',
      contracts: [
        'write_stdin_input_from_input',
        'write_stdin_input_validation_message',
        'write_stdin_result_value',
        'write_stdin_session_not_found_result',
        'exec_command_local_completion',
        'exec_command_remote_completion',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/control.rs',
      contracts: [
        'exec_command_control_tool_input_from_input',
        'exec_command_control_tool_input_validation_message',
        'exec_control_result_value',
        'exec_command_local_completion',
        'exec_command_remote_completion',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/env_snapshot.rs',
      contracts: [
        'remote_exec_env_snapshot_command',
        'parse_remote_exec_env_snapshot_output',
        'remote_exec_env_snapshot_capture_policy',
        'ExecCommandRemoteEnvSnapshotCache',
        'ExecCommandRemoteEnvSnapshotCacheKey',
        'exec_command_shell_kind',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/exec_command/shell_kind.rs',
      contracts: [
        'exec_command_shell_kind',
        'terminal_shell_type',
        'ExecCommandShellKind::Custom(name.clone())',
        'ShellType::Custom(name)',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/tool_confirmation_contracts.rs',
      contracts: [
        'confirmation_gate_preserves_skip_policy_precedence',
        'confirmation_gate_requires_confirmation_only_for_permissioned_tools',
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
        'dialog_turn_queue_clear_and_requeue_front_preserve_scheduler_recovery_contract',
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
      path: 'src/crates/execution/agent-runtime/src/skill_agent_snapshot.rs',
      contracts: [
        'SkillSnapshotEntry',
        'AgentSnapshotEntry',
        'TurnSkillAgentSnapshot',
        'SkillAgentDiff',
        'diff_skill_agent_snapshot',
        'build_skill_agent_tool_listing_sections_from_snapshot',
        'TurnSkillAgentSnapshotStore',
        'skill_agent_diff_renders_changed_added_and_removed_entries',
        'latest_snapshot_at_or_before_returns_nearest_sparse_snapshot',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/file_read_state.rs',
      contracts: [
        'FileReadState',
        'is_full_file_read',
        'FileReadStateStore',
        'file_read_state_accepts_nonempty_whole_file',
        'file_read_state_store_scopes_entries_by_session',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/evidence_ledger.rs',
      contracts: [
        'EvidenceLedgerTargetKind',
        'EvidenceLedgerEventStatus',
        'EvidenceLedgerEvent',
        'EvidenceLedgerSummary',
        'SessionEvidenceLedger',
        'impl From<EvidenceLedgerSummary> for CompressionContract',
        'impl From<LightCheckpoint> for EvidenceLedgerCheckpoint',
        'ledger_reads_events_scoped_by_session_and_turn',
        'checkpoint_from_light_checkpoint_preserves_recovery_boundary_metadata',
        'summary_projects_into_compression_contract',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/turn_cancellation.rs',
      contracts: [
        'DialogTurnCancellationTokenStore',
        'get_or_insert_new',
        'is_cancelled',
        'turn_cancellation_store_reuses_existing_token',
        'turn_cancellation_store_cancels_registered_token',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/execution/round_executor.rs',
      contracts: ['DialogTurnCancellationTokenStore', 'get_or_insert_new', 'is_cancelled'],
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
      path: 'src/crates/execution/agent-runtime/src/event_queue.rs',
      contracts: ['EventQueue', 'impl StreamEventSink for EventQueue', 'clear_session'],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/session_state_manager.rs',
      contracts: [
        'pub struct SessionStateManager',
        'DashMap<String, SessionState>',
        'EventQueue',
        'AgenticEvent::SessionStateChanged',
        'session_state_label_for_state',
        'can_start_new_turn',
        'session_state_manager_emits_compatible_state_change_events',
        'session_state_manager_keeps_turn_start_guard_semantics',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/coordination/state_manager.rs',
      contracts: ['pub use bitfun_agent_runtime::session_state_manager::SessionStateManager'],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/event_router.rs',
      contracts: ['EventSubscriber', 'EventRouter', 'route_batch'],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/prompt_markup.rs',
      contracts: [
        'PromptEnvelope',
        'render_user_query',
        'strip_prompt_markup',
        'strips_current_and_legacy_system_reminder_suffix',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/remote_file_delivery.rs',
      contracts: [
        'TOOL_CONTEXT_REMOTE_FILE_DELIVERY_KEY',
        'remote_file_delivery_reminder',
        'user_workspace_relative_file_link',
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
        'CoreServiceAgentRuntime::agent_runtime_with_dialog_turns',
        'AgentDialogTurnRequest',
        'AgentDialogPrependedReminder',
        'submit_dialog_turn',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/cron_tool.rs',
      contracts: [
        'CoreServiceAgentRuntime::agent_runtime',
        'AgentSessionListRequest',
        'AgentSessionWorkspaceRequest',
        'list_sessions',
        'resolve_session_workspace_binding',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/execution/types.rs',
      contracts: ['bitfun_agent_runtime::events::FinishReason'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/events/types.rs',
      contracts: [
        'bitfun_agent_runtime::session_state::session_state_label_for_state',
      ],
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
      path: 'src/crates/assembly/core/src/agentic/persistence/session_branch.rs',
      contracts: ['SessionBranchRequest', 'SessionBranchResult', 'build_branched_session_metadata'],
    },
    {
      path: 'src/crates/services/services-core/src/session/mod.rs',
      contracts: [
        'mod metadata_store',
        'mod migration',
        'SessionMetadataStore',
        'merge_legacy_session_store',
      ],
    },
    {
      path: 'src/crates/services/services-core/src/session/migration.rs',
      contracts: [
        'merge_legacy_session_store',
        'merge_session_metadata_file',
        'SessionMetadataStore::new',
        'metadata_file_count',
        'merge_legacy_session_store_preserves_newer_metadata_and_rebuilds_visible_index',
      ],
    },
    {
      path: 'src/crates/services/services-core/src/session/metadata_store.rs',
      contracts: [
        'SessionMetadataStore',
        'SessionMetadataStoreError',
        'SessionStorageLayout',
        'list_metadata',
        'list_metadata_page',
        'list_metadata_including_internal',
        'rebuild_index',
        'save_metadata',
        'load_metadata',
        'delete_session_dir_and_index',
        'metadata_store_saves_visible_metadata_and_updates_index',
        'metadata_store_rebuilds_stale_index_entries',
        'metadata_store_rebuild_index_counts_hidden_metadata_files',
        'metadata_store_delete_session_updates_visible_index',
      ],
    },
    {
      path: 'src/crates/services/services-core/src/managed_runtime.rs',
      contracts: [
        'ManagedRuntimeResolver',
        'RuntimeSource',
        'resolve_command',
        'merged_path_env',
        'normalizes_windows_alias_for_managed_lookup',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/service/runtime/mod.rs',
      contracts: ['ManagedRuntimeResolver::new', 'get_path_manager_arc'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/persistence/manager.rs',
      contracts: [
        'SessionMetadataStore',
        'session_metadata_store',
        'list_metadata',
        'list_metadata_page',
        'list_metadata_including_internal',
        'save_metadata',
        'load_metadata',
        'delete_session_dir_and_index',
        'ensure_runtime_for_write',
      ],
    },
    {
      path: 'src/crates/assembly/product-capabilities/src/lib.rs',
      contracts: [
        'HarnessProviderDescriptor',
        'build_descriptor_harness_registry',
        'ProductCapabilityAssembly',
        'ProductFeatureGroup',
        'ProductRuntimeAssembly',
        'DeliveryProfile::Sdk',
        'into_runtime_parts',
        'feature_groups_from_tool_provider_group_plan',
      ],
    },
    {
      path: 'src/crates/assembly/product-capabilities/tests/product_capabilities.rs',
      contracts: [
        'product_assembly_plan_exposes_build_feature_groups_explicitly',
        'product_runtime_assembly_reports_runtime_service_capability_gaps',
        'product_harness_provider_plans_legacy_facade_without_execution',
      ],
    },
    {
      path: 'src/crates/assembly/product-capabilities/tests/plugin_product_shape.rs',
      contracts: [
        'p0_plugin_host_is_executable_only_for_product_full_desktop_and_cli',
        'p0_plugin_host_binding_builds_agent_runtime_parts',
        'non_p0_surfaces_cannot_inherit_executable_plugin_host',
        'default_product_shapes_expose_only_disabled_plugin_availability',
        'default_assembled_product_shapes_keep_profile_specific_plugin_availability',
      ],
    },
    {
      path: 'src/crates/execution/plugin-runtime-host/tests/plugin_runtime_host.rs',
      contracts: [
        'host_dispatches_candidates',
        'host_replays_idempotent_dispatch_without_recalling_adapter',
        'concurrent_idempotent_dispatch_reuses_in_flight_response',
        'concurrent_cross_key_dispatch_observes_active_quarantine_before_success',
        'idempotent_dispatch_cache_is_scoped_by_project_workspace_and_source',
        'idempotent_dispatch_cache_does_not_replay_across_events',
        'idempotent_dispatch_cache_is_scoped_by_epoch_changes',
        'idempotent_dispatch_cache_evicts_old_entries',
        'read_model_is_scoped_by_project_and_workspace',
        'read_model_rejects_wrong_workspace_response',
        'active_quarantine_blocks_new_dispatches_until_host_restart',
        'active_quarantine_blocks_malformed_follow_up_without_new_quarantine',
        'malformed_dispatch_with_missing_identity_observes_active_quarantine',
        'host_owned_quarantine_is_visible_in_read_model_with_diagnostics',
        'host_restart_clears_domain_quarantine_and_cached_dispatch',
        'zero_deadline_quarantines_without_adapter_dispatch',
        'malformed_dispatch_envelope_quarantines_without_adapter_dispatch',
        'nonzero_deadline_timeout_quarantines_without_success_effects',
        'adapter_failure_quarantines_without_writing_success',
        'malformed_adapter_success_quarantines_without_effects',
        'permission_prompt_target_mismatch_quarantines_without_effects',
        'final_policy_decision_from_adapter_fails_closed',
        'adapter_id_or_quarantine_with_effects_mismatch_fails_closed',
        'status_quarantine_with_success_effects_fails_closed',
        'disposed_project_rejects_dispatch_and_read_model_reports_statuses',
      ],
    },
    {
      path: 'src/crates/assembly/product-capabilities/tests/product_sdk_assembly.rs',
      contracts: [
        'product_runtime_parts_can_build_agent_runtime_sdk_without_core',
        'sdk_delivery_profile_builds_minimal_agent_runtime_without_product_full_capabilities',
        'DeliveryProfile::Cli',
        'DeliveryProfile::Sdk',
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
      contracts: [
        'tool_restrictions_for_delegation_policy',
        'miniapp_headless_agent_tool_restrictions',
        'impl From<ToolRestrictionError> for BitFunError',
        'is_local_path_within_root',
      ],
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
        'ToolTaskStateKind',
        'should_cancel_tool_state',
        'summarize_dialog_turn_cancellation',
        'ToolCancellationTokenStore',
        'count_tool_states',
        'ToolStateEventFacts',
        'ToolStateEventKind',
        'tool_state_event_data',
        'sanitize_tool_result_for_event',
      ],
    },
    {
      path: 'src/crates/execution/tool-execution/tests/tool_pipeline_planning.rs',
      contracts: [
        'partitions_consecutive_concurrency_safe_tools_into_parallel_batches',
        'retry_policy_preserves_attempt_limit_and_error_class_contract',
        'cancellation_policy_preserves_cancellable_and_terminal_state_contract',
        'dialog_turn_cancellation_summary_counts_cancelled_and_skipped_tasks',
        'cancellation_token_store_cancels_and_removes_tokens',
        'state_counts_preserve_pipeline_stats_contract',
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
        'AgentSessionManagementPort',
        'runtime_session_summary',
        'AgentSessionSummary',
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
        'RoundInjection',
        'RoundInjectionKind',
        'RoundInjectionTarget',
        'SessionRoundInjectionBuffer',
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
        'normalize_remote_model_selection',
        'normalize_remote_model_selection_contract',
        'remote_chat_messages_from_turns',
        'RemoteDialogSchedulerOutcomeFact',
        'remote_dialog_submit_outcome_from_scheduler',
        'RemoteChatHistoryTurn',
        'build_remote_chat_messages',
        'project_remote_chat_user',
        'load_remote_chat_messages',
        'agent_runtime',
        'agent_runtime_with_dialog_turns',
        'agent_runtime_with_lifecycle_delivery',
        'agent_runtime_with_scheduler_ports',
        'global_agent_runtime_with_lifecycle_delivery',
        'with_lifecycle_delivery_port',
        'agent_input_attachment_from_image_context',
        'AgentDialogTurnRequest',
        'submit_dialog_turn',
        'AgentRuntimeBuilder',
        'remote_control_state_port',
        'CoreRemoteDialogRuntimeHost',
        'CoreRemoteCancelRuntimeHost',
        'CoreRemoteCancelRuntimeHost\\s*\\{[\\s\\S]*\\bruntime:\\s*AgentRuntime',
        'CoreServiceAgentRuntime::agent_runtime_with_scheduler_ports',
        'CoreRemoteWorkspaceFileRuntimeHost',
        'CoreRemoteWorkspaceRuntimeHost',
        'CoreRemoteSessionRuntimeHost',
        'CoreRemoteSessionRuntimeHost\\s*\\{[\\s\\S]*\\bruntime:\\s*AgentRuntime',
        'CoreRemotePollRuntimeHost',
        'CoreRemoteInteractionRuntimeHost',
        'CoreRemoteSessionTrackerHost',
        'RemoteExecutionDispatcher',
        'ImageContextData',
        'agent_input_attachment_from_remote_image_context',
        'AgentSubmissionPort',
        'AgentDialogTurnPort',
        'AgentTurnCancellationPort',
        'AgentSessionManagementPort',
        'with_session_management_port',
        'RemoteControlStatePort',
        'SessionTranscriptReader',
        'core_service_agent_runtime_owner_keeps_coordinator_port_contracts',
        'core_service_agent_runtime_owner_normalizes_remote_session_model_ids',
        'core_service_agent_runtime_owner_normalizes_remote_model_selection_aliases',
        'core_service_agent_runtime_owner_preserves_remote_chat_history_shape',
        'core_service_agent_runtime_owner_skips_in_progress_remote_assistant_history',
        'core_service_agent_runtime_owner_maps_image_context_to_lifecycle_attachment',
        'core_service_agent_runtime_owner_keeps_scheduler_lifecycle_port_contracts',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/session_control_tool.rs',
      contracts: [
        'CoreServiceAgentRuntime::agent_runtime',
        'CoreServiceAgentRuntime::agent_runtime_with_scheduler_ports',
        'AgentSessionCreateRequest',
        'AgentSessionListRequest',
        'AgentSessionDeleteRequest',
        'AgentSessionWorkspaceRequest',
        'list_sessions',
        'delete_session',
        'resolve_session_workspace_binding',
        '"createdBy"',
        'AgentTurnCancellationRequest',
        'requester_session_id',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/session_message_tool.rs',
      contracts: [
        'CoreServiceAgentRuntime::agent_runtime_with_dialog_turns',
        'AgentSessionCreateRequest',
        'AgentSessionListRequest',
        'AgentSessionWorkspaceRequest',
        'list_sessions',
        'resolve_session_workspace_binding',
        '"createdBy"',
        'AgentDialogTurnRequest',
        'AgentDialogPrependedReminder',
        'submit_dialog_turn',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_connect.rs',
      contracts: [
        'pub mod device',
        'pub mod encryption',
        'pub mod pairing',
        'pub mod qr_generator',
        'pub mod relay_client',
        'pub use device::DeviceIdentity',
        'pub use encryption::{decrypt_from_base64, encrypt_to_base64, KeyPair}',
        'PairingProtocol',
        'QrPayload',
        'pub use qr_generator::QrGenerator',
        'RelayClient',
        'RelayMessage',
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
        'RemoteChatUserProjection',
        'project_remote_chat_user',
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
        'remote_connect_pairing_primitives_live_in_services_owner',
        'remote_connect_qr_and_relay_primitives_live_in_services_owner',
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
      contracts: [
        'remote_queue_policy_preserves_confirmation_boundary',
        'AgentDialogTurnPort',
        'AgentLifecycleDeliveryPort',
        'AgentTurnCancellationPort',
        'AgentBackgroundResultRequest',
        'AgentThreadGoalDeliveryRequest',
        'AgentThreadGoalDeliveryKind::ObjectiveUpdated',
        'cancel_active_turn_for_session_from_requester',
        'agent_dialog_turn_image_contexts',
        'agent_dialog_turn_prepended_messages',
        'agent_dialog_turn_attachments_preserve_remote_image_context',
        'agent_dialog_turn_attachments_reject_unknown_kind',
        'agent_dialog_turn_prepended_reminders_preserve_session_message_kind',
        'agent_dialog_turn_prepended_reminders_reject_unknown_kind',
      ],
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
        'product_tool_runtime_keeps_no_direct_core_profiles_empty',
        'DeliveryProfile::Sdk',
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
        'create_registry_from_static_provider_entries',
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
        'materialized_tool_snapshot',
        'create_registry_from_static_provider_plans',
        'create_registry_from_static_provider_entries',
        'ToolCatalogRuntime',
        'ToolDecoratorRef',
        'SnapshotToolWrapper',
        'SnapshotToolDecorator',
        'create_registry_from_static_providers',
        'install_static_provider',
        'resolve_readonly_enabled_tools',
        'build_get_tool_spec_duplicate_load_result',
        'build_get_tool_spec_detail_result',
        'miniapp_headless_agent_tool_restrictions',
        'tool_restrictions_for_delegation_policy',
        'denied_tool_messages',
        'resolve_get_tool_spec_execution_plan',
        'resolve_get_tool_spec_execution_result_from_provider',
        'GetToolSpecRuntime',
        'call_results',
      ],
    },
    {
      path: 'src/crates/execution/tool-contracts/src/tool_snapshot.rs',
      contracts: [
        'MaterializedToolSnapshot',
        'ToolProviderIdentity',
        'ToolEffectFacts',
        'ToolEffectFactsSource',
        'ToolEffectFilter',
        'ToolCallSnapshotGuard',
        'ToolSnapshotCallError',
        'materialize_tool_snapshot',
      ],
    },
    {
      path: 'src/crates/execution/tool-contracts/src/mcp_tool_bridge.rs',
      contracts: [
        'build_mcp_tool_bridge_name',
        'McpToolBridgeDefinition',
        'McpToolBridgeBehaviorHints',
        'build_mcp_tool_bridge_definition',
        'mcp_tool_bridge_dynamic_tool_info',
        'validate_mcp_tool_bridge_input',
        'render_mcp_tool_bridge_use_message',
        'render_mcp_tool_bridge_rejected_message',
        'render_mcp_tool_bridge_result_message',
        'build_mcp_tool_bridge_result',
      ],
    },
    {
      path: 'src/crates/execution/tool-contracts/src/acp_tool_bridge.rs',
      contracts: [
        'build_acp_external_agent_tool_name',
        'AcpExternalAgentToolDefinition',
        'build_acp_external_agent_tool_definition',
        'acp_external_agent_tool_input_schema',
        'validate_acp_external_agent_tool_input',
        'render_acp_external_agent_use_message',
        'render_acp_external_agent_rejected_message',
        'render_acp_external_agent_result_message',
        'render_acp_external_agent_result_for_assistant',
        'build_acp_external_agent_tool_result',
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
        'project_tool_context_facts',
        'build_tool_runtime_custom_data',
        'delegation_policy_from_custom_data',
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
        'should_post_process_research_report',
        'bitfun_services_integrations::deep_research::run_for_session_workspace',
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
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/task/input.rs',
      contracts: [
        'fork_context',
        'SubagentContextMode::Fork',
        'run_in_background',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/task/execution.rs',
      contracts: [
        'delegation_policy\\(\\)\\.spawn_child\\(\\)',
        'start_background_subagent',
        'background_task_id',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/task/background.rs',
      contracts: ['Background subagent started successfully'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/task/tests.rs',
      contracts: [
        'background_subagent_start_acknowledgement_uses_session_id_only',
        '<background_task',
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
      path: 'src/crates/execution/agent-runtime/src/deep_review/task_execution.rs',
      contracts: [
        'deep_review_task_completion_result',
        'deep_review_cancelled_reviewer_result',
        'should_emit_deep_review_retry_guidance',
        'deep_review_retry_guidance',
        'auto_retry_suppression_reason',
        'ensure_deep_review_auto_retry_allowed',
        'DeepReviewProviderCapacityQueueRuntime',
        'DeepReviewProviderCapacityRetryRuntime',
        'DeepReviewProviderCapacityRetryDecision',
        'DeepReviewReviewerAdmissionQueueRuntime',
        'QueueWaitTimer',
      ],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/deep_review/report.rs',
      contracts: ['fill_deep_review_runtime_tracker_signal'],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/deep_review/diagnostics.rs',
      contracts: ['deep_review_runtime_diagnostics_log_line'],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/deep_review/task_adapter.rs',
      contracts: [
        'runtime_task_execution::deep_review_task_completion_result',
        'runtime_task_execution::deep_review_cancelled_reviewer_result',
        'runtime_task_execution::should_emit_deep_review_retry_guidance',
        'runtime_task_execution::deep_review_retry_guidance',
        'runtime_task_execution::auto_retry_suppression_reason',
        'runtime_task_execution::ensure_deep_review_auto_retry_allowed',
        'runtime_task_execution::DeepReviewProviderCapacityQueueRuntime::start',
        'DeepReviewProviderCapacityRetryRuntime',
        'DeepReviewProviderCapacityRetryDecision',
        'runtime_task_execution::DeepReviewReviewerAdmissionQueueRuntime::start',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/task/deep_review.rs',
      contracts: [
        'deep_review_task_adapter::should_emit_deep_review_retry_guidance',
        'deep_review_task_adapter::auto_retry_suppression_reason',
        'deep_review_task_adapter::ensure_deep_review_auto_retry_allowed',
        'deep_review_task_adapter::deep_review_cancelled_reviewer_result',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/agentic/tools/implementations/task/execution.rs',
      contracts: [
        'deep_review_task_adapter::deep_review_retry_guidance',
        'deep_review_task_adapter::deep_review_task_completion_result',
        'DeepReviewProviderCapacityRetryRuntime::default',
        'DeepReviewProviderCapacityRetryDecision::WaitForCapacity',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/src/deep_research.rs',
      contracts: ['run_for_session_workspace', 'try_renumber_research_report', 'renumber_research_report', 'report.md', 'citations.md', 'display_map', 'REJECTED'],
    },
    {
      path: 'src/crates/execution/agent-runtime/src/deep_research.rs',
      contracts: ['renumber_research_report', 'ResearchCitationRenumberOutput', 'ResearchCitationDisplayMapEntry', 'rejected_index_rows_dropped', 'should_post_process_research_report'],
    },
    {
      path: 'src/crates/execution/agent-runtime/tests/deep_research_contracts.rs',
      contracts: ['deep_research_citation_renumber_owner_preserves_report_and_display_map_contracts', 'deep_research_citation_renumber_owner_is_idempotent_without_citations'],
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
        'not(feature = "remote-ssh-concrete")',
        'pub mod disabled',
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
      contracts: [
        'feature = "ssh-remote"',
        'bitfun_services_integrations::remote_ssh::workspace_search::disabled',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_ssh/workspace_search/disabled.rs',
      contracts: ['Remote SSH search is disabled', 'RemoteWorkspaceSearchService', 'remote_workspace_search_service_for_path'],
    },
    {
      path: 'src/crates/assembly/core/Cargo.toml',
      contracts: [
        'bitfun-product-capabilities = \\{ path = "\\.\\.\\/product-capabilities", default-features = false, optional = true \\}',
        'bitfun-ai-adapters = \\{ path = "\\.\\.\\/\\.\\.\\/adapters\\/ai-adapters", optional = true \\}',
        'bitfun-tool-packs = \\{ path = "\\.\\.\\/\\.\\.\\/execution\\/tool-provider-groups", default-features = false, optional = true \\}',
        'bitfun-services-integrations = \\{ path = "\\.\\.\\/\\.\\.\\/services\\/services-integrations", default-features = false, features = \\["remote-ssh"\\] \\}',
        'bitfun-product-domains = \\{ path = "\\.\\.\\/\\.\\.\\/contracts\\/product-domains", default-features = false, optional = true \\}',
        'dep:bitfun-ai-adapters',
        'ai-adapter-runtime',
        'canvas-runtime',
        'bitfun-services-integrations\\/canvas-runtime',
        'bitfun-services-integrations\\/function-agents',
        'bitfun-services-integrations\\/miniapp-runtime',
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
      path: 'src/crates/assembly/core/src/infrastructure/mod.rs',
      contracts: [
        'feature = "ai-adapter-runtime"',
        'pub mod ai',
        'pub mod cli_credentials',
        'feature = "product-full"',
        'pub mod debug_log',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/util/types/ai.rs',
      contracts: [
        'bitfun_core_types',
        'feature = "ai-adapter-runtime"',
        'GeminiResponse',
        'GeminiUsage',
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
        'pub mod search',
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
      contracts: [
        'feature = "product-full"',
        'WorkspaceBinding',
        'ensure_runtime_for_workspace_binding',
        'merge_legacy_session_store',
        'move_legacy_path',
        'session_store_migration_error',
      ],
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
      contracts: [
        'ServiceMiniAppStorage',
        'map_storage_error',
        'MiniAppImportBundleWriteRequest',
        'read_import_meta_json',
        'write_import_bundle',
        'MiniAppStoragePort',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/src/miniapp/storage.rs',
      contracts: [
        'pub struct MiniAppStorage',
        'MiniAppStorageError',
        'MiniAppImportBundleWriteRequest',
        'read_import_meta_json',
        'write_import_bundle',
        'tokio::fs::read_to_string',
        'tokio::fs::write',
        'tokio::fs::remove_dir_all',
        'MiniAppStorageLayout',
        'MiniAppStoragePort',
        'storage_port_adapter_preserves_existing_file_lifecycle',
        'import_bundle_io_preserves_copy_and_fallback_contract',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/miniapp/builtin/mod.rs',
      contracts: [
        'BUILTIN_APPS',
        'seed_builtin_miniapps_with_host',
        'BuiltinMiniAppSeedHost',
        'CoreBuiltinMiniAppSeedHost',
        'mark_builtin_update_available',
        'miniapp_builtin_io::prepare_builtin_seed_bundle_files',
        'read_builtin_install_marker',
        'miniapp_builtin_io::read_builtin_install_marker',
        'write_builtin_install_marker',
        'miniapp_builtin_io::write_builtin_install_marker',
        'recompile',
        'load_customization_metadata',
        'available_builtin_update',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/src/miniapp/builtin_io.rs',
      contracts: [
        'read_builtin_install_marker',
        'parse_builtin_install_marker',
        'write_builtin_install_marker',
        'serialize_builtin_install_marker',
        'prepare_builtin_seed_bundle_files',
        'builtin_source_files',
        'build_builtin_seed_meta',
        'preserved_builtin_created_at',
        'BUILTIN_PLACEHOLDER_COMPILED_HTML',
        'storage.json',
      ],
    },
    {
      path: 'src/crates/contracts/product-domains/src/miniapp/builtin.rs',
      contracts: [
        'BUILTIN_APPS',
        'BuiltinMiniAppBundle',
        'BuiltinInstallMarker',
        'BUILTIN_INSTALL_MARKER',
        'builtin_content_hash',
        'should_seed_builtin_app',
        'BuiltinSeedArtifacts',
        'BuiltinSeedCheck',
        'BuiltinSeedAction',
        'BuiltinMiniAppSeedHost',
        'seed_builtin_miniapps_with_host',
        'seed_builtin_miniapp_with_host',
        'BuiltinMiniAppSeedOutcome',
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
        'bitfun_services_integrations::miniapp::host_dispatch::dispatch_host',
        'map_host_dispatch_error',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/src/miniapp/host_dispatch.rs',
      contracts: [
        'dispatch_host',
        'split_host_method',
        'dispatch_fs',
        'plan_fs_legacy_path_check',
        'plan_fs_host_call',
        'fs_policy_scopes',
        'MiniAppPermissionPolicyRequest::from_paths',
        'resolve_policy_with_request',
        'fs_resolved_path_allowed',
        'dispatch_shell',
        'plan_shell_host_call',
        'shell_exec_default_env',
        'command_basename_allowed',
        'host_allowed_by_allowlist',
        'process_manager::create_tokio_command',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/miniapp/js_worker_pool.rs',
      contracts: [
        'MiniAppRuntimePort',
        'ServiceJsWorkerPool',
        'CoreMiniAppWorkerEventSink',
        'emit_global_event',
        'map_worker_pool_error',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/miniapp/js_worker.rs',
      contracts: [
        'pub use bitfun_services_integrations::miniapp::worker::{',
        'MiniAppWorkerEventSink',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/src/miniapp/worker.rs',
      contracts: [
        'pub struct JsWorker',
        'pub trait MiniAppWorkerEventSink',
        'process_manager::create_tokio_command',
        'PendingResponseMap',
        'uuid::Uuid::new_v4',
      ],
    },
    {
      path: 'src/crates/services/services-integrations/src/miniapp/worker_pool.rs',
      contracts: [
        'pub struct JsWorkerPool',
        'MiniAppWorkerPoolError',
        'worker_pool_at_capacity',
        'select_lru_worker',
        'plan_install_deps',
        'process_manager::create_tokio_command',
        'MiniAppRuntimePort',
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
        'agent_runtime',
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
      contracts: ['bitfun_services_integrations::remote_ssh', 'pub mod manager', 'pub mod remote_fs', 'pub mod remote_terminal', 'pub mod workspace_state'],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_ssh/mod.rs',
      contracts: ['mod disabled', 'pub use disabled', 'remote-ssh-concrete', 'pub mod manager', 'mod remote_exec', 'pub mod remote_fs', 'pub mod remote_terminal'],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_ssh/disabled.rs',
      contracts: ['Remote SSH support is disabled', 'SSHConnectionManager', 'RemoteFileService', 'RemoteTerminalManager'],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_ssh/manager.rs',
      contracts: ['SSHConnectionManager', 'russh::client::connect_stream', 'SftpSession', 'prunes_password_connection_without_vault_entry'],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_ssh/remote_exec.rs',
      contracts: ['RemoteExecProcessManager', 'GLOBAL_REMOTE_EXEC_MANAGER', 'remote_exec_session_ids_match_local_test_baseline'],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_ssh/remote_fs.rs',
      contracts: ['RemoteFileService', 'sftp_read', 'sftp_write'],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_ssh/remote_terminal.rs',
      contracts: ['RemoteTerminalManager', 'PtyCommand', 'channel.window_change'],
    },
    {
      path: 'src/crates/services/services-integrations/src/remote_ssh/paths.rs',
      contracts: [
        'WorkspaceSessionIdentity',
        'workspace_session_identity',
        'remote_workspace_runtime_root',
        'remote_workspace_session_mirror_dir',
        'canonicalize_local_workspace_root',
        'normalize_local_workspace_root_for_stable_id',
        'local_workspace_roots_equal',
        'unresolved_remote_session_storage_dir',
      ],
    },
    {
      path: 'src/crates/contracts/product-domains/src/miniapp/runtime_facade.rs',
      contracts: [
        'MiniAppRuntimeFacade',
        'create_app',
        'persist_update_result_for_app',
        'persist_draft_for_app',
        'persist_draft_source_sync_result',
        'persist_draft_permission_update_result',
        'apply_draft_app',
        'mark_builtin_update_available',
        'mark_deps_installed_state',
        'persist_sync_from_fs_result_for_app',
        'persist_import_runtime_state',
        'pub async fn import_from_path',
        'MiniAppImportPort',
        'MiniAppCompilePort',
        'MiniAppImportBundleWriteRequest',
        'build_import_bundle_plan',
      ],
    },
    {
      path: 'src/crates/contracts/product-domains/src/miniapp/compiler.rs',
      contracts: [
        'MiniAppCompileRequest',
        'from_paths',
        'compile_with_request',
        'compile_request_from_paths_preserves_runtime_paths',
        'compile_with_request_preserves_legacy_compile_output',
      ],
    },
    {
      path: 'src/crates/contracts/product-domains/src/miniapp/permission_policy.rs',
      contracts: [
        'MiniAppPermissionPolicyRequest',
        'from_paths',
        'resolve_policy_with_request',
        'permission_policy_request_preserves_path_scope_and_granted_paths',
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
        'MiniAppImportBundlePlan',
        'MiniAppImportBundlePlanError',
        'MiniAppImportBundleWriteRequest',
        'build_import_bundle_plan',
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
        'CoreProductDomainRuntime',
        'MiniAppRuntimeFacade',
        'create_app',
        'persist_update_result_for_app',
        'persist_draft_for_app',
        'persist_draft_source_sync_result',
        'persist_draft_permission_update_result',
        'apply_draft_app',
        'mark_builtin_update_available',
        'decline_builtin_update',
        'persist_sync_from_fs_result_for_app',
        'compile_source',
        'MiniAppCompileRequest::from_paths',
        'compile_with_request',
        'MiniAppPermissionPolicyRequest::from_paths',
        'resolve_policy_with_request',
        'MiniAppCompilePort',
        'MiniAppImportFromPathRequest',
        'import_from_path',
        'runtime_preflight_preserves_recompile_sync_rollback_and_deps_state',
        'import_from_path_preserves_fallback_files_recompile_and_runtime_state',
      ],
    },
    {
      path: 'src/crates/assembly/core/src/function_agents/port_adapters.rs',
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

  const sessionControlRuleText = forbiddenRuleTextForPath(
    'src/crates/assembly/core/src/agentic/tools/implementations/session_control_tool.rs',
  );
  if (!sessionControlRuleText.includes('create_session_with_workspace_and_creator')) {
    throw new Error('SessionControl old create-path boundary rule must cover legacy create path');
  }

  const sdkSmokeRuleText = forbiddenRuleTextForPath(
    'src/crates/execution/agent-runtime/tests/sdk_smoke.rs',
  );
  for (const forbiddenSdkSmokeImport of [
    'bitfun_runtime_services::test_support',
    'FakeRuntimeServicesProvider',
  ]) {
    if (!sdkSmokeRuleText.includes(forbiddenSdkSmokeImport)) {
      throw new Error(
        `SDK smoke boundary rule must forbid ${forbiddenSdkSmokeImport}`,
      );
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
    'DynamicMcpToolInfo',
    'Input must be an object',
    'Using MCP tool',
    'was rejected by user',
    'completed\\. Result:',
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
  for (const dep of [
    'aes',
    'dunce',
    'futures',
    'hex',
    'local-ip-address',
    'md5',
    'reqwest',
    'sse-stream',
    'which',
  ]) {
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
