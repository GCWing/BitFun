// Public Review Team service facade over smaller implementation modules.

import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import {
  SubagentAPI,
  type SubagentInfo,
} from '@/infrastructure/api/service-api/SubagentAPI';
import {
  classifyReviewTargetFromFiles,
  createUnknownReviewTargetClassification,
  shouldRunReviewerForTarget,
  type ReviewTargetClassification,
} from '../reviewTargetClassifier';
import { evaluateReviewSubagentToolReadiness } from '../reviewSubagentCapabilities';
import {
  DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL,
  CORE_ROLE_IDS,
  DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY,
  DEFAULT_REVIEW_TEAM_CONFIG_PATH,
  DEFAULT_REVIEW_TEAM_CORE_ROLES,
  DEFAULT_REVIEW_TEAM_EXECUTION_POLICY,
  DEFAULT_REVIEW_TEAM_MODEL,
  DEFAULT_REVIEW_TEAM_PROJECT_STRATEGY_OVERRIDES_CONFIG_PATH,
  DEFAULT_REVIEW_TEAM_RATE_LIMIT_STATUS_CONFIG_PATH,
  DEFAULT_REVIEW_TEAM_STRATEGY_LEVEL,
  DISALLOWED_REVIEW_TEAM_MEMBER_IDS,
  EXTRA_MEMBER_DEFAULTS,
  FALLBACK_REVIEW_TEAM_DEFINITION,
  JUDGE_WORK_PACKET_REQUIRED_OUTPUT_FIELDS,
  MAX_AUTO_RETRY_ELAPSED_GUARD_SECONDS,
  MAX_PARALLEL_REVIEWER_INSTANCES,
  MAX_PREDICTIVE_TIMEOUT_SECONDS,
  MAX_QUEUE_WAIT_SECONDS,
  PREDICTIVE_TIMEOUT_BASE_SECONDS,
  PREDICTIVE_TIMEOUT_PER_100_LINES_SECONDS,
  PREDICTIVE_TIMEOUT_PER_FILE_SECONDS,
  PROMPT_BYTE_ESTIMATE_BASE_BYTES,
  PROMPT_BYTE_ESTIMATE_PER_CHANGED_LINE_BYTES,
  PROMPT_BYTE_ESTIMATE_PER_FILE_BYTES,
  PROMPT_BYTE_ESTIMATE_UNKNOWN_LINES_PER_FILE,
  REVIEWER_WORK_PACKET_REQUIRED_OUTPUT_FIELDS,
  REVIEW_WORK_PACKET_ALLOWED_TOOLS,
  TOKEN_BUDGET_PROMPT_BYTE_LIMIT_BY_MODE,
} from './defaults';
import {
  REVIEW_STRATEGY_COMMON_RULES,
  REVIEW_STRATEGY_LEVELS,
  REVIEW_STRATEGY_PROFILES,
  getReviewStrategyProfile,
} from './strategy';
import type {
  ReviewMemberStrategyLevel,
  ReviewModelFallbackReason,
  ReviewRoleDirectiveKey,
  ReviewStrategyLevel,
  ReviewStrategyProfile,
  ReviewStrategySource,
  ReviewTeam,
  ReviewTeamBackendRiskFactors,
  ReviewTeamBackendStrategyRecommendation,
  ReviewTeamChangeStats,
  ReviewTeamConcurrencyPolicy,
  ReviewTeamCoreRoleDefinition,
  ReviewTeamCoreRoleKey,
  ReviewTeamDefinition,
  ReviewTeamExecutionPolicy,
  ReviewTeamIncrementalReviewCacheInvalidation,
  ReviewTeamIncrementalReviewCachePlan,
  ReviewTeamManifestMember,
  ReviewTeamManifestMemberReason,
  ReviewTeamMember,
  ReviewTeamPreReviewSummary,
  ReviewTeamRateLimitStatus,
  ReviewTeamRiskFactors,
  ReviewTeamRunManifest,
  ReviewTeamSharedContextCachePlan,
  ReviewTeamSharedContextTool,
  ReviewTeamStoredConfig,
  ReviewTeamStrategyDecision,
  ReviewTeamStrategyMismatchSeverity,
  ReviewTeamStrategyRecommendation,
  ReviewTeamTokenBudgetDecision,
  ReviewTeamTokenBudgetPlan,
  ReviewTeamWorkPacket,
  ReviewTeamWorkPacketScope,
  ReviewTokenBudgetMode,
} from './types';

export * from './types';
export * from './strategy';
export {
  DEFAULT_REVIEW_TEAM_ID,
  DEFAULT_REVIEW_TEAM_CONFIG_PATH,
  DEFAULT_REVIEW_TEAM_RATE_LIMIT_STATUS_CONFIG_PATH,
  DEFAULT_REVIEW_TEAM_PROJECT_STRATEGY_OVERRIDES_CONFIG_PATH,
  DEFAULT_REVIEW_TEAM_MODEL,
  DEFAULT_REVIEW_TEAM_STRATEGY_LEVEL,
  DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL,
  DEFAULT_REVIEW_TEAM_EXECUTION_POLICY,
  DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY,
  DEFAULT_REVIEW_TEAM_CORE_ROLES,
  FALLBACK_REVIEW_TEAM_DEFINITION,
} from './defaults';

function isReviewTeamCoreRoleDefinition(value: unknown): value is ReviewTeamCoreRoleDefinition {
  if (!value || typeof value !== 'object') return false;
  const role = value as Partial<ReviewTeamCoreRoleDefinition>;
  return (
    typeof role.key === 'string' &&
    typeof role.subagentId === 'string' &&
    typeof role.funName === 'string' &&
    typeof role.roleName === 'string' &&
    typeof role.description === 'string' &&
    Array.isArray(role.responsibilities) &&
    role.responsibilities.every((item) => typeof item === 'string') &&
    typeof role.accentColor === 'string'
  );
}

function isReviewStrategyProfile(value: unknown): value is ReviewStrategyProfile {
  if (!value || typeof value !== 'object') return false;
  const profile = value as Partial<ReviewStrategyProfile>;
  return (
    isReviewStrategyLevel(profile.level) &&
    typeof profile.label === 'string' &&
    typeof profile.summary === 'string' &&
    typeof profile.tokenImpact === 'string' &&
    typeof profile.runtimeImpact === 'string' &&
    (profile.defaultModelSlot === 'fast' || profile.defaultModelSlot === 'primary') &&
    typeof profile.promptDirective === 'string' &&
    Boolean(profile.roleDirectives) &&
    typeof profile.roleDirectives === 'object'
  );
}

function nonEmptyStringOrFallback(value: unknown, fallback: string): string {
  if (typeof value !== 'string') {
    return fallback;
  }

  return value.trim() || fallback;
}

function normalizeReviewTeamDefinition(raw: unknown): ReviewTeamDefinition {
  if (!raw || typeof raw !== 'object') {
    return FALLBACK_REVIEW_TEAM_DEFINITION;
  }

  const source = raw as Partial<ReviewTeamDefinition>;
  const coreRoles = Array.isArray(source.coreRoles)
    ? source.coreRoles.filter(isReviewTeamCoreRoleDefinition)
    : [];
  const strategyProfiles = REVIEW_STRATEGY_LEVELS.reduce<
    Partial<Record<ReviewStrategyLevel, ReviewStrategyProfile>>
  >((profiles, level) => {
    const profile = source.strategyProfiles?.[level];
    profiles[level] = isReviewStrategyProfile(profile)
      ? profile
      : FALLBACK_REVIEW_TEAM_DEFINITION.strategyProfiles[level];
    return profiles;
  }, {}) as Record<ReviewStrategyLevel, ReviewStrategyProfile>;
  const disallowedExtraSubagentIds = Array.isArray(source.disallowedExtraSubagentIds)
    ? dedupeIds(source.disallowedExtraSubagentIds.filter((id): id is string => typeof id === 'string'))
    : [];
  const hiddenAgentIds = Array.isArray(source.hiddenAgentIds)
    ? dedupeIds(source.hiddenAgentIds.filter((id): id is string => typeof id === 'string'))
    : [];

  return {
    id: nonEmptyStringOrFallback(source.id, FALLBACK_REVIEW_TEAM_DEFINITION.id),
    name: nonEmptyStringOrFallback(source.name, FALLBACK_REVIEW_TEAM_DEFINITION.name),
    description: nonEmptyStringOrFallback(
      source.description,
      FALLBACK_REVIEW_TEAM_DEFINITION.description,
    ),
    warning: nonEmptyStringOrFallback(
      source.warning,
      FALLBACK_REVIEW_TEAM_DEFINITION.warning,
    ),
    defaultModel: nonEmptyStringOrFallback(
      source.defaultModel,
      FALLBACK_REVIEW_TEAM_DEFINITION.defaultModel,
    ),
    defaultStrategyLevel: isReviewStrategyLevel(source.defaultStrategyLevel)
      ? source.defaultStrategyLevel
      : FALLBACK_REVIEW_TEAM_DEFINITION.defaultStrategyLevel,
    defaultExecutionPolicy: source.defaultExecutionPolicy
      ? {
        reviewerTimeoutSeconds: clampInteger(
          source.defaultExecutionPolicy.reviewerTimeoutSeconds,
          0,
          3600,
          FALLBACK_REVIEW_TEAM_DEFINITION.defaultExecutionPolicy.reviewerTimeoutSeconds,
        ),
        judgeTimeoutSeconds: clampInteger(
          source.defaultExecutionPolicy.judgeTimeoutSeconds,
          0,
          3600,
          FALLBACK_REVIEW_TEAM_DEFINITION.defaultExecutionPolicy.judgeTimeoutSeconds,
        ),
        reviewerFileSplitThreshold: clampInteger(
          source.defaultExecutionPolicy.reviewerFileSplitThreshold,
          0,
          9999,
          FALLBACK_REVIEW_TEAM_DEFINITION.defaultExecutionPolicy.reviewerFileSplitThreshold,
        ),
        maxSameRoleInstances: clampInteger(
          source.defaultExecutionPolicy.maxSameRoleInstances,
          1,
          8,
          FALLBACK_REVIEW_TEAM_DEFINITION.defaultExecutionPolicy.maxSameRoleInstances,
        ),
        maxRetriesPerRole: clampInteger(
          source.defaultExecutionPolicy.maxRetriesPerRole,
          0,
          3,
          FALLBACK_REVIEW_TEAM_DEFINITION.defaultExecutionPolicy.maxRetriesPerRole,
        ),
      }
      : FALLBACK_REVIEW_TEAM_DEFINITION.defaultExecutionPolicy,
    coreRoles: coreRoles.length > 0 ? coreRoles : FALLBACK_REVIEW_TEAM_DEFINITION.coreRoles,
    strategyProfiles,
    disallowedExtraSubagentIds:
      disallowedExtraSubagentIds.length > 0
        ? disallowedExtraSubagentIds
        : FALLBACK_REVIEW_TEAM_DEFINITION.disallowedExtraSubagentIds,
    hiddenAgentIds:
      hiddenAgentIds.length > 0
        ? hiddenAgentIds
        : FALLBACK_REVIEW_TEAM_DEFINITION.hiddenAgentIds,
  };
}

export async function loadDefaultReviewTeamDefinition(): Promise<ReviewTeamDefinition> {
  try {
    return normalizeReviewTeamDefinition(
      await agentAPI.getDefaultReviewTeamDefinition(),
    );
  } catch {
    return FALLBACK_REVIEW_TEAM_DEFINITION;
  }
}

function dedupeIds(ids: string[]): string[] {
  return Array.from(
    new Set(
      ids
        .map((id) => id.trim())
        .filter(Boolean),
    ),
  );
}

function isReviewStrategyLevel(value: unknown): value is ReviewStrategyLevel {
  return (
    typeof value === 'string' &&
    REVIEW_STRATEGY_LEVELS.includes(value as ReviewStrategyLevel)
  );
}

function normalizeTeamStrategyLevel(value: unknown): ReviewStrategyLevel {
  return isReviewStrategyLevel(value)
    ? value
    : DEFAULT_REVIEW_TEAM_STRATEGY_LEVEL;
}

function normalizeMemberStrategyOverrides(
  raw: unknown,
): Record<string, ReviewStrategyLevel> {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
    return {};
  }

  return Object.entries(raw as Record<string, unknown>).reduce<
    Record<string, ReviewStrategyLevel>
  >((result, [subagentId, value]) => {
    const normalizedId = subagentId.trim();
    if (!normalizedId) {
      return result;
    }
    if (isReviewStrategyLevel(value)) {
      result[normalizedId] = value;
    } else {
      console.warn(
        `[ReviewTeamService] Ignoring invalid strategy override for '${normalizedId}': expected one of ${REVIEW_STRATEGY_LEVELS.join(', ')}, got '${value}'`,
      );
    }
    return result;
  }, {});
}

function normalizeProjectStrategyOverrideKey(workspacePath?: string): string | undefined {
  const normalized = workspacePath?.trim().replace(/\\/g, '/');
  if (!normalized) {
    return undefined;
  }
  if (normalized === '/' || /^[a-zA-Z]:\/$/.test(normalized)) {
    return normalized.toLowerCase();
  }
  return normalized.replace(/\/+$/, '').toLowerCase();
}

