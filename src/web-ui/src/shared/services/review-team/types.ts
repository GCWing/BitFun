import type { SubagentSource } from '@/infrastructure/api/service-api/SubagentAPI';
import type {
  ReviewDomainTag,
  ReviewTargetClassification,
} from '../reviewTargetClassifier';

export type ReviewStrategyLevel = 'quick' | 'normal' | 'deep';
export type ReviewMemberStrategyLevel = ReviewStrategyLevel | 'inherit';
export type ReviewStrategySource = 'team' | 'member';
export type ReviewModelFallbackReason = 'model_removed';

export type DeepReviewScopeReviewDepth =
  | 'high_risk_only'
  | 'risk_expanded'
  | 'full_depth';
export type DeepReviewScopeDependencyHops = number | 'policy_limited';
export type DeepReviewOptionalReviewerPolicy =
  | 'risk_matched_only'
  | 'configured'
  | 'full';
export type DeepReviewRiskFocusTag =
  | 'security'
  | 'data_loss'
  | 'migrations'
  | 'authentication_authorization'
  | 'cross_boundary_api_contracts'
  | 'concurrency'
  | 'persistence'
  | 'configuration_changes'
  | 'platform_boundary_violations';

export interface DeepReviewScopeProfile {
  reviewDepth: DeepReviewScopeReviewDepth;
  riskFocusTags: DeepReviewRiskFocusTag[];
  maxDependencyHops: DeepReviewScopeDependencyHops;
  optionalReviewerPolicy: DeepReviewOptionalReviewerPolicy;
  allowBroadToolExploration: boolean;
  coverageExpectation: string;
}

export type DeepReviewEvidencePackSource = 'target_manifest';
export type DeepReviewEvidencePackContentBoundary = 'metadata_only';
export type DeepReviewEvidencePackContractHintKind =
  | 'i18n_key'
  | 'tauri_command'
  | 'api_contract'
  | 'config_key';

export interface DeepReviewEvidencePackDiffStat {
  fileCount: number;
  totalChangedLines?: number;
  lineCountSource: ReviewTeamChangeStats['lineCountSource'];
}

export interface DeepReviewEvidencePackHunkHint {
  filePath: string;
  changedLineCount: number;
  lineCountSource: ReviewTeamChangeStats['lineCountSource'];
}

export interface DeepReviewEvidencePackContractHint {
  kind: DeepReviewEvidencePackContractHintKind;
  filePath: string;
  source: 'path_classifier';
}

export interface DeepReviewEvidencePackBudget {
  maxChangedFiles: number;
  maxHunkHints: number;
  maxContractHints: number;
  omittedChangedFileCount: number;
  omittedHunkHintCount: number;
  omittedContractHintCount: number;
}

export interface DeepReviewEvidencePackPrivacyBoundary {
  content: DeepReviewEvidencePackContentBoundary;
  excludes: [
    'source_text',
    'full_diff',
    'model_output',
    'provider_raw_body',
    'full_file_contents',
  ];
}

export interface DeepReviewEvidencePack {
  version: 1;
  source: DeepReviewEvidencePackSource;
  changedFiles: string[];
  diffStat: DeepReviewEvidencePackDiffStat;
  domainTags: ReviewDomainTag[];
  riskFocusTags: DeepReviewRiskFocusTag[];
  packetIds: string[];
  hunkHints: DeepReviewEvidencePackHunkHint[];
  contractHints: DeepReviewEvidencePackContractHint[];
  budget: DeepReviewEvidencePackBudget;
  privacy: DeepReviewEvidencePackPrivacyBoundary;
}

export interface ReviewStrategyCommonRules {
  reviewerPromptRules: string[];
}

export type ReviewRoleDirectiveKey = string;

export interface ReviewStrategyProfile {
  level: ReviewStrategyLevel;
  label: string;
  summary: string;
  tokenImpact: string;
  runtimeImpact: string;
  defaultModelSlot: 'fast' | 'primary';
  promptDirective: string;
  /** Per-role strategy directives. When a role key is present, its directive
   *  overrides `promptDirective` for that reviewer or the judge. */
  roleDirectives: Record<ReviewRoleDirectiveKey, string>;
}

