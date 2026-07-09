// Public API allowlists for contract modules where accidental surface growth is costly.

export const publicApiContractSlices = [
  'frontend-backend-capability-service',
  'bitfun-plugin-extension-contract',
  'plugin-runtime-internal-abi',
  'opencode-adapter-boundary',
];

const contractSlices = {
  frontendBackendCapabilityService: 'frontend-backend-capability-service',
  bitfunPluginExtension: 'bitfun-plugin-extension-contract',
  pluginRuntimeInternalAbi: 'plugin-runtime-internal-abi',
  opencodeAdapterBoundary: 'opencode-adapter-boundary',
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
      'P0-C needs one adapter factory for host-readable OpenCode-compatible sources; trust input reuses existing PluginSourceRef snapshots plus trust epoch instead of adding an ecosystem DTO',
    exit:
      'remove only if source discovery moves behind a reviewed product source registry with equivalent host tests',
  };
}

export const opencodeAdapterPublicApiEntries = [
  opencodeAdapterEntry(
    'load_opencode_workspace_adapter',
    'PluginRuntimeHost::new integration tests with PluginSourceRef trust snapshots; production Product Assembly binding is out of scope for this PR',
  ),
];

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
];
