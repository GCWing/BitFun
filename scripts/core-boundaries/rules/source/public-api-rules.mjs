// Public API allowlists for contract modules where accidental surface growth is costly.

export const publicApiContractSlices = [
  'frontend-backend-capability-service',
  'bitfun-plugin-extension-contract',
  'plugin-runtime-internal-abi',
  'opencode-adapter-boundary',
  'external-source-command-contract',
  'external-source-tool-contract',
  'external-source-subagent-contract',
  'external-source-mcp-contract',
  'external-integration-policy-contract',
];

const contractSlices = {
  frontendBackendCapabilityService: 'frontend-backend-capability-service',
  bitfunPluginExtension: 'bitfun-plugin-extension-contract',
  pluginRuntimeInternalAbi: 'plugin-runtime-internal-abi',
  opencodeAdapterBoundary: 'opencode-adapter-boundary',
  externalSourceCommandContract: 'external-source-command-contract',
  externalSourceToolContract: 'external-source-tool-contract',
  externalSourceSubagentContract: 'external-source-subagent-contract',
  externalSourceMcpContract: 'external-source-mcp-contract',
  externalIntegrationPolicyContract: 'external-integration-policy-contract',
};

function pluginRuntimeEntry(symbol, p0, consumer, verification, contractSlice, wireImpact = true) {
  return {
    symbol,
    owner: 'runtime-ports plugin contract owner',
    consumer,
    verification,
    p0,
    contractSlice,
    wireImpact,
    rationale: `${p0} needs a stable contract symbol instead of raw JSON or product-full leakage`,
    exit: 'remove only after a reviewed compatibility migration and root re-export budget update',
  };
}

export const pluginRuntimePublicApiEntries = [
  ...[
    'PluginSourceKind',
    'PluginSourceRef',
    'PluginManifestRef',
    'PluginTrustLevel',
    'PluginStatusKind',
    'PluginStatusSnapshot',
    'PluginConfigValidationIssue',
    'PluginConfigValidationState',
    'PluginConfigValidationStatus',
  ].map((symbol) =>
    pluginRuntimeEntry(
      symbol,
      'plugin discovery, status, and config-validation projection',
      'Plugin Runtime Host read model and product assembly plugin status projection',
      'runtime-ports read-model contract tests, OpenCode fixture projection tests, and plugin-runtime-host read-model tests',
      contractSlices.bitfunPluginExtension,
    ),
  ),
  ...[
    'PluginCapabilityRef',
    'PluginTargetRef',
    'PluginArtifactRef',
    'PluginAuditRef',
    'PluginOwnerKind',
    'PluginOwnerRef',
    'PluginDataClassification',
    'PluginPayloadRedaction',
    'PluginPayloadRef',
    'PluginRiskLevel',
    'PermissionPromptEffectKind',
    'PermissionPromptDenyState',
    'PermissionPromptDescriptor',
    'PluginRollbackMode',
    'PluginRollbackPolicy',
    'PluginPermissionGate',
    'PluginEffectCandidate',
    'PluginEffectCandidatePayload',
  ].map((symbol) =>
    pluginRuntimeEntry(
      symbol,
      'plugin permission, effect-preview, and provider handoff',
      'Plugin Runtime Host, tool ABI integration, and security-control candidate validation',
      'runtime-ports candidate-effect contract tests and plugin-runtime-host permission/effect validation tests',
      contractSlices.bitfunPluginExtension,
    ),
  ),
  ...[
    'PluginDiagnostic',
    'PluginDiagnosticDetail',
    'PluginDiagnosticSeverity',
    'PluginQuarantineScope',
    'PluginQuarantineReason',
    'PluginQuarantineClearCondition',
    'PluginQuarantineState',
  ].map((symbol) =>
    pluginRuntimeEntry(
      symbol,
      'plugin diagnostics and quarantine read-model projection',
      'Plugin Runtime Host read model and capability-service diagnostics projection',
      'runtime-ports diagnostics tests and plugin-runtime-host quarantine/read-model owner tests',
      contractSlices.bitfunPluginExtension,
    ),
  ),
  ...[
    'ExtensionCapabilityAvailability',
    'PluginRuntimeAvailability',
    'PluginRuntimeUnavailableReason',
    'PluginRuntimeEpochs',
    'PluginRuntimeReadRequest',
    'PluginRuntimeReadResponse',
    'PluginDispatchEnvelope',
    'PluginResponseEnvelope',
    'PluginHostLifecyclePhase',
    'PluginRuntimeClient',
    'DisabledPluginRuntimeClient',
    'ProjectionOnlyPluginRuntimeClient',
    'PluginRuntimeBinding',
    'validate_plugin_runtime_read_response',
    'validate_plugin_dispatch_response',
  ].map((symbol) =>
    pluginRuntimeEntry(
      symbol,
      'plugin host boundary, lifecycle, and execution availability',
      'Product assembly host handoff and Agent Runtime plugin binding',
      'runtime-ports contract tests and plugin-runtime-host owner validation',
      contractSlices.pluginRuntimeInternalAbi,
    ),
  ),
];