function normalizeProjectStrategyOverrideStore(
  raw: unknown,
): Record<string, ReviewStrategyLevel> {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
    return {};
  }

  return Object.entries(raw as Record<string, unknown>).reduce<
    Record<string, ReviewStrategyLevel>
  >((result, [workspacePath, value]) => {
    const key = normalizeProjectStrategyOverrideKey(workspacePath);
    if (!key) {
      return result;
    }
    if (isReviewStrategyLevel(value)) {
      result[key] = value;
    } else {
      console.warn(
        `[ReviewTeamService] Ignoring invalid project strategy override for '${key}': expected one of ${REVIEW_STRATEGY_LEVELS.join(', ')}, got '${value}'`,
      );
    }
    return result;
  }, {});
}

function clampInteger(
  value: unknown,
  min: number,
  max: number,
  fallback: number,
): number {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    return fallback;
  }

  return Math.min(max, Math.max(min, Math.floor(numeric)));
}

function normalizeConcurrencyPolicy(
  raw?: Partial<ReviewTeamConcurrencyPolicy>,
): ReviewTeamConcurrencyPolicy {
  return {
    maxParallelInstances: clampInteger(
      raw?.maxParallelInstances,
      1,
      MAX_PARALLEL_REVIEWER_INSTANCES,
      DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.maxParallelInstances,
    ),
    staggerSeconds: clampInteger(
      raw?.staggerSeconds,
      0,
      60,
      DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.staggerSeconds,
    ),
    maxQueueWaitSeconds: clampInteger(
      raw?.maxQueueWaitSeconds,
      0,
      MAX_QUEUE_WAIT_SECONDS,
      DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.maxQueueWaitSeconds,
    ),
    batchExtrasSeparately:
      typeof raw?.batchExtrasSeparately === 'boolean'
        ? raw.batchExtrasSeparately
        : DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.batchExtrasSeparately,
    allowProviderCapacityQueue:
      typeof raw?.allowProviderCapacityQueue === 'boolean'
        ? raw.allowProviderCapacityQueue
        : DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.allowProviderCapacityQueue,
    allowBoundedAutoRetry:
      typeof raw?.allowBoundedAutoRetry === 'boolean'
        ? raw.allowBoundedAutoRetry
        : DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.allowBoundedAutoRetry,
    autoRetryElapsedGuardSeconds: clampInteger(
      raw?.autoRetryElapsedGuardSeconds,
      30,
      MAX_AUTO_RETRY_ELAPSED_GUARD_SECONDS,
      DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.autoRetryElapsedGuardSeconds,
    ),
  };
}

function normalizeStoredConcurrencyPolicy(
  raw: unknown,
): Pick<
  ReviewTeamStoredConfig,
  | 'max_parallel_reviewers'
  | 'max_queue_wait_seconds'
  | 'allow_provider_capacity_queue'
  | 'allow_bounded_auto_retry'
  | 'auto_retry_elapsed_guard_seconds'
> {
  const config = raw as Partial<ReviewTeamStoredConfig> | undefined;

  return {
    max_parallel_reviewers: clampInteger(
      config?.max_parallel_reviewers,
      1,
      MAX_PARALLEL_REVIEWER_INSTANCES,
      DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.maxParallelInstances,
    ),
    max_queue_wait_seconds: clampInteger(
      config?.max_queue_wait_seconds,
      0,
      MAX_QUEUE_WAIT_SECONDS,
      DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.maxQueueWaitSeconds,
    ),
    allow_provider_capacity_queue:
      typeof config?.allow_provider_capacity_queue === 'boolean'
        ? config.allow_provider_capacity_queue
        : DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.allowProviderCapacityQueue,
    allow_bounded_auto_retry:
      typeof config?.allow_bounded_auto_retry === 'boolean'
        ? config.allow_bounded_auto_retry
        : DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.allowBoundedAutoRetry,
    auto_retry_elapsed_guard_seconds: clampInteger(
      config?.auto_retry_elapsed_guard_seconds,
      30,
      MAX_AUTO_RETRY_ELAPSED_GUARD_SECONDS,
      DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.autoRetryElapsedGuardSeconds,
    ),
  };
}

function applyRateLimitToConcurrencyPolicy(
  policy: ReviewTeamConcurrencyPolicy,
  rateLimitStatus?: ReviewTeamRateLimitStatus | null,
): ReviewTeamConcurrencyPolicy {
  const remaining = Math.floor(Number(rateLimitStatus?.remaining));
  if (!Number.isFinite(remaining)) {
    return policy;
  }

  if (remaining > policy.maxParallelInstances * 2) {
    return policy;
  }

  if (remaining > policy.maxParallelInstances) {
    return {
      ...policy,
      staggerSeconds: Math.max(policy.staggerSeconds, 5),
    };
  }

  return {
    ...policy,
    maxParallelInstances: Math.max(
      1,
      Math.min(policy.maxParallelInstances, Math.max(2, remaining)),
    ),
    staggerSeconds: Math.max(policy.staggerSeconds, 10),
  };
}

function normalizeRateLimitStatus(raw: unknown): ReviewTeamRateLimitStatus | null {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
    return null;
  }

  const remaining = Math.floor(Number((raw as { remaining?: unknown }).remaining));
  if (!Number.isFinite(remaining)) {
    return null;
  }

  return {
    remaining: Math.max(0, remaining),
  };
}

function normalizeExecutionPolicy(
  raw: unknown,
): Pick<
  ReviewTeamStoredConfig,
  | 'reviewer_timeout_seconds'
  | 'judge_timeout_seconds'
  | 'reviewer_file_split_threshold'
  | 'max_same_role_instances'
  | 'max_retries_per_role'
> {
  const config = raw as Partial<ReviewTeamStoredConfig> | undefined;

  return {
    reviewer_timeout_seconds: clampInteger(
      config?.reviewer_timeout_seconds,
      0,
      3600,
      DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.reviewerTimeoutSeconds,
    ),
    judge_timeout_seconds: clampInteger(
      config?.judge_timeout_seconds,
      0,
      3600,
      DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.judgeTimeoutSeconds,
    ),
    reviewer_file_split_threshold: clampInteger(
      config?.reviewer_file_split_threshold,
      0,
      9999,
      DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.reviewerFileSplitThreshold,
    ),
    max_same_role_instances: clampInteger(
      config?.max_same_role_instances,
      1,
      8,
      DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.maxSameRoleInstances,
    ),
    max_retries_per_role: clampInteger(
      config?.max_retries_per_role,
      0,
      3,
      DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.maxRetriesPerRole,
    ),
  };
}

function executionPolicyFromStoredConfig(
  config: ReviewTeamStoredConfig,
): ReviewTeamExecutionPolicy {
  return {
    reviewerTimeoutSeconds: config.reviewer_timeout_seconds,
    judgeTimeoutSeconds: config.judge_timeout_seconds,
    reviewerFileSplitThreshold: config.reviewer_file_split_threshold,
    maxSameRoleInstances: config.max_same_role_instances,
    maxRetriesPerRole: config.max_retries_per_role,
  };
}

function concurrencyPolicyFromStoredConfig(
  config: ReviewTeamStoredConfig,
): ReviewTeamConcurrencyPolicy {
  return normalizeConcurrencyPolicy({
    maxParallelInstances: config.max_parallel_reviewers,
    staggerSeconds: DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.staggerSeconds,
    maxQueueWaitSeconds: config.max_queue_wait_seconds,
    batchExtrasSeparately: DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.batchExtrasSeparately,
    allowProviderCapacityQueue: config.allow_provider_capacity_queue,
    allowBoundedAutoRetry: config.allow_bounded_auto_retry,
    autoRetryElapsedGuardSeconds: config.auto_retry_elapsed_guard_seconds,
  });
}

function normalizeStoredConfig(raw: unknown): ReviewTeamStoredConfig {
  const extraIds = Array.isArray((raw as { extra_subagent_ids?: unknown })?.extra_subagent_ids)
    ? (raw as { extra_subagent_ids: unknown[] }).extra_subagent_ids
      .filter((value): value is string => typeof value === 'string')
    : [];
  const executionPolicy = normalizeExecutionPolicy(raw);
  const concurrencyPolicy = normalizeStoredConcurrencyPolicy(raw);
  const config = raw as Partial<ReviewTeamStoredConfig> | undefined;

  return {
    extra_subagent_ids: dedupeIds(extraIds).filter((id) => !DISALLOWED_REVIEW_TEAM_MEMBER_IDS.has(id)),
    strategy_level: normalizeTeamStrategyLevel(config?.strategy_level),
    member_strategy_overrides: normalizeMemberStrategyOverrides(
      config?.member_strategy_overrides,
    ),
    ...executionPolicy,
    ...concurrencyPolicy,
  };
}

function isMissingDefaultReviewTeamConfigError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  const normalized = message.toLowerCase();
  const quotedDefaultPath = `'${DEFAULT_REVIEW_TEAM_CONFIG_PATH.toLowerCase()}'`;
  return (
    normalized.includes('config path') &&
    normalized.includes(quotedDefaultPath) &&
    normalized.includes('not found')
  );
}

export async function loadDefaultReviewTeamConfig(): Promise<ReviewTeamStoredConfig> {
  let raw: unknown;
  try {
    raw = await configAPI.getConfig(DEFAULT_REVIEW_TEAM_CONFIG_PATH);
  } catch (error) {
    if (!isMissingDefaultReviewTeamConfigError(error)) {
      throw error;
    }
  }
  return normalizeStoredConfig(raw);
}

export async function saveDefaultReviewTeamConfig(
  config: ReviewTeamStoredConfig,
): Promise<void> {
  const normalizedConfig = normalizeStoredConfig(config);

  await configAPI.setConfig(DEFAULT_REVIEW_TEAM_CONFIG_PATH, {
    extra_subagent_ids: dedupeIds(normalizedConfig.extra_subagent_ids)
      .filter((id) => !DISALLOWED_REVIEW_TEAM_MEMBER_IDS.has(id)),
    strategy_level: normalizedConfig.strategy_level,
    member_strategy_overrides: normalizedConfig.member_strategy_overrides,
    reviewer_timeout_seconds: normalizedConfig.reviewer_timeout_seconds,
    judge_timeout_seconds: normalizedConfig.judge_timeout_seconds,
    reviewer_file_split_threshold: normalizedConfig.reviewer_file_split_threshold,
    max_same_role_instances: normalizedConfig.max_same_role_instances,
    max_retries_per_role: normalizedConfig.max_retries_per_role,
    max_parallel_reviewers: normalizedConfig.max_parallel_reviewers,
    max_queue_wait_seconds: normalizedConfig.max_queue_wait_seconds,
    allow_provider_capacity_queue: normalizedConfig.allow_provider_capacity_queue,
    allow_bounded_auto_retry: normalizedConfig.allow_bounded_auto_retry,
    auto_retry_elapsed_guard_seconds: normalizedConfig.auto_retry_elapsed_guard_seconds,
  });
}

export async function loadReviewTeamRateLimitStatus(): Promise<ReviewTeamRateLimitStatus | null> {
  try {
    const raw = await configAPI.getConfig(
      DEFAULT_REVIEW_TEAM_RATE_LIMIT_STATUS_CONFIG_PATH,
      { skipRetryOnNotFound: true },
    );
    return normalizeRateLimitStatus(raw);
  } catch (error) {
    console.warn('[ReviewTeamService] Failed to load review team rate limit status', error);
    return null;
  }
}

export async function loadReviewTeamProjectStrategyOverride(
  workspacePath?: string,
): Promise<ReviewStrategyLevel | undefined> {
  const key = normalizeProjectStrategyOverrideKey(workspacePath);
  if (!key) {
    return undefined;
  }

  try {
    const raw = await configAPI.getConfig(
      DEFAULT_REVIEW_TEAM_PROJECT_STRATEGY_OVERRIDES_CONFIG_PATH,
      { skipRetryOnNotFound: true },
    );
    return normalizeProjectStrategyOverrideStore(raw)[key];
  } catch (error) {
    console.warn('[ReviewTeamService] Failed to load project review strategy override', error);
    return undefined;
  }
}

export async function saveReviewTeamProjectStrategyOverride(
  workspacePath: string | undefined,
  strategyLevel?: ReviewStrategyLevel,
): Promise<void> {
  const key = normalizeProjectStrategyOverrideKey(workspacePath);
  if (!key) {
    return;
  }

  const raw = await configAPI.getConfig(
    DEFAULT_REVIEW_TEAM_PROJECT_STRATEGY_OVERRIDES_CONFIG_PATH,
    { skipRetryOnNotFound: true },
  ).catch(() => undefined);
  const nextOverrides = normalizeProjectStrategyOverrideStore(raw);

  if (strategyLevel) {
    nextOverrides[key] = normalizeTeamStrategyLevel(strategyLevel);
  } else {
    delete nextOverrides[key];
  }

  await configAPI.setConfig(
    DEFAULT_REVIEW_TEAM_PROJECT_STRATEGY_OVERRIDES_CONFIG_PATH,
    nextOverrides,
  );
}

export async function addDefaultReviewTeamMember(subagentId: string): Promise<void> {
  const current = await loadDefaultReviewTeamConfig();
  await saveDefaultReviewTeamConfig({
    ...current,
    extra_subagent_ids: [...current.extra_subagent_ids, subagentId],
  });
}

export async function removeDefaultReviewTeamMember(subagentId: string): Promise<void> {
  const current = await loadDefaultReviewTeamConfig();
  await saveDefaultReviewTeamConfig({
    ...current,
    extra_subagent_ids: current.extra_subagent_ids.filter((id) => id !== subagentId),
  });
}