export type ReviewTeamCoreRoleKey = string;

export interface ReviewTeamCoreRoleDefinition {
  key: ReviewTeamCoreRoleKey;
  subagentId: string;
  funName: string;
  roleName: string;
  description: string;
  responsibilities: string[];
  accentColor: string;
  /** If true, this reviewer is only included when the change contains relevant files. */
  conditional?: boolean;
}

export interface ReviewTeamDefinition {
  id: string;
  name: string;
  description: string;
  warning: string;
  defaultModel: string;
  defaultStrategyLevel: ReviewStrategyLevel;
  defaultExecutionPolicy: ReviewTeamExecutionPolicy;
  coreRoles: ReviewTeamCoreRoleDefinition[];
  strategyProfiles: Record<ReviewStrategyLevel, ReviewStrategyProfile>;
  disallowedExtraSubagentIds: string[];
  hiddenAgentIds: string[];
}

export interface ReviewTeamStoredConfig {
  extra_subagent_ids: string[];
  strategy_level: ReviewStrategyLevel;
  member_strategy_overrides: Record<string, ReviewStrategyLevel>;
  reviewer_timeout_seconds: number;
  judge_timeout_seconds: number;
  reviewer_file_split_threshold: number;
  max_same_role_instances: number;
  max_retries_per_role: number;
  max_parallel_reviewers: number;
  max_queue_wait_seconds: number;
  allow_provider_capacity_queue: boolean;
  allow_bounded_auto_retry: boolean;
  auto_retry_elapsed_guard_seconds: number;
}

export interface ReviewTeamExecutionPolicy {
  reviewerTimeoutSeconds: number;
  judgeTimeoutSeconds: number;
  reviewerFileSplitThreshold: number;
  maxSameRoleInstances: number;
  maxRetriesPerRole: number;
}

export interface ReviewTeamConcurrencyPolicy {
  maxParallelInstances: number;
  staggerSeconds: number;
  maxQueueWaitSeconds: number;
  batchExtrasSeparately: boolean;
  allowProviderCapacityQueue: boolean;
  allowBoundedAutoRetry: boolean;
  autoRetryElapsedGuardSeconds: number;
}

export interface ReviewTeamRateLimitStatus {
  remaining: number;
}

export type ReviewTeamManifestMemberReason =
  | 'disabled'
  | 'unavailable'
  | 'not_applicable'
  | 'budget_limited'
  | 'invalid_tooling';

export type ReviewTokenBudgetMode = 'economy' | 'balanced' | 'thorough';
export type ReviewPromptByteEstimateSource = 'manifest_heuristic';
export type ReviewTeamTokenBudgetDecisionKind =
  | 'summary_first_full_scope'
  | 'skip_extra_reviewers';
export type ReviewTeamTokenBudgetDecisionReason =
  | 'prompt_bytes_exceeded'
  | 'extra_reviewers_skipped';

export interface ReviewTeamTokenBudgetDecision {
  kind: ReviewTeamTokenBudgetDecisionKind;
  reason: ReviewTeamTokenBudgetDecisionReason;
  detail: string;
  affectedReviewerIds?: string[];
}

export interface ReviewTeamTokenBudgetPlan {
  mode: ReviewTokenBudgetMode;
  estimatedReviewerCalls: number;
  maxReviewerCalls: number;
  maxExtraReviewers: number;
  maxFilesPerReviewer?: number;
  maxPromptBytesPerReviewer?: number;
  estimatedPromptBytesPerReviewer?: number;
  promptByteEstimateSource?: ReviewPromptByteEstimateSource;
  promptByteLimitExceeded?: boolean;
  largeDiffSummaryFirst: boolean;
  decisions?: ReviewTeamTokenBudgetDecision[];
  skippedReviewerIds: string[];
  warnings: string[];
}

export interface ReviewTeamChangeStats {
  fileCount: number;
  totalLinesChanged?: number;
  lineCountSource: 'unknown' | 'diff_stat' | 'estimated';
}

