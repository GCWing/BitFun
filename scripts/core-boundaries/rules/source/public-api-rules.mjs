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
      'desktop settings, CLI diagnostics, and runtime host read-model tests',
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
      'runtime-ports dispatch response tests and product P0 permission projection',
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
    'PluginRecoveryAction',
    'PluginRecoveryActionKind',
    'PluginRecoveryActionRequest',
    'PluginRecoveryActionResult',
    'PluginRecoveryActionStatus',
  ].map((symbol) =>
    pluginRuntimeEntry(
      symbol,
      'plugin diagnostics, quarantine, and owner-mediated recovery',
      'runtime-ports diagnostics tests and product P0 plugin settings recovery projection',
    ),
  ),
  ...[
    'ExtensionCapabilityAvailability',
    'PluginRuntimeAvailability',
    'PluginRuntimeUnavailableReason',
    'PluginRuntimeEpochs',
    'PluginDispatchEnvelope',
    'PluginResponseEnvelope',
    'PluginHostLifecycleEvent',
    'PluginHostLifecyclePhase',
    'PluginRuntimeClient',
    'DisabledPluginRuntimeClient',
    'ProjectionOnlyPluginRuntimeClient',
    'PluginRuntimeBinding',
  ].map((symbol) =>
    pluginRuntimeEntry(
      symbol,
      'plugin host boundary, lifecycle, and execution availability',
      'product assembly, agent runtime, and runtime-ports contract tests',
    ),
  ),
];

export const pluginRuntimePublicApiSymbols = pluginRuntimePublicApiEntries.map(
  (entry) => entry.symbol,
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
      'OpenCode adapter fixture contract must not expose public API before reviewed Plugin Runtime Host integration',
    allowedSymbolEntries: [],
  },
];
