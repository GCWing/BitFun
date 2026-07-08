// Public API allowlists for contract modules where accidental surface growth is costly.

function pluginRuntimeEntry(symbol, p0, consumer, wireImpact = true) {
  return {
    symbol,
    owner: 'runtime-ports plugin contract owner',
    consumer,
    p0,
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
    'PluginRuntimeReadRequest',
    'PluginRuntimeReadResponse',
  ].map((symbol) =>
    pluginRuntimeEntry(
      symbol,
      'plugin discovery, status, and config-validation projection',
      'runtime-ports read-model contract tests, OpenCode fixture projection tests, and plugin-runtime-host read-model tests',
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
      'plugin permission, effect-preview, and candidate materialization',
      'runtime-ports candidate-effect contract tests and plugin-runtime-host permission/effect validation tests',
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
      'runtime-ports diagnostics tests and plugin-runtime-host quarantine/read-model owner tests',
    ),
  ),
  ...[
    'ExtensionCapabilityAvailability',
    'PluginRuntimeAvailability',
    'PluginRuntimeUnavailableReason',
    'PluginRuntimeEpochs',
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
      'product assembly, agent runtime, runtime-ports contract tests, and plugin-runtime-host owner validation',
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
    p0: 'Plugin Runtime Host executable boundary for the OpenCode-compatible P0 vertical slice',
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
      'OpenCode adapter fixture contract must not expose public API before reviewed Plugin Runtime Host integration',
    allowedSymbolEntries: [],
  },
  {
    path: 'src/crates/execution/plugin-runtime-host/src/lib.rs',
    reason:
      'Plugin Runtime Host public API must stay limited to the injected adapter trait and host boundary type',
    allowedSymbolEntries: pluginRuntimeHostPublicApiEntries,
  },
];