export async function saveDefaultReviewTeamExecutionPolicy(
  policy: ReviewTeamExecutionPolicy,
): Promise<void> {
  const current = await loadDefaultReviewTeamConfig();
  await saveDefaultReviewTeamConfig({
    ...current,
    reviewer_timeout_seconds: policy.reviewerTimeoutSeconds,
    judge_timeout_seconds: policy.judgeTimeoutSeconds,
    reviewer_file_split_threshold: policy.reviewerFileSplitThreshold,
    max_same_role_instances: policy.maxSameRoleInstances,
    max_retries_per_role: policy.maxRetriesPerRole,
  });
}

export async function saveDefaultReviewTeamConcurrencyPolicy(
  policy: ReviewTeamConcurrencyPolicy,
): Promise<void> {
  const current = await loadDefaultReviewTeamConfig();
  const normalizedPolicy = normalizeConcurrencyPolicy(policy);
  await saveDefaultReviewTeamConfig({
    ...current,
    max_parallel_reviewers: normalizedPolicy.maxParallelInstances,
    max_queue_wait_seconds: normalizedPolicy.maxQueueWaitSeconds,
    allow_provider_capacity_queue: normalizedPolicy.allowProviderCapacityQueue,
    allow_bounded_auto_retry: normalizedPolicy.allowBoundedAutoRetry,
    auto_retry_elapsed_guard_seconds: normalizedPolicy.autoRetryElapsedGuardSeconds,
  });
}

export async function lowerDefaultReviewTeamMaxParallelReviewers(): Promise<ReviewTeamConcurrencyPolicy> {
  const current = await loadDefaultReviewTeamConfig();
  const currentPolicy = concurrencyPolicyFromStoredConfig(current);
  const nextPolicy = {
    ...currentPolicy,
    maxParallelInstances: Math.max(1, currentPolicy.maxParallelInstances - 1),
  };
  await saveDefaultReviewTeamConfig({
    ...current,
    max_parallel_reviewers: nextPolicy.maxParallelInstances,
    max_queue_wait_seconds: nextPolicy.maxQueueWaitSeconds,
    allow_provider_capacity_queue: nextPolicy.allowProviderCapacityQueue,
    allow_bounded_auto_retry: nextPolicy.allowBoundedAutoRetry,
    auto_retry_elapsed_guard_seconds: nextPolicy.autoRetryElapsedGuardSeconds,
  });
  return nextPolicy;
}

export async function saveDefaultReviewTeamStrategyLevel(
  strategyLevel: ReviewStrategyLevel,
): Promise<void> {
  const current = await loadDefaultReviewTeamConfig();
  await saveDefaultReviewTeamConfig({
    ...current,
    strategy_level: normalizeTeamStrategyLevel(strategyLevel),
  });
}

export async function saveDefaultReviewTeamMemberStrategyOverride(
  subagentId: string,
  strategyLevel: ReviewMemberStrategyLevel,
): Promise<void> {
  const normalizedId = subagentId.trim();
  if (!normalizedId) {
    return;
  }

  const current = await loadDefaultReviewTeamConfig();
  const nextOverrides = { ...current.member_strategy_overrides };
  if (strategyLevel === DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL) {
    delete nextOverrides[normalizedId];
  } else if (isReviewStrategyLevel(strategyLevel)) {
    nextOverrides[normalizedId] = strategyLevel;
  }

  await saveDefaultReviewTeamConfig({
    ...current,
    member_strategy_overrides: nextOverrides,
  });
}

export interface ResolveDefaultReviewTeamOptions {
  availableModelIds?: string[];
  definition?: ReviewTeamDefinition;
}

function extractAvailableModelIds(rawModels: unknown): string[] | undefined {
  if (!Array.isArray(rawModels)) {
    return undefined;
  }

  return rawModels
    .map((model) => {
      if (typeof model === 'string') {
        return model.trim();
      }
      if (model && typeof model === 'object') {
        const value = (model as { id?: unknown }).id;
        return typeof value === 'string' ? value.trim() : '';
      }
      return '';
    })
    .filter(Boolean);
}

function resolveMemberStrategy(
  storedConfig: ReviewTeamStoredConfig,
  subagentId: string,
): {
  strategyOverride: ReviewMemberStrategyLevel;
  strategyLevel: ReviewStrategyLevel;
  strategySource: ReviewStrategySource;
} {
  const override = storedConfig.member_strategy_overrides[subagentId];
  if (override) {
    return {
      strategyOverride: override,
      strategyLevel: override,
      strategySource: 'member',
    };
  }

  return {
    strategyOverride: DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL,
    strategyLevel: storedConfig.strategy_level,
    strategySource: 'team',
  };
}

function resolveMemberModel(
  configuredModel: string | undefined,
  strategyLevel: ReviewStrategyLevel,
  availableModelIds?: Set<string>,
  strategyProfiles: Record<ReviewStrategyLevel, ReviewStrategyProfile> = REVIEW_STRATEGY_PROFILES,
): {
  model: string;
  configuredModel: string;
  modelFallbackReason?: ReviewModelFallbackReason;
} {
  const normalizedConfiguredModel = configuredModel?.trim() || '';
  const defaultModelSlot = strategyProfiles[strategyLevel].defaultModelSlot;

  if (
    !normalizedConfiguredModel ||
    normalizedConfiguredModel === 'fast' ||
    normalizedConfiguredModel === 'primary'
  ) {
    return {
      model: defaultModelSlot,
      configuredModel: normalizedConfiguredModel || defaultModelSlot,
    };
  }

  if (availableModelIds && !availableModelIds.has(normalizedConfiguredModel)) {
    return {
      model: defaultModelSlot,
      configuredModel: normalizedConfiguredModel,
      modelFallbackReason: 'model_removed',
    };
  }

  return {
    model: normalizedConfiguredModel,
    configuredModel: normalizedConfiguredModel,
  };
}

function buildCoreMember(
  definition: ReviewTeamCoreRoleDefinition,
  info: SubagentInfo | undefined,
  storedConfig: ReviewTeamStoredConfig,
  availableModelIds?: Set<string>,
  strategyProfiles: Record<ReviewStrategyLevel, ReviewStrategyProfile> = REVIEW_STRATEGY_PROFILES,
): ReviewTeamMember {
  const strategy = resolveMemberStrategy(storedConfig, definition.subagentId);
  const model = resolveMemberModel(
    info?.model || DEFAULT_REVIEW_TEAM_MODEL,
    strategy.strategyLevel,
    availableModelIds,
    strategyProfiles,
  );
  const strategyProfile = strategyProfiles[strategy.strategyLevel];

  return {
    id: `core:${definition.subagentId}`,
    subagentId: definition.subagentId,
    definitionKey: definition.key,
    conditional: definition.conditional,
    displayName: definition.funName,
    roleName: definition.roleName,
    description: definition.description,
    responsibilities: definition.responsibilities,
    model: model.model,
    configuredModel: model.configuredModel,
    ...(model.modelFallbackReason
      ? { modelFallbackReason: model.modelFallbackReason }
      : {}),
    ...strategy,
    enabled: info?.enabled ?? true,
    available: Boolean(info),
    locked: true,
    source: 'core',
    subagentSource: info?.subagentSource ?? 'builtin',
    accentColor: definition.accentColor,
    allowedTools: [...REVIEW_WORK_PACKET_ALLOWED_TOOLS],
    defaultModelSlot: strategyProfile.defaultModelSlot,
    strategyDirective:
      strategyProfile.roleDirectives[definition.subagentId] ||
      strategyProfile.promptDirective,
  };
}

function buildExtraMember(
  info: SubagentInfo,
  storedConfig: ReviewTeamStoredConfig,
  availableModelIds?: Set<string>,
  options: {
    available?: boolean;
    skipReason?: ReviewTeamManifestMemberReason;
    strategyProfiles?: Record<ReviewStrategyLevel, ReviewStrategyProfile>;
  } = {},
): ReviewTeamMember {
  const strategy = resolveMemberStrategy(storedConfig, info.id);
  const strategyProfiles = options.strategyProfiles ?? REVIEW_STRATEGY_PROFILES;
  const model = resolveMemberModel(
    info.model || DEFAULT_REVIEW_TEAM_MODEL,
    strategy.strategyLevel,
    availableModelIds,
    strategyProfiles,
  );
  const strategyProfile = strategyProfiles[strategy.strategyLevel];

  return {
    id: `extra:${info.id}`,
    subagentId: info.id,
    displayName: info.name,
    roleName: EXTRA_MEMBER_DEFAULTS.roleName,
    description: info.description?.trim() || EXTRA_MEMBER_DEFAULTS.description,
    responsibilities: EXTRA_MEMBER_DEFAULTS.responsibilities,
    model: model.model,
    configuredModel: model.configuredModel,
    ...(model.modelFallbackReason
      ? { modelFallbackReason: model.modelFallbackReason }
      : {}),
    ...strategy,
    enabled: info.enabled,
    available: options.available ?? true,
    locked: false,
    source: 'extra',
    subagentSource: info.subagentSource ?? 'builtin',
    accentColor: EXTRA_MEMBER_DEFAULTS.accentColor,
    allowedTools:
      info.defaultTools && info.defaultTools.length > 0
        ? [...info.defaultTools]
        : [...REVIEW_WORK_PACKET_ALLOWED_TOOLS],
    defaultModelSlot: strategyProfile.defaultModelSlot,
    strategyDirective: strategyProfile.promptDirective,
    ...(options.skipReason ? { skipReason: options.skipReason } : {}),
  };
}

function buildUnavailableExtraMember(
  subagentId: string,
  storedConfig: ReviewTeamStoredConfig,
  availableModelIds?: Set<string>,
  strategyProfiles: Record<ReviewStrategyLevel, ReviewStrategyProfile> = REVIEW_STRATEGY_PROFILES,
): ReviewTeamMember {
  const strategy = resolveMemberStrategy(storedConfig, subagentId);
  const model = resolveMemberModel(
    DEFAULT_REVIEW_TEAM_MODEL,
    strategy.strategyLevel,
    availableModelIds,
    strategyProfiles,
  );
  const strategyProfile = strategyProfiles[strategy.strategyLevel];

  return {
    id: `extra:${subagentId}`,
    subagentId,
    displayName: subagentId,
    roleName: EXTRA_MEMBER_DEFAULTS.roleName,
    description: EXTRA_MEMBER_DEFAULTS.description,
    responsibilities: EXTRA_MEMBER_DEFAULTS.responsibilities,
    model: model.model,
    configuredModel: model.configuredModel,
    ...(model.modelFallbackReason
      ? { modelFallbackReason: model.modelFallbackReason }
      : {}),
    ...strategy,
    enabled: true,
    available: false,
    locked: false,
    source: 'extra',
    subagentSource: 'user',
    accentColor: EXTRA_MEMBER_DEFAULTS.accentColor,
    allowedTools: [],
    defaultModelSlot: strategyProfile.defaultModelSlot,
    strategyDirective: strategyProfile.promptDirective,
    skipReason: 'unavailable',
  };
}

/**
 * Context information shown in the reviewer task card instead of the raw prompt.
 * Keeps internal prompt directives private while giving the user a clear picture
 * of what each reviewer is doing.
 */
export interface ReviewerContext {
  definitionKey: ReviewTeamCoreRoleKey;
  roleName: string;
  description: string;
  responsibilities: string[];
  accentColor: string;
}

/**
 * If `subagentId` belongs to a built-in review-team role, return the
 * user-facing context for that role.  Otherwise return `null`.
 */
export function getReviewerContextBySubagentId(
  subagentId: string,
): ReviewerContext | null {
  const coreRole = DEFAULT_REVIEW_TEAM_CORE_ROLES.find(
    (role) => role.subagentId === subagentId,
  );
  if (!coreRole) return null;
  return {
    definitionKey: coreRole.key,
    roleName: coreRole.roleName,
    description: coreRole.description,
    responsibilities: coreRole.responsibilities,
    accentColor: coreRole.accentColor,
  };
}

export function isReviewTeamCoreSubagent(subagentId: string): boolean {
  return CORE_ROLE_IDS.has(subagentId);
}

export function canAddSubagentToReviewTeam(subagentId: string): boolean {
  return !DISALLOWED_REVIEW_TEAM_MEMBER_IDS.has(subagentId);
}

function hasReviewTeamExtraMemberShape(
  subagent: Pick<SubagentInfo, 'id' | 'isReadonly' | 'isReview'>,
): boolean {
  return (
    subagent.isReview &&
    subagent.isReadonly &&
    canAddSubagentToReviewTeam(subagent.id)
  );
}

export function canUseSubagentAsReviewTeamMember(
  subagent: Pick<SubagentInfo, 'id' | 'isReadonly' | 'isReview' | 'defaultTools'>,
): boolean {
  return (
    hasReviewTeamExtraMemberShape(subagent) &&
    evaluateReviewSubagentToolReadiness(subagent.defaultTools ?? []).readiness !== 'invalid'
  );
}

