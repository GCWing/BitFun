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
  MAX_AUTO_RETRY_ELAPSED_GUARD_SECONDS,
  MAX_PARALLEL_REVIEWER_INSTANCES,
  MAX_QUEUE_WAIT_SECONDS,
  REVIEW_WORK_PACKET_ALLOWED_TOOLS,
} from './defaults';
import {
  REVIEW_STRATEGY_LEVELS,
  REVIEW_STRATEGY_PROFILES,
} from './strategy';
import { buildPreReviewSummary } from './preReviewSummary';
import {
  buildIncrementalReviewCachePlan,
  buildSharedContextCachePlan,
} from './cachePlan';
import { buildDeepReviewEvidencePack } from './evidencePack';
import {
  applyTeamStrategyOverrideToMember,
  toManifestMember,
} from './manifestMembers';
import {
  buildReviewStrategyDecision,
  recommendBackendCompatibleStrategyForTarget,
  recommendReviewStrategyForTarget,
} from './risk';
import { buildDeepReviewScopeProfile } from './scopeProfile';
import {
  buildEffectiveExecutionPolicy,
  buildTokenBudgetPlan,
} from './tokenBudget';
import {
  buildWorkPackets,
  resolveChangeStats,
  resolveMaxExtraReviewers,
} from './workPackets';
import { buildReviewTeamPromptBlockContent } from './promptBlock';
import type {
  ReviewMemberStrategyLevel,
  ReviewModelFallbackReason,
  ReviewStrategyLevel,
  ReviewStrategyProfile,
  ReviewStrategySource,
  ReviewTeam,
  ReviewTeamChangeStats,
  ReviewTeamConcurrencyPolicy,
  ReviewTeamCoreRoleDefinition,
  ReviewTeamCoreRoleKey,
  ReviewTeamDefinition,
  ReviewTeamExecutionPolicy,
  ReviewTeamManifestMemberReason,
  ReviewTeamMember,
  ReviewTeamRateLimitStatus,
  ReviewTeamRunManifest,
  ReviewTeamStoredConfig,
  ReviewTokenBudgetMode,
} from './types';

export * from './types';
export * from './strategy';
export { recommendReviewStrategyForTarget } from './risk';
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

function shouldRunCoreReviewerForTarget(
  member: ReviewTeamMember,
  target: ReviewTargetClassification,
): boolean {
  return shouldRunReviewerForTarget(member.subagentId, target);
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
  const scopeProfile = buildDeepReviewScopeProfile(strategyLevel);
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
  const evidencePack = buildDeepReviewEvidencePack({
    target,
    changeStats,
    scopeProfile,
    workPackets,
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
    scopeProfile,
    strategyRecommendation,
    strategyDecision,
    executionPolicy,
    concurrencyPolicy,
    changeStats,
    preReviewSummary,
    evidencePack,
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

export function buildReviewTeamPromptBlock(
  team: ReviewTeam,
  manifest = buildEffectiveReviewTeamManifest(team),
): string {
  return buildReviewTeamPromptBlockContent(team, manifest);
}