export const pluginRuntimePublicApiSymbols = pluginRuntimePublicApiEntries.map(
  (entry) => entry.symbol,
);

function pluginRuntimeHostEntry(symbol, consumer) {
  return {
    symbol,
    owner: 'plugin-runtime-host owner',
    consumer,
    verification: 'plugin-runtime-host owner tests and product assembly host binding checks',
    p0: 'Plugin Runtime Host executable boundary for the OpenCode-compatible P0 vertical slice',
    contractSlice: contractSlices.pluginRuntimeInternalAbi,
    wireImpact: false,
    rationale:
      'P0 host execution needs a narrow injected adapter boundary without exposing concrete plugin runtimes',
    exit: 'remove only if Host ownership moves to a reviewed replacement crate with equivalent boundary tests',
  };
}

export const pluginRuntimeHostPublicApiEntries = [
  pluginRuntimeHostEntry(
    'PluginHostAdapter',
    'PluginRuntimeHost::new injected adapter boundary and plugin-runtime-host owner tests',
  ),
  pluginRuntimeHostEntry(
    'PluginRuntimeHost',
    'Product Assembly host binding, AgentRuntimeBuilder runtime handoff, and plugin-runtime-host contract tests',
  ),
];

function opencodeAdapterEntry(symbol, consumer) {
  return {
    symbol,
    owner: 'opencode-adapter owner',
    consumer,
    verification:
      'opencode-adapter source adapter tests, PluginRuntimeHost integration path, and core-boundary public API budget checks',
    p0: 'OpenCode-compatible P0-C.1 source discovery/read model and P0-C.2 custom tool candidate mapping',
    contractSlice: contractSlices.opencodeAdapterBoundary,
    wireImpact: false,
    rationale:
      'P0-C needs one adapter factory that consumes fixed BitFun-managed package content and returns the existing PluginHostAdapter boundary',
    exit:
      'remove only if source discovery moves behind a reviewed product source registry with equivalent host tests',
  };
}