export interface ReviewTeamRiskFactors {
  fileCount: number;
  totalLinesChanged?: number;
  lineCountSource: ReviewTeamChangeStats['lineCountSource'];
  securityFileCount: number;
  workspaceAreaCount: number;
  contractSurfaceChanged: boolean;
}

export interface ReviewTeamStrategyRecommendation {
  strategyLevel: ReviewStrategyLevel;
  score: number;
  rationale: string;
  factors: ReviewTeamRiskFactors;
}

export type ReviewTeamStrategyAuthority = 'mismatch_warning';
export type ReviewTeamStrategyMismatchSeverity = 'none' | 'low' | 'medium' | 'high';

export interface ReviewTeamBackendRiskFactors {
  fileCount: number;
  totalLinesChanged: number;
  lineCountSource: ReviewTeamChangeStats['lineCountSource'];
  filesInSecurityPaths: number;
  crossCrateChanges: number;
  maxCyclomaticComplexityDelta: number;
  maxCyclomaticComplexityDeltaSource: 'not_measured';
}

export interface ReviewTeamBackendStrategyRecommendation {
  strategyLevel: ReviewStrategyLevel;
  score: number;
  rationale: string;
  factors: ReviewTeamBackendRiskFactors;
}

export interface ReviewTeamStrategyDecision {
  authority: ReviewTeamStrategyAuthority;
  teamDefaultStrategy: ReviewStrategyLevel;
  userOverride?: ReviewStrategyLevel;
  finalStrategy: ReviewStrategyLevel;
  frontendRecommendation: ReviewTeamStrategyRecommendation;
  backendRecommendation: ReviewTeamBackendStrategyRecommendation;
  mismatch: boolean;
  mismatchSeverity: ReviewTeamStrategyMismatchSeverity;
  rationale: string;
}

export interface ReviewTeamPreReviewSummaryArea {
  key: string;
  fileCount: number;
  sampleFiles: string[];
}

export interface ReviewTeamPreReviewSummary {
  source: 'target_manifest';
  summary: string;
  fileCount: number;
  excludedFileCount: number;
  lineCount?: number;
  lineCountSource: ReviewTeamChangeStats['lineCountSource'];
  targetTags: ReviewDomainTag[];
  workspaceAreas: ReviewTeamPreReviewSummaryArea[];
  warnings: ReviewTargetClassification['warnings'][number]['code'][];
}

export type ReviewTeamSharedContextTool = 'GetFileDiff' | 'Read';

export interface ReviewTeamSharedContextCacheEntry {
  cacheKey: string;
  path: string;
  workspaceArea: string;
  recommendedTools: ReviewTeamSharedContextTool[];
  consumerPacketIds: string[];
}

export interface ReviewTeamSharedContextCachePlan {
  source: 'work_packets';
  strategy: 'reuse_readonly_file_context_by_cache_key';
  entries: ReviewTeamSharedContextCacheEntry[];
  omittedEntryCount: number;
}

export type ReviewTeamIncrementalReviewCacheInvalidation =
  | 'target_file_set_changed'
  | 'target_line_count_changed'
  | 'target_tag_changed'
  | 'target_warning_changed'
  | 'reviewer_roster_changed'
  | 'strategy_changed';

export interface ReviewTeamIncrementalReviewCachePlan {
  source: 'target_manifest';
  strategy: 'reuse_completed_packets_when_fingerprint_matches';
  cacheKey: string;
  fingerprint: string;
  filePaths: string[];
  workspaceAreas: string[];
  targetTags: ReviewDomainTag[];
  reviewerPacketIds: string[];
  lineCount?: number;
  lineCountSource: ReviewTeamChangeStats['lineCountSource'];
  invalidatesOn: ReviewTeamIncrementalReviewCacheInvalidation[];
}

export interface ReviewTeamWorkPacketScope {
  kind: 'review_target';
  targetSource: ReviewTargetClassification['source'];
  targetResolution: ReviewTargetClassification['resolution'];
  targetTags: ReviewDomainTag[];
  fileCount: number;
  files: string[];
  excludedFileCount: number;
  groupIndex?: number;
  groupCount?: number;
}