export function resolveDefaultReviewTeam(
  subagents: SubagentInfo[],
  storedConfig: ReviewTeamStoredConfig,
  options: ResolveDefaultReviewTeamOptions = {},
): ReviewTeam {
  const definition = options.definition ?? FALLBACK_REVIEW_TEAM_DEFINITION;
  const byId = new Map(subagents.map((subagent) => [subagent.id, subagent]));
  const availableModelIds = options.availableModelIds
    ? new Set(options.availableModelIds)
    : undefined;
  const coreMembers = definition.coreRoles.map((roleDefinition) =>
    buildCoreMember(
      roleDefinition,
      byId.get(roleDefinition.subagentId),
      storedConfig,
      availableModelIds,
      definition.strategyProfiles,
    ),
  );
  const disallowedExtraSubagentIds = new Set(definition.disallowedExtraSubagentIds);
  const extraMembers = storedConfig.extra_subagent_ids
    .filter((subagentId) => !disallowedExtraSubagentIds.has(subagentId))
    .map((subagentId) => {
    const subagent = byId.get(subagentId);
    if (!subagent) {
      return buildUnavailableExtraMember(
        subagentId,
        storedConfig,
        availableModelIds,
        definition.strategyProfiles,
      );
    }
    if (!hasReviewTeamExtraMemberShape(subagent)) {
      return buildExtraMember(subagent, storedConfig, availableModelIds, {
        available: false,
        skipReason: 'invalid_tooling',
        strategyProfiles: definition.strategyProfiles,
      });
    }
    const toolingReadiness = evaluateReviewSubagentToolReadiness(
      subagent.defaultTools ?? [],
    );
    return buildExtraMember(
      subagent,
      storedConfig,
      availableModelIds,
      toolingReadiness.readiness === 'invalid'
        ? {
          available: false,
          skipReason: 'invalid_tooling',
          strategyProfiles: definition.strategyProfiles,
        }
        : { strategyProfiles: definition.strategyProfiles },
    );
  });

  return {
    id: definition.id,
    name: definition.name,
    description: definition.description,
    warning: definition.warning,
    strategyLevel: storedConfig.strategy_level,
    memberStrategyOverrides: storedConfig.member_strategy_overrides,
    executionPolicy: executionPolicyFromStoredConfig(storedConfig),
    concurrencyPolicy: concurrencyPolicyFromStoredConfig(storedConfig),
    definition,
    members: [...coreMembers, ...extraMembers],
    coreMembers,
    extraMembers,
  };
}

export async function loadDefaultReviewTeam(
  workspacePath?: string,
): Promise<ReviewTeam> {
  const [definition, storedConfig, subagents, rawModels] = await Promise.all([
    loadDefaultReviewTeamDefinition(),
    loadDefaultReviewTeamConfig(),
    SubagentAPI.listSubagents({ workspacePath }),
    configAPI.getConfig('ai.models').catch(() => undefined),
  ]);

  return resolveDefaultReviewTeam(subagents, storedConfig, {
    definition,
    availableModelIds: extractAvailableModelIds(rawModels),
  });
}

interface ReviewTeamLaunchOptions {
  target?: ReviewTargetClassification;
  reviewTargetFilePaths?: string[];
}

interface ReviewTeamManifestOptions {
  workspacePath?: string;
  policySource?: ReviewTeamRunManifest['policySource'];
  target?: ReviewTargetClassification;
  changeStats?: Partial<ReviewTeamChangeStats>;
  tokenBudgetMode?: ReviewTokenBudgetMode;
  concurrencyPolicy?: Partial<ReviewTeamConcurrencyPolicy>;
  rateLimitStatus?: ReviewTeamRateLimitStatus | null;
  strategyOverride?: ReviewStrategyLevel;
  reviewTargetFilePaths?: string[];
}

function hasExplicitReviewTarget(filePaths?: string[]): boolean {
  return Boolean(filePaths?.some((filePath) => filePath.trim().length > 0));
}

function resolveReviewTargetForOptions(
  target: ReviewTargetClassification | undefined,
  reviewTargetFilePaths: string[] | undefined,
  fallbackSource: Parameters<typeof createUnknownReviewTargetClassification>[0],
): ReviewTargetClassification {
  if (target) {
    return target;
  }
  if (hasExplicitReviewTarget(reviewTargetFilePaths)) {
    return classifyReviewTargetFromFiles(reviewTargetFilePaths ?? [], 'session_files');
  }
  return createUnknownReviewTargetClassification(fallbackSource);
}

function isCoreMemberApplicableForLaunch(
  member: ReviewTeamMember,
  options: ReviewTeamLaunchOptions,
): boolean {
  return shouldRunCoreReviewerForTarget(
    member,
    resolveReviewTargetForOptions(
      options.target,
      options.reviewTargetFilePaths,
      'unknown',
    ),
  );
}

export async function prepareDefaultReviewTeamForLaunch(
  workspacePath?: string,
  options: ReviewTeamLaunchOptions = {},
): Promise<ReviewTeam> {
  const team = await loadDefaultReviewTeam(workspacePath);
  const missingCoreMembers = team.coreMembers.filter(
    (member) =>
      !member.available &&
      isCoreMemberApplicableForLaunch(member, options),
  );

  if (missingCoreMembers.length > 0) {
    throw new Error(
      `Required code review team members are unavailable: ${missingCoreMembers
        .map((member) => member.subagentId)
        .join(', ')}`,
    );
  }

  const coreMembersToEnable = team.coreMembers.filter(
    (member) =>
      member.available &&
      !member.enabled &&
      isCoreMemberApplicableForLaunch(member, options),
  );

  if (coreMembersToEnable.length > 0) {
    await Promise.all(
      coreMembersToEnable.map((member) =>
        SubagentAPI.updateSubagentConfig({
          subagentId: member.subagentId,
          enabled: true,
          workspacePath,
        }),
      ),
    );

    // Update local team state to reflect enabled status without re-fetching
    for (const member of team.members) {
      if (coreMembersToEnable.some((m) => m.subagentId === member.subagentId)) {
        member.enabled = true;
      }
    }
    for (const member of team.coreMembers) {
      if (coreMembersToEnable.some((m) => m.subagentId === member.subagentId)) {
        member.enabled = true;
      }
    }
  }

  return team;
}

function toManifestMember(
  member: ReviewTeamMember,
  reason?: ReviewTeamManifestMember['reason'],
): ReviewTeamManifestMember {
  const strategyProfile = getReviewStrategyProfile(member.strategyLevel);
  const roleDirective =
    strategyProfile.roleDirectives[member.subagentId as ReviewRoleDirectiveKey];
  return {
    subagentId: member.subagentId,
    displayName: member.displayName,
    roleName: member.roleName,
    model: member.model || DEFAULT_REVIEW_TEAM_MODEL,
    configuredModel: member.configuredModel || member.model || DEFAULT_REVIEW_TEAM_MODEL,
    modelFallbackReason: member.modelFallbackReason,
    defaultModelSlot: member.defaultModelSlot ?? strategyProfile.defaultModelSlot,
    strategyLevel: member.strategyLevel,
    strategySource: member.strategySource,
    strategyDirective:
      member.strategyDirective || roleDirective || strategyProfile.promptDirective,
    locked: member.locked,
    source: member.source,
    subagentSource: member.subagentSource,
    ...(reason ? { reason } : {}),
  };
}

function resolveManifestMemberModelForStrategy(
  member: ReviewTeamMember,
  strategyLevel: ReviewStrategyLevel,
): {
  model: string;
  configuredModel: string;
  modelFallbackReason?: ReviewModelFallbackReason;
} {
  if (member.modelFallbackReason === 'model_removed') {
    return {
      model: getReviewStrategyProfile(strategyLevel).defaultModelSlot,
      configuredModel: member.configuredModel,
      modelFallbackReason: member.modelFallbackReason,
    };
  }

  return resolveMemberModel(
    member.configuredModel || member.model || DEFAULT_REVIEW_TEAM_MODEL,
    strategyLevel,
  );
}

function applyTeamStrategyOverrideToMember(
  member: ReviewTeamMember,
  strategyLevel: ReviewStrategyLevel,
): ReviewTeamMember {
  if (member.strategySource === 'member' || member.strategyLevel === strategyLevel) {
    return member;
  }

  const strategyProfile = getReviewStrategyProfile(strategyLevel);
  const model = resolveManifestMemberModelForStrategy(member, strategyLevel);
  return {
    ...member,
    model: model.model,
    configuredModel: model.configuredModel,
    modelFallbackReason: model.modelFallbackReason,
    strategyOverride: DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL,
    strategyLevel,
    strategySource: 'team',
    defaultModelSlot: strategyProfile.defaultModelSlot,
    strategyDirective:
      strategyProfile.roleDirectives[member.subagentId as ReviewRoleDirectiveKey] ||
      strategyProfile.promptDirective,
  };
}

function shouldRunCoreReviewerForTarget(
  member: ReviewTeamMember,
  target: ReviewTargetClassification,
): boolean {
  return shouldRunReviewerForTarget(member.subagentId, target);
}

function resolveMaxExtraReviewers(
  mode: ReviewTokenBudgetMode,
  eligibleExtraReviewerCount: number,
): number {
  if (mode === 'economy') {
    return 0;
  }
  return eligibleExtraReviewerCount;
}

function resolveChangeStats(
  target: ReviewTargetClassification,
  stats?: Partial<ReviewTeamChangeStats>,
): ReviewTeamChangeStats {
  const fileCount = Math.max(
    0,
    Math.floor(
      stats?.fileCount ??
        target.files.filter((file) => !file.excluded).length,
    ),
  );
  const totalLinesChanged =
    typeof stats?.totalLinesChanged === 'number' &&
    Number.isFinite(stats.totalLinesChanged)
      ? Math.max(0, Math.floor(stats.totalLinesChanged))
      : undefined;

  return {
    fileCount,
    ...(totalLinesChanged !== undefined ? { totalLinesChanged } : {}),
    lineCountSource:
      totalLinesChanged !== undefined
        ? stats?.lineCountSource ?? 'diff_stat'
        : 'unknown',
  };
}

const SECURITY_SENSITIVE_PATH_PATTERN =
  /(^|[/._-])(auth|oauth|crypto|security|permission|permissions|secret|secrets|token|tokens|credential|credentials)([/._-]|$)/;

function isSecuritySensitiveReviewPath(normalizedPath: string): boolean {
  return SECURITY_SENSITIVE_PATH_PATTERN.test(normalizedPath.toLowerCase());
}

function workspaceAreaForReviewPath(normalizedPath: string): string {
  const crateMatch = normalizedPath.match(/^src\/crates\/([^/]+)/);
  if (crateMatch) {
    return `crate:${crateMatch[1]}`;
  }

  const appMatch = normalizedPath.match(/^src\/apps\/([^/]+)/);
  if (appMatch) {
    return `app:${appMatch[1]}`;
  }

  if (normalizedPath.startsWith('src/web-ui/')) {
    return 'web-ui';
  }

  if (normalizedPath.startsWith('BitFun-Installer/')) {
    return 'installer';
  }

  const [root] = normalizedPath.split('/');
  return root || 'unknown';
}

function pluralize(count: number, singular: string): string {
  return `${count} ${singular}${count === 1 ? '' : 's'}`;
}

const PRE_REVIEW_SUMMARY_SAMPLE_FILE_LIMIT = 3;
const PRE_REVIEW_SUMMARY_AREA_LIMIT = 8;

function buildPreReviewSummary(
  target: ReviewTargetClassification,
  changeStats: ReviewTeamChangeStats,
): ReviewTeamPreReviewSummary {
  const includedFiles = target.files
    .filter((file) => !file.excluded)
    .map((file) => file.normalizedPath);
  const excludedFileCount = target.files.length - includedFiles.length;
  const allWorkspaceAreas = groupFilesByWorkspaceArea(includedFiles)
    .sort((a, b) => b.files.length - a.files.length || a.index - b.index);
  const workspaceAreas = allWorkspaceAreas
    .slice(0, PRE_REVIEW_SUMMARY_AREA_LIMIT)
    .map((area) => ({
      key: area.key,
      fileCount: area.files.length,
      sampleFiles: area.files.slice(0, PRE_REVIEW_SUMMARY_SAMPLE_FILE_LIMIT),
    }));
  const lineCount = changeStats.totalLinesChanged;
  const lineCountLabel =
    lineCount === undefined
      ? 'unknown changed lines'
      : `${lineCount} changed lines`;
  const areaLabel = workspaceAreas.length > 0
    ? workspaceAreas.map((area) => `${area.key} (${area.fileCount})`).join(', ')
    : 'no resolved workspace area';
  const targetTags = [...target.tags];
  const tagLabel = targetTags.filter((tag) => tag !== 'unknown').join(', ') || 'unknown';
  const omittedAreaCount = Math.max(
    0,
    allWorkspaceAreas.length - workspaceAreas.length,
  );
  const summaryParts = [
    `${pluralize(changeStats.fileCount, 'file')}, ${lineCountLabel} across ${pluralize(allWorkspaceAreas.length, 'workspace area')}: ${areaLabel}`,
    `tags: ${tagLabel}`,
    omittedAreaCount > 0 ? `${pluralize(omittedAreaCount, 'workspace area')} omitted from summary` : undefined,
  ].filter(Boolean);

  return {
    source: 'target_manifest',
    summary: summaryParts.join('; '),
    fileCount: changeStats.fileCount,
    excludedFileCount,
    ...(lineCount !== undefined ? { lineCount } : {}),
    lineCountSource: changeStats.lineCountSource,
    targetTags,
    workspaceAreas,
    warnings: target.warnings.map((warning) => warning.code),
  };
}