export const opencodeAdapterPublicApiEntries = [
  opencodeAdapterEntry(
    'load_opencode_package_adapter',
    'bitfun-core managed plugin composition root and PluginRuntimeHost integration tests',
  ),
  opencodeAdapterEntry(
    'OpenCodeCommandProvider',
    'bitfun-core external source composition root and OpenCode command adapter tests',
  ),
  opencodeAdapterEntry(
    'OpenCodeCommandProviderOptions',
    'OpenCode command adapter fixture tests and explicit environment injection',
  ),
  opencodeAdapterEntry(
    'OpenCodeToolProvider',
    'bitfun-core external source composition root and OpenCode standalone-tool adapter tests',
  ),
  opencodeAdapterEntry(
    'OpenCodeToolProviderOptions',
    'OpenCode standalone-tool adapter fixture tests and explicit environment injection',
  ),
  opencodeAdapterEntry(
    'OpenCodeSubagentProvider',
    'bitfun-core external source composition root and OpenCode subagent adapter tests',
  ),
  opencodeAdapterEntry(
    'OpenCodeSubagentProviderOptions',
    'OpenCode subagent adapter fixture tests and explicit environment injection',
  ),
  opencodeAdapterEntry(
    'OpenCodeMcpProvider',
    'bitfun-core external source composition root and OpenCode MCP adapter tests',
  ),
  opencodeAdapterEntry(
    'OpenCodeMcpProviderOptions',
    'OpenCode MCP adapter fixture tests and explicit environment injection',
  ),
];

function externalSourceEntry(symbol, owner, consumer, wireImpact = false) {
  return {
    symbol,
    owner,
    consumer,
    verification:
      'external source contract tests, fake-provider coordinator tests, OpenCode command fixtures, and CLI/Desktop product tests',
    p0: 'PR1 ecosystem-neutral source catalog and OpenCode prompt-command vertical slice',
    contractSlice: contractSlices.externalSourceCommandContract,
    wireImpact,
    rationale:
      'PR1 needs typed capability contracts and provider-neutral lifecycle coordination without ecosystem payload leakage',
    exit: 'remove only through a reviewed capability-contract migration with equivalent isolation and product tests',
  };
}

function externalIntegrationPolicyEntry(
  symbol,
  owner = 'product-domains external integration policy contract owner',
  consumer = 'bitfun-core product composition and cross-host product surfaces',
  wireImpact = true,
) {
  return {
    symbol,
    owner,
    consumer,
    verification:
      'external integration policy contract tests, core policy lifecycle tests, cross-host route tests, and Web policy-control tests',
    p0: 'host-owned external integration policy and OpenCode-compatible product defaults',
    contractSlice: contractSlices.externalIntegrationPolicyContract,
    wireImpact,
    rationale:
      'all product surfaces need one ecosystem-neutral, versioned, fail-closed policy contract while concrete ecosystem defaults remain in product assembly',
    exit:
      'remove only through a reviewed policy-contract migration with equivalent compatibility, safety-ceiling, and cross-host behavior tests',
  };
}

export const externalIntegrationPolicyPublicApiEntries = [
  'EXTERNAL_INTEGRATION_POLICY_SCHEMA_MAJOR',
  'ExternalIntegrationMode',
  'ExternalIntegrationAccess',
  'ExternalEcosystemPolicy',
  'ExternalIntegrationPolicySettings',
  'ExternalIntegrationPolicySettingsView',
  'ExternalEcosystemPolicyOverride',
  'ExternalEcosystemPolicyOverrideView',
  'ExternalIntegrationPolicyOverride',
  'ExternalIntegrationPolicyOverrideView',
  'ExternalIntegrationPolicyDocument',
  'ExternalIntegrationCapabilityDescriptor',
  'ExternalIntegrationEcosystemDescriptor',
  'ExternalEcosystemPolicyView',
  'EffectiveExternalEcosystemPolicy',
  'EffectiveExternalIntegrationPolicy',
  'ExternalIntegrationPolicyStatus',
  'ExternalIntegrationPolicySnapshot',
  'ExternalIntegrationPolicyScope',
  'ExternalIntegrationPolicyOperation',
  'ExternalIntegrationPolicyMutation',
  'evaluate_external_integration_policy',
  'external_integration_policy_snapshot',
  'incompatible_external_integration_policy_snapshot',
].map((symbol) => externalIntegrationPolicyEntry(symbol));