export interface ReviewTeamWorkPacket {
  packetId: string;
  phase: 'reviewer' | 'judge';
  launchBatch: number;
  subagentId: string;
  displayName: string;
  roleName: string;
  assignedScope: ReviewTeamWorkPacketScope;
  allowedTools: string[];
  timeoutSeconds: number;
  requiredOutputFields: string[];
  strategyLevel: ReviewStrategyLevel;
  strategyDirective: string;
  model: string;
}

export interface ReviewTeamMember {
  id: string;
  subagentId: string;
  definitionKey?: ReviewTeamCoreRoleKey;
  conditional?: boolean;
  displayName: string;
  roleName: string;
  description: string;
  responsibilities: string[];
  model: string;
  configuredModel: string;
  modelFallbackReason?: ReviewModelFallbackReason;
  strategyOverride: ReviewMemberStrategyLevel;
  strategyLevel: ReviewStrategyLevel;
  strategySource: ReviewStrategySource;
  enabled: boolean;
  available: boolean;
  locked: boolean;
  source: 'core' | 'extra';
  subagentSource: SubagentSource;
  accentColor: string;
  allowedTools: string[];
  defaultModelSlot?: ReviewStrategyProfile['defaultModelSlot'];
  strategyDirective?: string;
  skipReason?: ReviewTeamManifestMemberReason;
}

export interface ReviewTeam {
  id: string;
  name: string;
  description: string;
  warning: string;
  strategyLevel: ReviewStrategyLevel;
  memberStrategyOverrides: Record<string, ReviewStrategyLevel>;
  executionPolicy: ReviewTeamExecutionPolicy;
  concurrencyPolicy: ReviewTeamConcurrencyPolicy;
  definition: ReviewTeamDefinition;
  members: ReviewTeamMember[];
  coreMembers: ReviewTeamMember[];
  extraMembers: ReviewTeamMember[];
}

export interface ReviewTeamManifestMember {
  subagentId: string;
  displayName: string;
  roleName: string;
  model: string;
  configuredModel: string;
  modelFallbackReason?: ReviewModelFallbackReason;
  defaultModelSlot: ReviewStrategyProfile['defaultModelSlot'];
  strategyLevel: ReviewStrategyLevel;
  strategySource: ReviewStrategySource;
  strategyDirective: string;
  locked: boolean;
  source: ReviewTeamMember['source'];
  subagentSource: ReviewTeamMember['subagentSource'];
  reason?: ReviewTeamManifestMemberReason;
}

export interface ReviewTeamRunManifest {
  reviewMode: 'deep';
  workspacePath?: string;
  policySource: 'default-review-team-config';
  target: ReviewTargetClassification;
  strategyLevel: ReviewStrategyLevel;
  scopeProfile?: DeepReviewScopeProfile;
  strategyRecommendation?: ReviewTeamStrategyRecommendation;
  strategyDecision: ReviewTeamStrategyDecision;
  executionPolicy: ReviewTeamExecutionPolicy;
  concurrencyPolicy: ReviewTeamConcurrencyPolicy;
  changeStats?: ReviewTeamChangeStats;
  preReviewSummary: ReviewTeamPreReviewSummary;
  evidencePack?: DeepReviewEvidencePack;
  sharedContextCache: ReviewTeamSharedContextCachePlan;
  incrementalReviewCache: ReviewTeamIncrementalReviewCachePlan;
  tokenBudget: ReviewTeamTokenBudgetPlan;
  coreReviewers: ReviewTeamManifestMember[];
  qualityGateReviewer?: ReviewTeamManifestMember;
  enabledExtraReviewers: ReviewTeamManifestMember[];
  skippedReviewers: ReviewTeamManifestMember[];
  workPackets?: ReviewTeamWorkPacket[];
}

export function getActiveReviewTeamManifestMembers(
  manifest: ReviewTeamRunManifest,
): ReviewTeamManifestMember[] {
  return [
    ...manifest.coreReviewers,
    ...manifest.enabledExtraReviewers,
    ...(manifest.qualityGateReviewer ? [manifest.qualityGateReviewer] : []),
  ];
}