export function recommendReviewStrategyForTarget(
  target: ReviewTargetClassification,
  changeStats: ReviewTeamChangeStats,
): ReviewTeamStrategyRecommendation {
  const includedFiles = target.files.filter((file) => !file.excluded);
  const securityFileCount = includedFiles.filter((file) =>
    isSecuritySensitiveReviewPath(file.normalizedPath),
  ).length;
  const workspaceAreaCount = new Set(
    includedFiles.map((file) => workspaceAreaForReviewPath(file.normalizedPath)),
  ).size;
  const contractSurfaceChanged = target.tags.includes('frontend_contract') ||
    target.tags.includes('desktop_contract') ||
    target.tags.includes('web_server_contract') ||
    target.tags.includes('api_layer') ||
    target.tags.includes('transport');
  const totalLinesChanged = changeStats.totalLinesChanged;
  const factors: ReviewTeamRiskFactors = {
    fileCount: changeStats.fileCount,
    ...(totalLinesChanged !== undefined ? { totalLinesChanged } : {}),
    lineCountSource: changeStats.lineCountSource,
    securityFileCount,
    workspaceAreaCount,
    contractSurfaceChanged,
  };

  if (target.resolution === 'unknown' || changeStats.fileCount === 0) {
    return {
      strategyLevel: 'normal',
      score: 0,
      rationale: 'unresolved target; keep a conservative normal review recommendation.',
      factors,
    };
  }

  const lineScore =
    totalLinesChanged === undefined
      ? 0
      : Math.floor(totalLinesChanged / 100);
  const crossAreaScore = Math.max(0, workspaceAreaCount - 1) * 2;
  const score =
    changeStats.fileCount +
    lineScore +
    securityFileCount * 3 +
    crossAreaScore +
    (contractSurfaceChanged ? 2 : 0);
  const strategyLevel: ReviewStrategyLevel =
    score <= 5
      ? 'quick'
      : score <= 20
        ? 'normal'
        : 'deep';
  const sizeLabel = totalLinesChanged === undefined
    ? `${changeStats.fileCount} files, unknown lines`
    : `${changeStats.fileCount} files, ${totalLinesChanged} lines`;
  const riskDetails = [
    pluralize(securityFileCount, 'security-sensitive file'),
    pluralize(workspaceAreaCount, 'workspace area'),
    contractSurfaceChanged ? 'contract surface changed' : undefined,
  ].filter(Boolean).join(', ');
  const rationale =
    strategyLevel === 'quick'
      ? `Small change (${sizeLabel}). Quick scan sufficient.`
      : strategyLevel === 'normal'
        ? `Medium change (${sizeLabel}; ${riskDetails}). Standard review recommended.`
        : `Large/high-risk change (${sizeLabel}; ${riskDetails}). Deep review recommended.`;

  return {
    strategyLevel,
    score,
    rationale,
    factors,
  };
}

const REVIEW_STRATEGY_RANK: Record<ReviewStrategyLevel, number> = {
  quick: 0,
  normal: 1,
  deep: 2,
};

function crossCrateChangeCountForReviewTarget(
  target: ReviewTargetClassification,
): number {
  const crateNames = new Set(
    target.files
      .filter((file) => !file.excluded)
      .map((file) => /^src\/crates\/([^/]+)/.exec(file.normalizedPath)?.[1])
      .filter((crateName): crateName is string => Boolean(crateName)),
  );

  return Math.max(0, crateNames.size - 1);
}

function buildBackendCompatibleRiskFactors(
  target: ReviewTargetClassification,
  changeStats: ReviewTeamChangeStats,
): ReviewTeamBackendRiskFactors {
  const includedFiles = target.files.filter((file) => !file.excluded);

  return {
    fileCount: changeStats.fileCount,
    totalLinesChanged: changeStats.totalLinesChanged ?? 0,
    lineCountSource: changeStats.lineCountSource,
    filesInSecurityPaths: includedFiles.filter((file) =>
      isSecuritySensitiveReviewPath(file.normalizedPath),
    ).length,
    crossCrateChanges: crossCrateChangeCountForReviewTarget(target),
    maxCyclomaticComplexityDelta: 0,
    maxCyclomaticComplexityDeltaSource: 'not_measured',
  };
}

function recommendBackendCompatibleStrategyForTarget(
  target: ReviewTargetClassification,
  changeStats: ReviewTeamChangeStats,
): ReviewTeamBackendStrategyRecommendation {
  const factors = buildBackendCompatibleRiskFactors(target, changeStats);
  const score =
    factors.fileCount +
    Math.floor(factors.totalLinesChanged / 100) +
    factors.filesInSecurityPaths * 3 +
    factors.crossCrateChanges * 2;
  const strategyLevel: ReviewStrategyLevel =
    score <= 5
      ? 'quick'
      : score <= 20
        ? 'normal'
        : 'deep';
  const rationale =
    strategyLevel === 'quick'
      ? `Backend-compatible policy sees a small change (${factors.fileCount} files, ${factors.totalLinesChanged} lines).`
      : strategyLevel === 'normal'
        ? `Backend-compatible policy sees a medium change (${factors.fileCount} files, ${factors.totalLinesChanged} lines).`
        : `Backend-compatible policy sees a large/high-risk change (${factors.fileCount} files, ${factors.totalLinesChanged} lines, ${factors.filesInSecurityPaths} security files).`;

  return {
    strategyLevel,
    score,
    rationale,
    factors,
  };
}

function resolveStrategyMismatchSeverity(params: {
  finalStrategy: ReviewStrategyLevel;
  frontendRecommendation: ReviewStrategyLevel;
  backendRecommendation: ReviewStrategyLevel;
}): ReviewTeamStrategyMismatchSeverity {
  const finalRank = REVIEW_STRATEGY_RANK[params.finalStrategy];
  const recommendedRank = Math.max(
    REVIEW_STRATEGY_RANK[params.frontendRecommendation],
    REVIEW_STRATEGY_RANK[params.backendRecommendation],
  );
  const distance = Math.abs(finalRank - recommendedRank);

  if (distance === 0) {
    return 'none';
  }
  if (distance >= 2) {
    return 'high';
  }
  return finalRank < recommendedRank ? 'medium' : 'low';
}

function buildReviewStrategyDecision(params: {
  teamDefaultStrategy: ReviewStrategyLevel;
  finalStrategy: ReviewStrategyLevel;
  userOverride?: ReviewStrategyLevel;
  frontendRecommendation: ReviewTeamStrategyRecommendation;
  backendRecommendation: ReviewTeamBackendStrategyRecommendation;
}): ReviewTeamStrategyDecision {
  const mismatch =
    params.finalStrategy !== params.frontendRecommendation.strategyLevel ||
    params.finalStrategy !== params.backendRecommendation.strategyLevel;
  const mismatchSeverity = resolveStrategyMismatchSeverity({
    finalStrategy: params.finalStrategy,
    frontendRecommendation: params.frontendRecommendation.strategyLevel,
    backendRecommendation: params.backendRecommendation.strategyLevel,
  });
  const recommendationSummary = [
    `frontend=${params.frontendRecommendation.strategyLevel}`,
    `backend=${params.backendRecommendation.strategyLevel}`,
  ].join(', ');

  return {
    authority: 'mismatch_warning',
    teamDefaultStrategy: params.teamDefaultStrategy,
    ...(params.userOverride ? { userOverride: params.userOverride } : {}),
    finalStrategy: params.finalStrategy,
    frontendRecommendation: params.frontendRecommendation,
    backendRecommendation: params.backendRecommendation,
    mismatch,
    mismatchSeverity,
    rationale: mismatch
      ? `Final strategy ${params.finalStrategy} differs from advisory recommendations (${recommendationSummary}); keep this as non-blocking launch/report metadata.`
      : `Final strategy ${params.finalStrategy} matches advisory recommendations (${recommendationSummary}).`,
  };
}

function buildWorkPacketScopeFromFiles(
  target: ReviewTargetClassification,
  files: string[],
  group?: { index: number; count: number },
): ReviewTeamWorkPacketScope {
  return {
    kind: 'review_target',
    targetSource: target.source,
    targetResolution: target.resolution,
    targetTags: [...target.tags],
    fileCount: files.length,
    files,
    excludedFileCount:
      target.files.length - target.files.filter((file) => !file.excluded).length,
    ...(group ? { groupIndex: group.index, groupCount: group.count } : {}),
  };
}

function buildWorkPacket(params: {
  member: ReviewTeamMember;
  phase: ReviewTeamWorkPacket['phase'];
  launchBatch: number;
  scope: ReviewTeamWorkPacketScope;
  timeoutSeconds: number;
}): ReviewTeamWorkPacket {
  const manifestMember = toManifestMember(params.member);
  const packetGroupSuffix =
    params.phase === 'reviewer' &&
    params.scope.groupIndex !== undefined &&
    params.scope.groupCount !== undefined
      ? `:group-${params.scope.groupIndex}-of-${params.scope.groupCount}`
      : '';

  return {
    packetId: `${params.phase}:${manifestMember.subagentId}${packetGroupSuffix}`,
    phase: params.phase,
    launchBatch: params.launchBatch,
    subagentId: manifestMember.subagentId,
    displayName: manifestMember.displayName,
    roleName: manifestMember.roleName,
    assignedScope: params.scope,
    allowedTools: [...params.member.allowedTools],
    timeoutSeconds: params.timeoutSeconds,
    requiredOutputFields:
      params.phase === 'judge'
        ? [...JUDGE_WORK_PACKET_REQUIRED_OUTPUT_FIELDS]
        : [...REVIEWER_WORK_PACKET_REQUIRED_OUTPUT_FIELDS],
    strategyLevel: manifestMember.strategyLevel,
    strategyDirective: manifestMember.strategyDirective,
    model: manifestMember.model || DEFAULT_REVIEW_TEAM_MODEL,
  };
}

function splitFilesIntoGroups(files: string[], groupCount: number): string[][] {
  if (groupCount <= 1) {
    return [files];
  }

  const groups: string[][] = [];
  let cursor = 0;
  for (let index = 0; index < groupCount; index += 1) {
    const remainingFiles = files.length - cursor;
    const remainingGroups = groupCount - index;
    const groupSize = Math.ceil(remainingFiles / remainingGroups);
    groups.push(files.slice(cursor, cursor + groupSize));
    cursor += groupSize;
  }
  return groups;
}

interface WorkspaceAreaFileBucket {
  key: string;
  index: number;
  files: string[];
}

function groupFilesByWorkspaceArea(files: string[]): WorkspaceAreaFileBucket[] {
  const buckets: WorkspaceAreaFileBucket[] = [];
  const bucketByKey = new Map<string, WorkspaceAreaFileBucket>();

  for (const file of files) {
    const key = workspaceAreaForReviewPath(file);
    let bucket = bucketByKey.get(key);
    if (!bucket) {
      bucket = {
        key,
        index: buckets.length,
        files: [],
      };
      buckets.push(bucket);
      bucketByKey.set(key, bucket);
    }
    bucket.files.push(file);
  }

  return buckets;
}

function splitFilesIntoModuleAwareGroups(
  files: string[],
  groupCount: number,
): string[][] {
  if (groupCount <= 1) {
    return [files];
  }

  const buckets = groupFilesByWorkspaceArea(files);
  if (buckets.length <= 1) {
    return splitFilesIntoGroups(files, groupCount);
  }

  if (buckets.length >= groupCount) {
    const groups = Array.from({ length: groupCount }, () => [] as string[]);
    const sortedBuckets = [...buckets].sort(
      (a, b) => b.files.length - a.files.length || a.index - b.index,
    );

    for (const bucket of sortedBuckets) {
      let targetIndex = 0;
      for (let index = 1; index < groups.length; index += 1) {
        if (groups[index].length < groups[targetIndex].length) {
          targetIndex = index;
        }
      }
      groups[targetIndex].push(...bucket.files);
    }

    return groups.filter((group) => group.length > 0);
  }

  const chunkCounts = buckets.map(() => 1);
  let remainingChunks = groupCount - buckets.length;
  while (remainingChunks > 0) {
    let targetBucketIndex = -1;
    let largestAverageChunkSize = 0;

    for (let index = 0; index < buckets.length; index += 1) {
      if (chunkCounts[index] >= buckets[index].files.length) {
        continue;
      }
      const averageChunkSize = buckets[index].files.length / chunkCounts[index];
      if (averageChunkSize > largestAverageChunkSize) {
        largestAverageChunkSize = averageChunkSize;
        targetBucketIndex = index;
      }
    }

    if (targetBucketIndex === -1) {
      break;
    }

    chunkCounts[targetBucketIndex] += 1;
    remainingChunks -= 1;
  }

  return buckets.flatMap((bucket, index) =>
    splitFilesIntoGroups(bucket.files, chunkCounts[index]),
  );
}

function effectiveMaxSameRoleInstances(params: {
  executionPolicy: ReviewTeamExecutionPolicy;
  concurrencyPolicy: ReviewTeamConcurrencyPolicy;
  reviewerMemberCount: number;
}): number {
  const reviewerMemberCount = Math.max(1, params.reviewerMemberCount);
  const maxPerRole = Math.floor(
    params.concurrencyPolicy.maxParallelInstances / reviewerMemberCount,
  );

  return Math.max(
    1,
    Math.min(params.executionPolicy.maxSameRoleInstances, Math.max(1, maxPerRole)),
  );
}