function externalToolEntry(symbol, owner, consumer, wireImpact = false) {
  return {
    symbol,
    owner,
    consumer,
    verification:
      'external tool contract, coordinator, OpenCode adapter, worker runtime, core routing, CLI, and Desktop tests',
    p0: 'PR2 ecosystem-neutral standalone-tool activation and OpenCode JavaScript vertical slice',
    contractSlice: contractSlices.externalSourceToolContract,
    wireImpact,
    rationale:
      'PR2 needs typed preview, approval, conflict, activation, and preparation contracts without ecosystem payload leakage',
    exit: 'remove only through a reviewed tool-capability contract migration with equivalent isolation and product tests',
  };
}

function externalSubagentEntry(symbol, owner, consumer, wireImpact = false) {
  return {
    symbol,
    owner,
    consumer,
    verification:
      'external subagent contract, coordinator, OpenCode adapter, product reconciliation, registry lease, TUI, Desktop, and Web tests',
    p0: 'PR3 ecosystem-neutral fresh subagent activation and OpenCode agent vertical slice',
    contractSlice: contractSlices.externalSourceSubagentContract,
    wireImpact,
    rationale:
      'PR3 needs typed discovery, approval-envelope, conflict, summary, and fresh-invocation contracts without ecosystem payload leakage',
    exit:
      'remove only through a reviewed subagent-capability contract migration with equivalent fail-closed routing and product tests',
  };
}

function externalMcpEntry(symbol, owner, consumer, wireImpact = false) {
  return {
    symbol,
    owner,
    consumer,
    verification:
      'external MCP contract, coordinator, OpenCode adapter, MCP owner lifecycle, TUI, Desktop, and Web tests',
    p0: 'PR6 ecosystem-neutral MCP source activation and OpenCode MCP configuration vertical slice',
    contractSlice: contractSlices.externalSourceMcpContract,
    wireImpact,
    rationale:
      'PR6 needs typed static discovery, versioned approval, conflict, preparation, and runtime status contracts without OpenCode or MCP-owner payload leakage',
    exit:
      'remove only through a reviewed MCP source contract migration with equivalent fail-closed activation and lifecycle tests',
  };
}

export const externalSourceContractPublicApiEntries = [
  'ExternalSourceContractError',
  'SourceKey',
  'SourceQualifiedCommandId',
  'ExternalSourceScope',
  'ExternalSourceHealth',
  'ExternalSourceAssetKind',
  'ExternalSourceDiagnosticSeverity',
  'ExternalSourceDiagnostic',
  'ExternalSourceRecord',
  'PromptCommandAvailability',
  'PromptCommandDefinition',
  'ExpandedPromptCommand',
  'PromptCommandProviderIdentity',
  'PromptCommandProviderSnapshot',
  'ExternalSourceContext',
  'ExternalWatchRoot',
  'ExternalSourceProviderError',
  'ExternalSourceOperationErrorCode',
  'ExternalSourceOperationError',
  'ExternalSourceOperationResult',
  'PromptCommandSourceProvider',
  'ExternalSourceLifecycleState',
  'ExternalSourceCatalogEntry',
  'PromptCommandCatalogEntry',
  'PromptCommandConflictCandidate',
  'PromptCommandConflict',
  'prompt_command_conflict_key',
  'ExternalSourceCatalogSnapshot',
  'ExternalPromptCommandDefinitionSummary',
  'ExternalPromptCommandSummary',
  'ExternalSourcePublicSnapshot',
  'ExternalSourceHostCapabilities',
].map((symbol) =>
  externalSourceEntry(
    symbol,
    'product-domains external source contract owner',
    'ecosystem command providers, external-source coordinator, product composition, and neutral product surfaces',
    true,
  ),
).concat(
  [
    'SourceQualifiedToolTargetId',
    'SourceQualifiedToolId',
    'ExternalToolRuntimeKind',
    'ExternalToolCapability',
    'ExternalToolStaticStatus',
    'ExternalToolDefinition',
    'external_tool_approval_key',
    'external_tool_conflict_key',
    'external_tool_decision_key',
    'ExternalToolProviderIdentity',
    'ExternalToolProviderSnapshot',
    'PreparedExternalToolExport',
    'PreparedExternalToolTarget',
    'ExternalToolSourceProvider',
    'ExternalToolActivationState',
    'ExternalToolCatalogEntry',
    'ExternalToolApprovalRequest',
    'ExternalToolConflictCandidateKind',
    'ExternalToolConflictCandidate',
    'ExternalToolConflict',
  ].map((symbol) =>
    externalToolEntry(
      symbol,
      'product-domains external tool contract owner',
      'ecosystem tool providers, external-tool coordinator, product composition, and neutral product surfaces',
      true,
    ),
  ),
  [
    'SourceQualifiedMcpServerId',
    'ExternalMcpTransportKind',
    'ExternalMcpStaticStatus',
    'ExternalMcpServerDefinition',
    'ExternalMcpActivationState',
    'ExternalMcpCatalogEntry',
    'ExternalMcpApprovalRequest',
    'ExternalMcpConflictCandidate',
    'ExternalMcpConflict',
    'SecretValue',
    'PreparedExternalMcpTransport',
    'PreparedExternalMcpServer',
    'ExternalMcpProviderIdentity',
    'ExternalMcpProviderSnapshot',
    'ExternalMcpSourceProvider',
    'external_mcp_approval_key',
    'external_mcp_conflict_key',
    'ExternalMcpDiscoveryInput',
  ].map((symbol) =>
    externalMcpEntry(
      symbol,
      'product-domains external MCP contract owner',
      'ecosystem MCP providers, external-MCP coordinator, product reconciliation, and MCP runtime owner',
      true,
    ),
  ),
);

export const externalSubagentContractPublicApiEntries = [
  'ExternalSubagentLocalId',
  'ExternalSubagentCandidateId',
  'ExternalSubagentBehaviorVersion',
  'SecretText',
  'ExternalSubagentContributionId',
  'ExternalSubagentContributionRole',
  'ExternalSubagentProvenanceRef',
  'ExternalSubagentProviderIdentity',
  'ExternalSubagentMode',
  'ExternalSubagentModelRequest',
  'ExternalSubagentToolSelector',
  'ExternalSubagentToolRequest',
  'ExternalSubagentCompatibilityState',
  'ExternalSubagentDefinition',
  'ExternalSubagentDiscoveryInput',
  'ExternalSubagentProviderSnapshot',
  'ExternalSubagentSourceProvider',
  'ExternalSubagentActivationState',
  'ExternalSubagentDiagnosticSummary',
  'ExternalSubagentSummary',
  'ExternalSubagentConflictCandidate',
  'ExternalSubagentConflict',
  'external_subagent_candidate_id',
  'external_subagent_approval_key',
  'external_subagent_conflict_key',
].map((symbol) =>
  externalSubagentEntry(
    symbol,
    'product-domains external subagent contract owner',
    'ecosystem subagent providers, external-subagent coordinator, product reconciliation, and neutral product surfaces',
    true,
  ),
);