function resolveReviewerPacketScopes(
  target: ReviewTargetClassification,
  executionPolicy: ReviewTeamExecutionPolicy,
  concurrencyPolicy: ReviewTeamConcurrencyPolicy,
  reviewerMemberCount: number,
): ReviewTeamWorkPacketScope[] {
  const includedFiles = target.files
    .filter((file) => !file.excluded)
    .map((file) => file.normalizedPath);
  const shouldSplit =
    executionPolicy.reviewerFileSplitThreshold > 0 &&
    executionPolicy.maxSameRoleInstances > 1 &&
    includedFiles.length > executionPolicy.reviewerFileSplitThreshold;

  if (!shouldSplit) {
    return [buildWorkPacketScopeFromFiles(target, includedFiles)];
  }

  const maxSameRoleInstances = effectiveMaxSameRoleInstances({
    executionPolicy,
    concurrencyPolicy,
    reviewerMemberCount,
  });
  const groupCount = Math.min(
    maxSameRoleInstances,
    Math.ceil(includedFiles.length / executionPolicy.reviewerFileSplitThreshold),
  );
  if (groupCount <= 1) {
    return [buildWorkPacketScopeFromFiles(target, includedFiles)];
  }

  const fileGroups = splitFilesIntoModuleAwareGroups(includedFiles, groupCount);
  return fileGroups.map((files, index) =>
    buildWorkPacketScopeFromFiles(target, files, {
      index: index + 1,
      count: fileGroups.length,
    }),
  );
}

function buildWorkPackets(params: {
  reviewerMembers: ReviewTeamMember[];
  judgeMember?: ReviewTeamMember;
  target: ReviewTargetClassification;
  executionPolicy: ReviewTeamExecutionPolicy;
  concurrencyPolicy: ReviewTeamConcurrencyPolicy;
}): ReviewTeamWorkPacket[] {
  const reviewerScopes = resolveReviewerPacketScopes(
    params.target,
    params.executionPolicy,
    params.concurrencyPolicy,
    params.reviewerMembers.length,
  );
  const fullScope = buildWorkPacketScopeFromFiles(
    params.target,
    params.target.files
      .filter((file) => !file.excluded)
      .map((file) => file.normalizedPath),
  );
  const reviewerSeeds = params.reviewerMembers.flatMap((member) =>
    reviewerScopes.map((scope) => ({ member, scope })),
  );
  const orderedReviewerSeeds = params.concurrencyPolicy.batchExtrasSeparately
    ? [
      ...reviewerSeeds.filter((seed) => seed.member.source === 'core'),
      ...reviewerSeeds.filter((seed) => seed.member.source === 'extra'),
    ]
    : reviewerSeeds;
  const reviewerPackets = orderedReviewerSeeds.map((seed, index) =>
    buildWorkPacket({
      member: seed.member,
      phase: 'reviewer',
      launchBatch:
        Math.floor(index / params.concurrencyPolicy.maxParallelInstances) + 1,
      scope: seed.scope,
      timeoutSeconds: params.executionPolicy.reviewerTimeoutSeconds,
    }),
  );
  const finalReviewerBatch = reviewerPackets.reduce(
    (maxBatch, packet) => Math.max(maxBatch, packet.launchBatch),
    0,
  );
  const judgePacket = params.judgeMember
    ? [
      buildWorkPacket({
        member: params.judgeMember,
        phase: 'judge',
        launchBatch: finalReviewerBatch + 1,
        scope: fullScope,
        timeoutSeconds: params.executionPolicy.judgeTimeoutSeconds,
      }),
    ]
    : [];

  return [...reviewerPackets, ...judgePacket];
}

const SHARED_CONTEXT_CACHE_ENTRY_LIMIT = 80;
const SHARED_CONTEXT_CACHE_RECOMMENDED_TOOLS: ReviewTeamSharedContextTool[] = [
  'GetFileDiff',
  'Read',
];

function buildSharedContextCachePlan(
  workPackets: ReviewTeamWorkPacket[] = [],
): ReviewTeamSharedContextCachePlan {
  const fileContextByPath = new Map<
    string,
    {
      path: string;
      workspaceArea: string;
      consumerPacketIds: string[];
      firstSeenIndex: number;
    }
  >();
  let nextSeenIndex = 0;

  for (const packet of workPackets) {
    if (packet.phase !== 'reviewer') {
      continue;
    }

    for (const path of packet.assignedScope.files) {
      let entry = fileContextByPath.get(path);
      if (!entry) {
        entry = {
          path,
          workspaceArea: workspaceAreaForReviewPath(path),
          consumerPacketIds: [],
          firstSeenIndex: nextSeenIndex,
        };
        nextSeenIndex += 1;
        fileContextByPath.set(path, entry);
      }
      if (!entry.consumerPacketIds.includes(packet.packetId)) {
        entry.consumerPacketIds.push(packet.packetId);
      }
    }
  }

  const repeatedFileContexts = Array.from(fileContextByPath.values())
    .filter((entry) => entry.consumerPacketIds.length > 1)
    .sort((a, b) => a.firstSeenIndex - b.firstSeenIndex);
  const entries = repeatedFileContexts
    .slice(0, SHARED_CONTEXT_CACHE_ENTRY_LIMIT)
    .map((entry, index) => ({
      cacheKey: `shared-context:${index + 1}`,
      path: entry.path,
      workspaceArea: entry.workspaceArea,
      recommendedTools: [...SHARED_CONTEXT_CACHE_RECOMMENDED_TOOLS],
      consumerPacketIds: entry.consumerPacketIds,
    }));

  return {
    source: 'work_packets',
    strategy: 'reuse_readonly_file_context_by_cache_key',
    entries,
    omittedEntryCount: Math.max(
      0,
      repeatedFileContexts.length - SHARED_CONTEXT_CACHE_ENTRY_LIMIT,
    ),
  };
}

const INCREMENTAL_REVIEW_CACHE_INVALIDATIONS: ReviewTeamIncrementalReviewCacheInvalidation[] = [
  'target_file_set_changed',
  'target_line_count_changed',
  'target_tag_changed',
  'target_warning_changed',
  'reviewer_roster_changed',
  'strategy_changed',
];