export const externalSourceCoordinatorPublicApiEntries = [
  externalSourceEntry(
    'ExternalSourceCoordinator',
    'external-sources assembly owner',
    'bitfun-core product composition root',
  ),
  ...['ExternalSourceDiscoveryRequest', 'ExternalSourceDiscoveryResult'].map((symbol) =>
    externalSourceEntry(
      symbol,
      'external-sources assembly owner',
      'bitfun-core bounded concurrent provider scheduler',
    ),
  ),
  ...[
    'ExternalToolCoordinator',
    'ExternalToolCoordinatorSnapshot',
    'ExternalToolDiscoveryRequest',
    'ExternalToolDiscoveryResult',
  ].map((symbol) =>
    externalToolEntry(
      symbol,
      'external-sources assembly owner',
      'bitfun-core bounded concurrent external-tool provider scheduler',
    ),
  ),
  ...[
    'ExternalSubagentCoordinator',
    'ExternalSubagentCoordinatorSnapshot',
    'ExternalSubagentDiscoveryRequest',
    'ExternalSubagentDiscoveryResult',
  ].map((symbol) =>
    externalSubagentEntry(
      symbol,
      'external-sources assembly owner',
      'bitfun-core bounded concurrent external-subagent provider scheduler',
    ),
  ),
  ...[
    'ExternalMcpCoordinator',
    'ExternalMcpCoordinatorSnapshot',
    'ExternalMcpDiscoveryRequest',
    'ExternalMcpDiscoveryResult',
  ].map((symbol) =>
    externalMcpEntry(
      symbol,
      'external-sources assembly owner',
      'bitfun-core bounded concurrent external-MCP provider scheduler',
    ),
  ),
];

export const externalSourceCorePublicApiEntries = [
  ...[
    'ExternalIntegrationAccess',
    'ExternalIntegrationMode',
    'ExternalIntegrationPolicyMutation',
    'ExternalIntegrationPolicyOperation',
    'ExternalIntegrationPolicyScope',
    'EffectiveExternalIntegrationPolicy',
    'ExternalIntegrationPolicySnapshot',
    'ExternalIntegrationPolicyStatus',
    'EcosystemId',
    'ExternalIntegrationCapabilityId',
    'EXTERNAL_CAPABILITY_COMMAND',
    'EXTERNAL_CAPABILITY_TOOL',
    'EXTERNAL_CAPABILITY_SUBAGENT',
    'EXTERNAL_CAPABILITY_MCP',
    'update_external_integration_policy',
  ].map((symbol) =>
    externalIntegrationPolicyEntry(
      symbol,
      'bitfun-core external integration policy composition facade',
      'BitFun CLI, Desktop, Server, Peer Host, and Web API adapters',
      true,
    ),
  ),
  ...[
    'ExpandedPromptCommand',
    'ExternalSourceCatalogEntry',
    'ExternalSourceCatalogSnapshot',
    'ExternalSourceAssetKind',
    'ExternalSourceDiagnostic',
    'ExternalSourceDiagnosticSeverity',
    'ExternalSourceLifecycleState',
    'ExternalSourceHostCapabilities',
    'ExternalSourceOperationError',
    'ExternalSourceOperationErrorCode',
    'ExternalSourceOperationResult',
    'PromptCommandAvailability',
    'PromptCommandCatalogEntry',
    'PromptCommandDefinition',
    'SourceKey',
    'prompt_command_conflict_key',
    'external_source_conflict_choices',
    'remember_external_source_conflict_choice',
    'set_external_prompt_command_conflict_choice',
    'external_source_snapshot',
    'external_source_read_only_snapshot',
    'set_external_source_enabled',
    'expand_external_prompt_command',
    'sanitize_external_source_operation_error',
    'subscribe_external_source_updates',
    'ExternalSourceSubscription',
    'ExternalSourcePublicSnapshot',
  ].map((symbol) =>
    externalSourceEntry(
      symbol,
      'bitfun-core external source composition facade',
      'BitFun CLI and desktop host APIs',
    ),
  ),
  externalSourceEntry(
    'external_source_location_for_host_action',
    'bitfun-core external source composition owner',
    'Desktop external-source configuration host adapter',
    true,
  ),
  ...[
    'ExternalToolActivationState',
    'ExternalToolApprovalRequest',
    'ExternalToolCapability',
    'ExternalToolCatalogEntry',
    'ExternalToolConflict',
    'ExternalToolConflictCandidateKind',
    'ExternalToolRuntimeKind',
    'set_external_tool_target_decision',
    'set_external_tool_conflict_choice',
  ].map((symbol) =>
    externalToolEntry(
      symbol,
      'bitfun-core external tool composition facade',
      'BitFun CLI and desktop host APIs',
    ),
  ),
  ...[
    'ExternalSubagentActivationState',
    'ExternalSubagentCompatibilityState',
    'ExternalSubagentConflict',
    'ExternalSubagentConflictCandidate',
    'ExternalSubagentSummary',
    'set_external_subagent_activation',
    'choose_external_subagent_conflict',
  ].map((symbol) =>
    externalSubagentEntry(
      symbol,
      'bitfun-core external subagent composition facade',
      'BitFun CLI and desktop host APIs',
    ),
  ),
  ...[
    'ExternalMcpActivationState',
    'ExternalMcpApprovalRequest',
    'ExternalMcpCatalogEntry',
    'ExternalMcpConflict',
    'ExternalMcpTransportKind',
    'native_mcp_candidate_id',
    'set_external_mcp_server_decision',
    'choose_external_mcp_conflict',
  ].map((symbol) =>
    externalMcpEntry(
      symbol,
      'bitfun-core external MCP composition facade',
      'BitFun CLI and desktop host APIs',
    ),
  ),
];