function stableFingerprint(input: unknown): string {
  const serialized = JSON.stringify(input);
  let hash = 0x811c9dc5;
  for (let index = 0; index < serialized.length; index += 1) {
    hash ^= serialized.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}

function buildIncrementalReviewCachePlan(params: {
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
  strategyLevel: ReviewStrategyLevel;
  workPackets: ReviewTeamWorkPacket[];
}): ReviewTeamIncrementalReviewCachePlan {
  const filePaths = params.target.files
    .filter((file) => !file.excluded)
    .map((file) => file.normalizedPath)
    .sort((a, b) => a.localeCompare(b));
  const workspaceAreas = Array.from(
    new Set(filePaths.map((file) => workspaceAreaForReviewPath(file))),
  ).sort((a, b) => a.localeCompare(b));
  const targetTags = [...params.target.tags].sort((a, b) => a.localeCompare(b));
  const targetWarnings = params.target.warnings
    .map((warning) => warning.code)
    .sort((a, b) => a.localeCompare(b));
  const reviewerPacketIds = params.workPackets
    .filter((packet) => packet.phase === 'reviewer')
    .map((packet) => packet.packetId)
    .sort((a, b) => a.localeCompare(b));
  const fingerprint = stableFingerprint({
    source: params.target.source,
    resolution: params.target.resolution,
    filePaths,
    workspaceAreas,
    targetTags,
    targetWarnings,
    lineCount: params.changeStats.totalLinesChanged ?? null,
    lineCountSource: params.changeStats.lineCountSource,
    reviewerPacketIds,
    strategyLevel: params.strategyLevel,
  });

  return {
    source: 'target_manifest',
    strategy: 'reuse_completed_packets_when_fingerprint_matches',
    cacheKey: `incremental-review:${fingerprint}`,
    fingerprint,
    filePaths,
    workspaceAreas,
    targetTags,
    reviewerPacketIds,
    ...(params.changeStats.totalLinesChanged !== undefined
      ? { lineCount: params.changeStats.totalLinesChanged }
      : {}),
    lineCountSource: params.changeStats.lineCountSource,
    invalidatesOn: [...INCREMENTAL_REVIEW_CACHE_INVALIDATIONS],
  };
}

function predictTimeoutSeconds(params: {
  role: 'reviewer' | 'judge';
  strategyLevel: ReviewStrategyLevel;
  changeStats: ReviewTeamChangeStats;
  reviewerCount: number;
}): number {
  const totalLinesChanged = params.changeStats.totalLinesChanged ?? 0;
  const base = PREDICTIVE_TIMEOUT_BASE_SECONDS[params.strategyLevel];
  const raw =
    base +
    params.changeStats.fileCount * PREDICTIVE_TIMEOUT_PER_FILE_SECONDS +
    Math.floor(totalLinesChanged / 100) *
      PREDICTIVE_TIMEOUT_PER_100_LINES_SECONDS;
  const reviewerCount = Math.max(1, params.reviewerCount);
  const multiplier =
    params.role === 'judge'
      ? 1 + Math.floor((reviewerCount - 1) / 3)
      : 1;

  return Math.min(raw * multiplier, MAX_PREDICTIVE_TIMEOUT_SECONDS);
}

function buildEffectiveExecutionPolicy(params: {
  basePolicy: ReviewTeamExecutionPolicy;
  strategyLevel: ReviewStrategyLevel;
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
  reviewerCount: number;
}): ReviewTeamExecutionPolicy {
  if (
    params.target.resolution === 'unknown' &&
    params.changeStats.fileCount === 0 &&
    params.changeStats.totalLinesChanged === undefined
  ) {
    return params.basePolicy;
  }

  const reviewerTimeoutSeconds = predictTimeoutSeconds({
    role: 'reviewer',
    strategyLevel: params.strategyLevel,
    changeStats: params.changeStats,
    reviewerCount: params.reviewerCount,
  });
  const judgeTimeoutSeconds = predictTimeoutSeconds({
    role: 'judge',
    strategyLevel: params.strategyLevel,
    changeStats: params.changeStats,
    reviewerCount: params.reviewerCount,
  });

  return {
    ...params.basePolicy,
    reviewerTimeoutSeconds:
      params.basePolicy.reviewerTimeoutSeconds === 0
        ? 0
        : Math.max(
          params.basePolicy.reviewerTimeoutSeconds,
          reviewerTimeoutSeconds,
        ),
    judgeTimeoutSeconds:
      params.basePolicy.judgeTimeoutSeconds === 0
        ? 0
        : Math.max(
          params.basePolicy.judgeTimeoutSeconds,
          judgeTimeoutSeconds,
        ),
  };
}

function estimateChangedLinesForScope(params: {
  scope: ReviewTeamWorkPacketScope;
  changeStats: ReviewTeamChangeStats;
  totalIncludedFileCount: number;
}): number {
  if (params.changeStats.totalLinesChanged === undefined) {
    return params.scope.fileCount * PROMPT_BYTE_ESTIMATE_UNKNOWN_LINES_PER_FILE;
  }

  if (params.totalIncludedFileCount <= 0) {
    return params.changeStats.totalLinesChanged;
  }

  return Math.ceil(
    params.changeStats.totalLinesChanged *
      (params.scope.fileCount / params.totalIncludedFileCount),
  );
}

function estimateReviewerPromptBytes(params: {
  packet: ReviewTeamWorkPacket;
  changeStats: ReviewTeamChangeStats;
  totalIncludedFileCount: number;
}): number {
  const pathBytes = params.packet.assignedScope.files.reduce(
    (total, filePath) => total + filePath.length + 1,
    0,
  );
  const estimatedChangedLines = estimateChangedLinesForScope({
    scope: params.packet.assignedScope,
    changeStats: params.changeStats,
    totalIncludedFileCount: params.totalIncludedFileCount,
  });

  return Math.ceil(
    PROMPT_BYTE_ESTIMATE_BASE_BYTES +
      pathBytes +
      params.packet.assignedScope.fileCount * PROMPT_BYTE_ESTIMATE_PER_FILE_BYTES +
      estimatedChangedLines * PROMPT_BYTE_ESTIMATE_PER_CHANGED_LINE_BYTES,
  );
}

function estimateMaxReviewerPromptBytes(params: {
  workPackets: ReviewTeamWorkPacket[];
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
}): number {
  const reviewerPackets = params.workPackets.filter(
    (packet) => packet.phase === 'reviewer',
  );
  const totalIncludedFileCount = params.target.files.filter(
    (file) => !file.excluded,
  ).length;

  if (reviewerPackets.length === 0) {
    return PROMPT_BYTE_ESTIMATE_BASE_BYTES;
  }

  return Math.max(
    ...reviewerPackets.map((packet) =>
      estimateReviewerPromptBytes({
        packet,
        changeStats: params.changeStats,
        totalIncludedFileCount,
      }),
    ),
  );
}

function buildTokenBudgetPlan(params: {
  mode: ReviewTokenBudgetMode;
  activeReviewerCalls: number;
  eligibleExtraReviewerCount: number;
  maxExtraReviewers: number;
  skippedReviewerIds: string[];
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
  executionPolicy: ReviewTeamExecutionPolicy;
  workPackets: ReviewTeamWorkPacket[];
}): ReviewTeamTokenBudgetPlan {
  const includedFileCount = params.target.files.filter(
    (file) => !file.excluded,
  ).length;
  const fileSplitGuardrailActive =
    params.executionPolicy.reviewerFileSplitThreshold > 0 &&
    includedFileCount > params.executionPolicy.reviewerFileSplitThreshold;
  const maxPromptBytesPerReviewer =
    TOKEN_BUDGET_PROMPT_BYTE_LIMIT_BY_MODE[params.mode];
  const estimatedPromptBytesPerReviewer = estimateMaxReviewerPromptBytes({
    workPackets: params.workPackets,
    target: params.target,
    changeStats: params.changeStats,
  });
  const promptByteLimitExceeded =
    estimatedPromptBytesPerReviewer > maxPromptBytesPerReviewer;
  const largeDiffSummaryFirst = promptByteLimitExceeded;
  const decisions: ReviewTeamTokenBudgetDecision[] = [];
  const warnings: string[] = [];

  if (promptByteLimitExceeded) {
    decisions.push({
      kind: 'summary_first_full_scope',
      reason: 'prompt_bytes_exceeded',
      detail:
        `Estimated reviewer prompt ${estimatedPromptBytesPerReviewer} bytes exceeds ${maxPromptBytesPerReviewer} bytes for ${params.mode} budget; use summary-first while keeping every assigned_scope file visible.`,
    });
    warnings.push(
      'Estimated reviewer prompt exceeds the selected token budget; use summary-first without hiding assigned files.',
    );
  }

  if (params.skippedReviewerIds.length > 0) {
    decisions.push({
      kind: 'skip_extra_reviewers',
      reason: 'extra_reviewers_skipped',
      detail:
        'Some extra reviewers were skipped by the selected token budget mode.',
      affectedReviewerIds: [...params.skippedReviewerIds],
    });
    warnings.push(
      'Some extra reviewers were skipped by the selected token budget mode.',
    );
  }

  return {
    mode: params.mode,
    estimatedReviewerCalls: params.activeReviewerCalls,
    maxReviewerCalls:
      params.activeReviewerCalls +
      Math.max(0, params.eligibleExtraReviewerCount - params.maxExtraReviewers),
    maxExtraReviewers: params.maxExtraReviewers,
    ...(fileSplitGuardrailActive
      ? { maxFilesPerReviewer: params.executionPolicy.reviewerFileSplitThreshold }
      : {}),
    maxPromptBytesPerReviewer,
    estimatedPromptBytesPerReviewer,
    promptByteEstimateSource: 'manifest_heuristic',
    promptByteLimitExceeded,
    largeDiffSummaryFirst,
    decisions,
    skippedReviewerIds: params.skippedReviewerIds,
    warnings,
  };
}

export function buildEffectiveReviewTeamManifest(
  team: ReviewTeam,
  options: ReviewTeamManifestOptions = {},
): ReviewTeamRunManifest {
  const target = resolveReviewTargetForOptions(
    options.target,
    options.reviewTargetFilePaths,
    'unknown',
  );
  const tokenBudgetMode = options.tokenBudgetMode ?? 'balanced';
  const changeStats = resolveChangeStats(target, options.changeStats);
  const baseConcurrencyPolicy = normalizeConcurrencyPolicy(team.concurrencyPolicy);
  const concurrencyPolicy = applyRateLimitToConcurrencyPolicy(
    normalizeConcurrencyPolicy({
      ...baseConcurrencyPolicy,
      ...options.concurrencyPolicy,
    }),
    options.rateLimitStatus,
  );
  const strategyLevel = options.strategyOverride ?? team.strategyLevel;
  const strategyRecommendation = recommendReviewStrategyForTarget(target, changeStats);
  const backendStrategyRecommendation = recommendBackendCompatibleStrategyForTarget(
    target,
    changeStats,
  );
  const strategyDecision = buildReviewStrategyDecision({
    teamDefaultStrategy: team.strategyLevel,
    finalStrategy: strategyLevel,
    ...(options.strategyOverride ? { userOverride: options.strategyOverride } : {}),
    frontendRecommendation: strategyRecommendation,
    backendRecommendation: backendStrategyRecommendation,
  });
  const preReviewSummary = buildPreReviewSummary(target, changeStats);
  const coreMembers = team.coreMembers.map((member) =>
    applyTeamStrategyOverrideToMember(member, strategyLevel),
  );
  const extraMembers = team.extraMembers.map((member) =>
    applyTeamStrategyOverrideToMember(member, strategyLevel),
  );
  const availableCoreMembers = coreMembers.filter((member) => member.available);
  const unavailableCoreMembers = coreMembers.filter((member) => !member.available);
  const notApplicableCoreMembers = availableCoreMembers.filter(
    (member) =>
      member.definitionKey !== 'judge' &&
      !shouldRunCoreReviewerForTarget(member, target),
  );
  const coreReviewerMembers = availableCoreMembers
    .filter((member) => member.definitionKey !== 'judge')
    .filter((member) => shouldRunCoreReviewerForTarget(member, target));
  const coreReviewers = coreReviewerMembers.map((member) => toManifestMember(member));
  const qualityGateReviewerMember = availableCoreMembers.find(
    (member) => member.definitionKey === 'judge',
  );
  const qualityGateReviewer = qualityGateReviewerMember
    ? toManifestMember(qualityGateReviewerMember)
    : undefined;
  const eligibleExtraMembers = extraMembers
    .filter((member) => member.available && member.enabled);
  const maxExtraReviewers = resolveMaxExtraReviewers(
    tokenBudgetMode,
    eligibleExtraMembers.length,
  );
  const enabledExtraMembers = eligibleExtraMembers.slice(0, maxExtraReviewers);
  const budgetLimitedExtraMembers = eligibleExtraMembers.slice(maxExtraReviewers);
  const enabledExtraReviewers = enabledExtraMembers
    .map((member) => toManifestMember(member));
  const reviewerCount = coreReviewers.length + enabledExtraReviewers.length;
  const executionPolicy = buildEffectiveExecutionPolicy({
    basePolicy: team.executionPolicy,
    strategyLevel,
    target,
    changeStats,
    reviewerCount,
  });
  const workPackets = buildWorkPackets({
    reviewerMembers: [...coreReviewerMembers, ...enabledExtraMembers],
    judgeMember: qualityGateReviewerMember,
    target,
    executionPolicy,
    concurrencyPolicy,
  });
  const sharedContextCache = buildSharedContextCachePlan(workPackets);
  const incrementalReviewCache = buildIncrementalReviewCachePlan({
    target,
    changeStats,
    strategyLevel,
    workPackets,
  });
  const tokenBudget = buildTokenBudgetPlan({
    mode: tokenBudgetMode,
    activeReviewerCalls: workPackets.length,
    eligibleExtraReviewerCount: eligibleExtraMembers.length,
    maxExtraReviewers,
    skippedReviewerIds: budgetLimitedExtraMembers.map((member) => member.subagentId),
    target,
    changeStats,
    executionPolicy,
    workPackets,
  });
  const skippedReviewers = [
    ...extraMembers
      .filter((member) => !member.available || !member.enabled)
      .map((member) =>
        toManifestMember(
          member,
          member.skipReason ?? (member.available ? 'disabled' : 'unavailable'),
        ),
      ),
    ...budgetLimitedExtraMembers.map((member) =>
      toManifestMember(member, 'budget_limited'),
    ),
    ...unavailableCoreMembers.map((member) =>
      toManifestMember(member, 'unavailable'),
    ),
    ...notApplicableCoreMembers.map((member) =>
      toManifestMember(member, 'not_applicable'),
    ),
  ];

  return {
    reviewMode: 'deep',
    ...(options.workspacePath ? { workspacePath: options.workspacePath } : {}),
    policySource: options.policySource ?? 'default-review-team-config',
    target,
    strategyLevel,
    strategyRecommendation,
    strategyDecision,
    executionPolicy,
    concurrencyPolicy,
    changeStats,
    preReviewSummary,
    sharedContextCache,
    incrementalReviewCache,
    tokenBudget,
    coreReviewers,
    ...(qualityGateReviewer ? { qualityGateReviewer } : {}),
    enabledExtraReviewers,
    skippedReviewers,
    workPackets,
  };
}

function formatResponsibilities(items: string[]): string {
  return items.map((item) => `    - ${item}`).join('\n');
}

function formatStrategyImpact(
  strategyLevel: ReviewStrategyLevel,
  strategyProfiles: Record<ReviewStrategyLevel, ReviewStrategyProfile> = REVIEW_STRATEGY_PROFILES,
): string {
  const definition = strategyProfiles[strategyLevel];
  return `Token/time impact: approximately ${definition.tokenImpact} token usage and ${definition.runtimeImpact} runtime.`;
}

function formatManifestList(
  members: ReviewTeamManifestMember[],
  emptyValue: string,
): string {
  if (members.length === 0) {
    return emptyValue;
  }

  return members
    .map((member) =>
      member.reason
        ? `${member.subagentId}: ${member.reason}`
        : member.subagentId,
    )
    .join(', ');
}

function workPacketToPromptPayload(packet: ReviewTeamWorkPacket) {
  return {
    packet_id: packet.packetId,
    phase: packet.phase,
    launch_batch: packet.launchBatch,
    subagent_type: packet.subagentId,
    display_name: packet.displayName,
    role: packet.roleName,
    assigned_scope: {
      kind: packet.assignedScope.kind,
      target_source: packet.assignedScope.targetSource,
      target_resolution: packet.assignedScope.targetResolution,
      target_tags: packet.assignedScope.targetTags,
      file_count: packet.assignedScope.fileCount,
      files: packet.assignedScope.files,
      excluded_file_count: packet.assignedScope.excludedFileCount,
      ...(packet.assignedScope.groupIndex !== undefined
        ? { group_index: packet.assignedScope.groupIndex }
        : {}),
      ...(packet.assignedScope.groupCount !== undefined
        ? { group_count: packet.assignedScope.groupCount }
        : {}),
    },
    allowed_tools: packet.allowedTools,
    timeout_seconds: packet.timeoutSeconds,
    required_output_fields: packet.requiredOutputFields,
    strategy: packet.strategyLevel,
    model_id: packet.model,
    prompt_directive: packet.strategyDirective,
  };
}

function formatWorkPacketBlock(workPackets: ReviewTeamWorkPacket[] = []): string {
  if (workPackets.length === 0) {
    return '- none';
  }

  return [
    '```json',
    JSON.stringify(workPackets.map(workPacketToPromptPayload), null, 2),
    '```',
  ].join('\n');
}

function formatPreReviewSummaryBlock(summary: ReviewTeamPreReviewSummary): string {
  return [
    'Pre-generated diff summary:',
    '```json',
    JSON.stringify(summary, null, 2),
    '```',
  ].join('\n');
}

function sharedContextCacheToPromptPayload(plan: ReviewTeamSharedContextCachePlan) {
  return {
    source: plan.source,
    strategy: plan.strategy,
    omitted_entry_count: plan.omittedEntryCount,
    entries: plan.entries.map((entry) => ({
      cache_key: entry.cacheKey,
      path: entry.path,
      workspace_area: entry.workspaceArea,
      recommended_tools: entry.recommendedTools,
      consumer_packet_ids: entry.consumerPacketIds,
    })),
  };
}

function formatSharedContextCacheBlock(plan: ReviewTeamSharedContextCachePlan): string {
  return [
    'Shared context cache plan:',
    '```json',
    JSON.stringify(sharedContextCacheToPromptPayload(plan), null, 2),
    '```',
  ].join('\n');
}

function incrementalReviewCacheToPromptPayload(plan: ReviewTeamIncrementalReviewCachePlan) {
  return {
    source: plan.source,
    strategy: plan.strategy,
    cache_key: plan.cacheKey,
    fingerprint: plan.fingerprint,
    file_paths: plan.filePaths,
    workspace_areas: plan.workspaceAreas,
    target_tags: plan.targetTags,
    reviewer_packet_ids: plan.reviewerPacketIds,
    ...(plan.lineCount !== undefined ? { line_count: plan.lineCount } : {}),
    line_count_source: plan.lineCountSource,
    invalidates_on: plan.invalidatesOn,
  };
}

function formatIncrementalReviewCacheBlock(plan: ReviewTeamIncrementalReviewCachePlan): string {
  return [
    'Incremental review cache plan:',
    '```json',
    JSON.stringify(incrementalReviewCacheToPromptPayload(plan), null, 2),
    '```',
  ].join('\n');
}

function formatTokenBudgetDecisionKinds(
  decisions: ReviewTeamTokenBudgetDecision[] = [],
): string {
  return decisions.length > 0
    ? decisions.map((decision) => decision.kind).join(', ')
    : 'none';
}

export function buildReviewTeamPromptBlock(
  team: ReviewTeam,
  manifest = buildEffectiveReviewTeamManifest(team),
): string {
  const activeSubagentIds = new Set([
    ...manifest.coreReviewers.map((member) => member.subagentId),
    ...manifest.enabledExtraReviewers.map((member) => member.subagentId),
    ...(manifest.qualityGateReviewer
      ? [manifest.qualityGateReviewer.subagentId]
      : []),
  ]);
  const activeManifestMembers = [
    ...manifest.coreReviewers,
    ...(manifest.qualityGateReviewer ? [manifest.qualityGateReviewer] : []),
    ...manifest.enabledExtraReviewers,
  ];
  const manifestMemberBySubagentId = new Map(
    activeManifestMembers.map((member) => [member.subagentId, member]),
  );
  const members = team.members
    .filter((member) => member.available && activeSubagentIds.has(member.subagentId))
    .map((member) => {
      const manifestMember =
        manifestMemberBySubagentId.get(member.subagentId) ?? toManifestMember(member);
      return [
        `- ${manifestMember.displayName}`,
        `  - subagent_type: ${manifestMember.subagentId}`,
        `  - preferred_task_label: ${manifestMember.displayName}`,
        `  - role: ${manifestMember.roleName}`,
        `  - locked_core_role: ${manifestMember.locked ? 'yes' : 'no'}`,
        `  - strategy: ${manifestMember.strategyLevel}`,
        `  - strategy_source: ${manifestMember.strategySource}`,
        `  - default_model_slot: ${manifestMember.defaultModelSlot}`,
        `  - model: ${manifestMember.model || DEFAULT_REVIEW_TEAM_MODEL}`,
        `  - model_id: ${manifestMember.model || DEFAULT_REVIEW_TEAM_MODEL}`,
        `  - configured_model: ${manifestMember.configuredModel || manifestMember.model || DEFAULT_REVIEW_TEAM_MODEL}`,
        ...(manifestMember.modelFallbackReason
          ? [`  - model_fallback: ${manifestMember.modelFallbackReason}`]
          : []),
        `  - prompt_directive: ${manifestMember.strategyDirective}`,
        '  - responsibilities:',
        formatResponsibilities(member.responsibilities),
      ].join('\n');
    })
    .join('\n');
  const executionPolicy = [
    `- reviewer_timeout_seconds: ${manifest.executionPolicy.reviewerTimeoutSeconds}`,
    `- judge_timeout_seconds: ${manifest.executionPolicy.judgeTimeoutSeconds}`,
    `- reviewer_file_split_threshold: ${manifest.executionPolicy.reviewerFileSplitThreshold}`,
    `- max_same_role_instances: ${manifest.executionPolicy.maxSameRoleInstances}`,
    `- max_retries_per_role: ${manifest.executionPolicy.maxRetriesPerRole}`,
  ].join('\n');
  const concurrencyPolicy = [
    `- max_parallel_instances: ${manifest.concurrencyPolicy.maxParallelInstances}`,
    `- stagger_seconds: ${manifest.concurrencyPolicy.staggerSeconds}`,
    `- max_queue_wait_seconds: ${manifest.concurrencyPolicy.maxQueueWaitSeconds}`,
    `- batch_extras_separately: ${manifest.concurrencyPolicy.batchExtrasSeparately ? 'yes' : 'no'}`,
    `- allow_provider_capacity_queue: ${manifest.concurrencyPolicy.allowProviderCapacityQueue ? 'yes' : 'no'}`,
    `- allow_bounded_auto_retry: ${manifest.concurrencyPolicy.allowBoundedAutoRetry ? 'yes' : 'no'}`,
    `- auto_retry_elapsed_guard_seconds: ${manifest.concurrencyPolicy.autoRetryElapsedGuardSeconds}`,
  ].join('\n');
  const targetLineCount =
    manifest.changeStats?.totalLinesChanged !== undefined
      ? `${manifest.changeStats.totalLinesChanged}`
      : 'unknown';
  const manifestBlock = [
    'Run manifest:',
    `- review_mode: ${manifest.reviewMode}`,
    `- team_strategy: ${manifest.strategyLevel}`,
    `- strategy_authority: ${manifest.strategyDecision.authority}`,
    `- final_strategy: ${manifest.strategyDecision.finalStrategy}`,
    `- frontend_recommended_strategy: ${manifest.strategyDecision.frontendRecommendation.strategyLevel}`,
    `- backend_recommended_strategy: ${manifest.strategyDecision.backendRecommendation.strategyLevel}`,
    `- strategy_user_override: ${manifest.strategyDecision.userOverride ?? 'none'}`,
    `- strategy_mismatch: ${manifest.strategyDecision.mismatch ? 'yes' : 'no'}`,
    `- strategy_mismatch_severity: ${manifest.strategyDecision.mismatchSeverity}`,
    `- max_cyclomatic_complexity_delta: ${manifest.strategyDecision.backendRecommendation.factors.maxCyclomaticComplexityDelta}`,
    `- max_cyclomatic_complexity_delta_source: ${manifest.strategyDecision.backendRecommendation.factors.maxCyclomaticComplexityDeltaSource}`,
    ...(manifest.strategyRecommendation
      ? [
        `- recommended_strategy: ${manifest.strategyRecommendation.strategyLevel}`,
        `- strategy_recommendation_score: ${manifest.strategyRecommendation.score}`,
        `- strategy_recommendation_rationale: ${manifest.strategyRecommendation.rationale}`,
      ]
      : []),
    `- workspace_path: ${manifest.workspacePath || 'inherited from current session'}`,
    `- policy_source: ${manifest.policySource}`,
    `- target_source: ${manifest.target.source}`,
    `- target_resolution: ${manifest.target.resolution}`,
    `- target_tags: ${manifest.target.tags.join(', ') || 'none'}`,
    `- target_warnings: ${manifest.target.warnings.map((warning) => warning.code).join(', ') || 'none'}`,
    `- target_file_count: ${manifest.changeStats?.fileCount ?? manifest.target.files.length}`,
    `- target_line_count: ${targetLineCount}`,
    `- target_line_count_source: ${manifest.changeStats?.lineCountSource ?? 'unknown'}`,
    `- token_budget_mode: ${manifest.tokenBudget.mode}`,
    `- estimated_reviewer_calls: ${manifest.tokenBudget.estimatedReviewerCalls}`,
    `- max_prompt_bytes_per_reviewer: ${manifest.tokenBudget.maxPromptBytesPerReviewer ?? 'none'}`,
    `- estimated_prompt_bytes_per_reviewer: ${manifest.tokenBudget.estimatedPromptBytesPerReviewer ?? 'unknown'}`,
    `- prompt_byte_estimate_source: ${manifest.tokenBudget.promptByteEstimateSource ?? 'none'}`,
    `- prompt_byte_limit_exceeded: ${manifest.tokenBudget.promptByteLimitExceeded ? 'yes' : 'no'}`,
    `- token_budget_decisions: ${formatTokenBudgetDecisionKinds(manifest.tokenBudget.decisions)}`,
    `- budget_limited_reviewers: ${manifest.tokenBudget.skippedReviewerIds.join(', ') || 'none'}`,
    `- core_reviewers: ${formatManifestList(manifest.coreReviewers, 'none')}`,
    `- quality_gate_reviewer: ${manifest.qualityGateReviewer?.subagentId || 'none'}`,
    `- enabled_extra_reviewers: ${formatManifestList(manifest.enabledExtraReviewers, 'none')}`,
    '- skipped_reviewers:',
    ...(manifest.skippedReviewers.length > 0
      ? manifest.skippedReviewers.map(
        (member) => `  - ${member.subagentId}: ${member.reason || 'skipped'}`,
      )
      : ['  - none']),
  ].join('\n');
  const strategyProfiles = team.definition?.strategyProfiles ?? REVIEW_STRATEGY_PROFILES;
  const strategyRules = REVIEW_STRATEGY_LEVELS.map((level) => {
    const definition = strategyProfiles[level];
    const roleEntries = Object.entries(definition.roleDirectives) as [ReviewRoleDirectiveKey, string][];
    const roleLines = roleEntries.map(
      ([role, directive]) => `    - ${role}: ${directive}`,
    );
    return [
      `- ${level}: ${definition.summary}`,
      `  - ${formatStrategyImpact(level, strategyProfiles)}`,
      `  - Default model slot: ${definition.defaultModelSlot}`,
      `  - Prompt directive (fallback): ${definition.promptDirective}`,
      `  - Role-specific directives:`,
      ...roleLines,
    ].join('\n');
  }).join('\n');
  const commonStrategyRules = REVIEW_STRATEGY_COMMON_RULES.reviewerPromptRules
    .map((rule) => `- ${rule}`)
    .join('\n');

  return [
    manifestBlock,
    formatPreReviewSummaryBlock(manifest.preReviewSummary),
    formatSharedContextCacheBlock(manifest.sharedContextCache),
    formatIncrementalReviewCacheBlock(manifest.incrementalReviewCache),
    'Review work packets:',
    formatWorkPacketBlock(manifest.workPackets),
    'Work packet rules:',
    '- Each reviewer Task prompt must include the matching work packet verbatim.',
    '- Include the packet_id in each Task description, for example "Security review [packet reviewer:ReviewSecurity:group-1-of-3]".',
    '- Each reviewer and judge response must echo packet_id and set status to completed, partial_timeout, timed_out, cancelled_by_user, failed, or skipped.',
    '- If the reviewer reports packet_id itself, mark reviewers[].packet_status_source as reported in the final submit_code_review payload.',
    '- If the reviewer omits packet_id but the Task was launched from a packet, infer the packet_id from the Task description or work packet and mark packet_status_source as inferred.',
    '- If packet_id cannot be reported or inferred, mark packet_status_source as missing and explain the confidence impact in coverage_notes.',
    '- If a reviewer response is missing packet_id or status, the judge must treat that reviewer output as lower confidence instead of discarding the whole review.',
    '- Use the pre-generated diff summary for initial orientation and token discipline, but verify claims against assigned files or diffs before reporting findings.',
    '- When prompt_byte_limit_exceeded is yes, use the pre-generated diff summary before detailed reads. Do not remove files from assigned_scope or hide unreviewed files; if a file cannot be covered, report it in coverage_notes and reliability_signals.',
    '- Use shared_context_cache entries to reuse read-only GetFileDiff/Read context by cache_key across reviewer packets. Do not duplicate full-file reads when a reusable cached diff or file summary already covers the same path.',
    '- Use incremental_review_cache only when the target fingerprint matches a prior run; preserve completed reviewer outputs by packet_id and rerun only missing, failed, timed-out, or stale packets. If any invalidates_on condition changed, ignore the cache and explain the fresh review boundary.',
    '- The assigned_scope is the default scope for that packet; only widen it when a critical cross-file dependency requires it and note the reason in coverage_notes.',
    'Configured code review team:',
    members || '- No team members available.',
    'Execution policy:',
    executionPolicy,
    'Concurrency policy:',
    concurrencyPolicy,
    'Team execution rules:',
    '- Run only reviewers listed in core_reviewers and enabled_extra_reviewers.',
    '- Do not launch skipped_reviewers.',
    '- If a skipped reviewer has reason not_applicable, mention it in coverage notes without treating it as reduced confidence.',
    '- If a skipped reviewer has reason budget_limited, mention the budget mode and the coverage tradeoff.',
    '- If a skipped reviewer has reason invalid_tooling, report it as a configuration issue and do not reduce confidence in the reviewers that did run.',
    '- If target_resolution is unknown, conditional reviewers may be activated conservatively; report that as coverage context.',
    `- Run the active core reviewer roles first: ${formatManifestList(manifest.coreReviewers, 'none')}.`,
    '- Launch reviewer Tasks by launch_batch. Do not launch a later reviewer batch until every reviewer Task in the earlier batch has completed, failed, timed out, or returned partial_timeout.',
    '- Never launch more reviewer Tasks in one batch than max_parallel_instances. If stagger_seconds is greater than 0, wait that many seconds before starting the next launch_batch.',
    '- Run ReviewJudge only after the reviewer batch finishes, as the quality-gate pass.',
    '- If other extra reviewers are configured and enabled, run them in parallel with the locked reviewers whenever possible.',
    '- When a configured member entry provides model_id, pass model_id with that value to the matching Task call.',
    '- If reviewer_timeout_seconds is greater than 0, pass timeout_seconds with that value to every reviewer Task call.',
    '- If judge_timeout_seconds is greater than 0, pass timeout_seconds with that value to the ReviewJudge Task call.',
    '- If a reviewer Task returns status partial_timeout, treat its output as partial evidence: preserve it in reviewers[].partial_output, mark the reviewer status partial_timeout, and mention the confidence impact in coverage_notes.',
    '- If a reviewer fails or times out without useful partial output, retry that same reviewer at most max_retries_per_role times: reduce its scope, downgrade strategy by one level when possible, use a shorter timeout, and set retry to true on the retry Task call.',
    '- In the final submit_code_review payload, populate reliability_signals for context_pressure, compression_preserved, partial_reviewer, and user_decision when those conditions apply. Use severity info/warning/action, count when useful, and source runtime/manifest/report/inferred.',
    '- If reviewer_file_split_threshold is greater than 0 and the target file count exceeds it, split files across multiple same-role reviewer instances only up to the concurrency-capped max_same_role_instances for this run.',
    '- Prefer module/workspace-area coherent file groups when splitting reviewer work; avoid mixing unrelated workspace areas in the same packet when the group budget allows it.',
    '- When file splitting is active, each same-role instance must only review its assigned file group. Label instances in the Task description with both group and packet_id (e.g. "Security review [group 1/3] [packet reviewer:ReviewSecurity:group-1-of-3]").',
    '- Do not run ReviewFixer during the review pass.',
    '- Wait for explicit user approval before starting any remediation.',
    '- The Review Quality Inspector acts as a third-party arbiter: it primarily examines reviewer reports for logical consistency and evidence quality, and only uses code inspection tools for targeted spot-checks when a specific claim needs verification.',
    'Review strategy rules:',
    `- Team strategy: ${manifest.strategyLevel}. ${formatStrategyImpact(manifest.strategyLevel, strategyProfiles)}`,
    '- Risk recommendation is advisory; follow team_strategy, member strategy fields, and work-packet strategy for this run unless the user explicitly changes strategy.',
    commonStrategyRules,
    'Review strategy profiles:',
    strategyRules,
  ].join('\n');
}