function pluginSourceEntry(symbol, owner, consumer, verification, wireImpact) {
  return {
    symbol,
    owner,
    consumer,
    verification,
    p0: 'P0-C managed package discovery, workspace review state, fixed adapter input, and CLI diagnostics',
    contractSlice: contractSlices.bitfunPluginExtension,
    wireImpact,
    rationale:
      'P0-C needs one ecosystem-neutral package identity, review, and fixed-content boundary without exposing adapter or Host ABI types',
    exit:
      'remove only after a reviewed package-source owner migration with equivalent CLI and trust-state tests',
  };
}

export const pluginSourceContractPublicApiEntries = [
  'PluginPackageFile',
  'PluginPackageManifest',
  'PluginPackageSourceIdentity',
  'PluginPackageInput',
  'PluginPackageTrustLevel',
  'PluginTrustDecision',
  'PluginTrustStore',
  'PluginSourceContractError',
  'PluginActivationAuthority',
].map((symbol) =>
  pluginSourceEntry(
    symbol,
    'product-domains plugin-source contract owner',
    'services-integrations managed package source owner, bitfun-core compatibility facade, and plugin-source contract tests',
    'product-domains plugin_source_contracts tests and services-integrations managed package discovery tests',
    true,
  ),
);

export const managedPluginSourcePublicApiEntries = [
  'ManagedPluginTrustLevel',
  'ManagedPluginTrustDecision',
  'ManagedPluginPackageView',
  'ManagedPluginSourceIssue',
  'ManagedPluginSourceSnapshot',
  'ManagedPluginSourceError',
  'refresh_managed_plugin_sources',
  'set_managed_plugin_trust',
].map((symbol) =>
  pluginSourceEntry(
    symbol,
    'bitfun-core managed plugin source compatibility facade',
    'BitFun CLI plugins and doctor commands',
    'services-integrations plugin_source tests, core boundary checks, and BitFun CLI plugin command tests',
    false,
  ),
);

export const managedPluginActivationPublicApiEntries = [
  'ManagedPluginCandidateView',
  'ManagedPluginActivationView',
  'ManagedPluginDeactivationResult',
  'preview_managed_plugin_activation',
  'activate_managed_plugin',
  'deactivate_managed_plugin',
].map((symbol) =>
  pluginSourceEntry(
    symbol,
    'bitfun-core managed plugin composition root',
    'BitFun CLI plugin activation commands',
    'bitfun-core plugin_runtime tests, BitFun CLI plugin source tests, and core boundary checks',
    false,
  ),
);

export const managedPluginSourceServicePublicApiEntries = [
  'ManagedPluginTrustLevel',
  'ManagedPluginTrustDecision',
  'ManagedPluginPackageView',
  'ManagedPluginSourceIssue',
  'ManagedPluginSourceSnapshot',
  'ManagedPluginSourceError',
  'ManagedPluginSourceService',
].map((symbol) =>
  pluginSourceEntry(
    symbol,
    'services-integrations managed plugin source owner',
    'bitfun-core managed plugin source compatibility facade',
    'services-integrations plugin_source tests and core boundary checks',
    false,
  ),
);

export const publicApiAllowlistRules = [
  {
    path: 'src/crates/contracts/runtime-ports/src/plugin.rs',
    reason:
      'plugin runtime public contract symbols must stay explicitly budgeted and consumer-backed',
    allowedSymbolEntries: pluginRuntimePublicApiEntries,
  },
  {
    path: 'src/crates/contracts/runtime-ports/src/lib.rs',
    reason:
      'runtime-ports root must re-export only the explicitly budgeted plugin runtime contract surface',
    allowedPluginReexportEntries: pluginRuntimePublicApiEntries,
  },
  {
    path: 'src/crates/adapters/opencode-adapter/src/lib.rs',
    reason:
      'OpenCode adapter public API must stay limited to source and candidate mapping through the Plugin Runtime Host adapter boundary',
    allowedSymbolEntries: opencodeAdapterPublicApiEntries,
  },
  {
    path: 'src/crates/execution/plugin-runtime-host/src/lib.rs',
    reason:
      'Plugin Runtime Host public API must stay limited to the injected adapter trait and host boundary type',
    allowedSymbolEntries: pluginRuntimeHostPublicApiEntries,
  },
  {
    path: 'src/crates/contracts/product-domains/src/plugin_source.rs',
    reason:
      'managed plugin package and trust contracts must stay explicitly budgeted and ecosystem-neutral',
    allowedSymbolEntries: pluginSourceContractPublicApiEntries,
  },
  {
    path: 'src/crates/contracts/product-domains/src/external_integration_policy.rs',
    reason:
      'external integration policy contracts must stay ecosystem-neutral, versioned, fail-closed, and explicitly consumer-backed',
    allowedSymbolEntries: externalIntegrationPolicyPublicApiEntries,
  },
  {
    path: 'src/crates/contracts/product-domains/src/external_sources.rs',
    reason:
      'external source contracts must stay capability-specific, ecosystem-neutral, and explicitly consumer-backed',
    allowedSymbolEntries: externalSourceContractPublicApiEntries,
  },
  {
    path: 'src/crates/contracts/product-domains/src/external_subagents.rs',
    reason:
      'external subagent contracts must stay ecosystem-neutral, fresh-only, and explicitly consumer-backed',
    allowedSymbolEntries: externalSubagentContractPublicApiEntries,
  },
  {
    path: 'src/crates/assembly/external-sources/src/lib.rs',
    reason:
      'external source assembly API must expose only the provider-neutral coordinator',
    allowedSymbolEntries: externalSourceCoordinatorPublicApiEntries,
  },
  {
    path: 'src/crates/assembly/core/src/external_sources.rs',
    reason:
      'core external source facade must stay limited to neutral product operations and read models',
    allowedSymbolEntries: externalSourceCorePublicApiEntries,
  },
  {
    path: 'src/crates/services/services-integrations/src/plugin_source.rs',
    reason:
      'managed plugin source service API must stay limited to one injected service and its result types',
    allowedSymbolEntries: managedPluginSourceServicePublicApiEntries,
  },
  {
    path: 'src/crates/assembly/core/src/plugin_source.rs',
    reason:
      'core managed plugin source compatibility API must stay limited to the current CLI consumer surface',
    allowedSymbolEntries: managedPluginSourcePublicApiEntries,
  },
  {
    path: 'src/crates/assembly/core/src/plugin_runtime.rs',
    reason:
      'core managed plugin activation API must stay limited to product status projection and explicit activation or deactivation transitions',
    allowedSymbolEntries: managedPluginActivationPublicApiEntries,
  },
];
