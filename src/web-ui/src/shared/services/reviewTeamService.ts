import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import {
  SubagentAPI,
  type SubagentInfo,
  type SubagentSource,
} from '@/infrastructure/api/service-api/SubagentAPI';

export const DEFAULT_REVIEW_TEAM_ID = 'default-review-team';
export const DEFAULT_REVIEW_TEAM_CONFIG_PATH = 'ai.review_teams.default';
export const DEFAULT_REVIEW_TEAM_MODEL = 'fast';
export const DEFAULT_REVIEW_TEAM_STRATEGY_LEVEL = 'normal' as const;
export const DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL = 'inherit' as const;
export const DEFAULT_REVIEW_TEAM_EXECUTION_POLICY = {
  reviewerTimeoutSeconds: 600,
  judgeTimeoutSeconds: 600,
  reviewerFileSplitThreshold: 20,
  maxSameRoleInstances: 3,
} as const;

export type ReviewStrategyLevel = 'quick' | 'normal' | 'deep';
export type ReviewMemberStrategyLevel = ReviewStrategyLevel | 'inherit';
export type ReviewStrategySource = 'team' | 'member';
export type ReviewModelFallbackReason = 'model_removed';

export interface ReviewStrategyCommonRules {
  reviewerPromptRules: string[];
}

export type ReviewRoleDirectiveKey =
  | 'ReviewBusinessLogic'
  | 'ReviewPerformance'
  | 'ReviewSecurity'
  | 'ReviewArchitecture'
  | 'ReviewFrontend'
  | 'ReviewJudge';

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
  roleDirectives: Partial<Record<ReviewRoleDirectiveKey, string>>;
}

export const REVIEW_STRATEGY_LEVELS: ReviewStrategyLevel[] = [
  'quick',
  'normal',
  'deep',
];

export const REVIEW_STRATEGY_COMMON_RULES: ReviewStrategyCommonRules = {
  reviewerPromptRules: [
    'Each reviewer must follow its own strategy field.',
    'Member-level strategy overrides take precedence over the team strategy.',
    'The reviewer Task prompt must include the resolved prompt_directive.',
  ],
};

export const REVIEW_STRATEGY_PROFILES: Record<
  ReviewStrategyLevel,
  ReviewStrategyProfile
> = {
  quick: {
    level: 'quick',
    label: 'Quick',
    summary:
      'Fast screening for high-confidence issues in the requested diff or scope.',
    tokenImpact: '0.4-0.6x',
    runtimeImpact: '0.5-0.7x',
    defaultModelSlot: 'fast',
    promptDirective:
      'Prefer a concise diff-focused pass. Report only high-confidence correctness, security, or regression risks and avoid speculative design rewrites.',
    roleDirectives: {
      ReviewBusinessLogic:
        'Only trace logic paths directly changed by the diff. Do not follow call chains beyond one hop. Report only issues where the diff introduces a provably wrong behavior.',
      ReviewPerformance:
        'Scan the diff for known anti-patterns only: nested loops, repeated fetches, blocking calls on hot paths, unnecessary re-renders. Do not trace call chains or estimate impact beyond what the diff shows.',
      ReviewSecurity:
        'Scan the diff for direct security risks only: injection, secret exposure, unsafe commands, missing auth. Do not trace data flows beyond one hop.',
      ReviewArchitecture:
        'Only check imports directly changed by the diff. Flag violations of documented layer boundaries.',
      ReviewFrontend:
        'Only check i18n key completeness and direct platform boundary violations in changed frontend files.',
      ReviewJudge:
        'This was a quick review. Focus on confirming or rejecting each finding efficiently. If a finding\'s evidence is thin, reject it rather than spending time verifying.',
    },
  },
  normal: {
    level: 'normal',
    label: 'Normal',
    summary:
      'Balanced review depth for day-to-day code review with practical evidence.',
    tokenImpact: '1x',
    runtimeImpact: '1x',
    defaultModelSlot: 'fast',
    promptDirective:
      'Perform the standard role-specific review. Balance coverage with precision and include concrete evidence for each issue.',
    roleDirectives: {
      ReviewBusinessLogic:
        'Trace each changed function\'s direct callers and callees to verify business rules and state transitions. Stop investigating a path once you have enough evidence to confirm or dismiss it.',
      ReviewPerformance:
        'Inspect the diff for anti-patterns, then read surrounding code to confirm impact on hot paths. Report only issues likely to matter at realistic scale.',
      ReviewSecurity:
        'Trace each changed input path from entry point to usage. Check trust boundaries, auth assumptions, and data sanitization. Report only issues with a realistic threat narrative.',
      ReviewArchitecture:
        "Check the diff's imports plus one level of dependency direction. Verify API contract consistency.",
      ReviewFrontend:
        'Check i18n, React performance patterns, and accessibility in changed components. Verify frontend-backend API contract alignment.',
      ReviewJudge:
        'Validate each finding\'s logical consistency and evidence quality. Spot-check code only when a claim needs verification.',
    },
  },
  deep: {
    level: 'deep',
    label: 'Deep',
    summary:
      'Thorough multi-pass review for risky, broad, or release-sensitive changes.',
    tokenImpact: '1.8-2.5x',
    runtimeImpact: '1.5-2.5x',
    defaultModelSlot: 'primary',
    promptDirective:
      'Run a thorough role-specific pass. Inspect edge cases, cross-file interactions, failure modes, and remediation tradeoffs before finalizing findings.',
    roleDirectives: {
      ReviewBusinessLogic:
        'Map full call chains for changed functions. Verify state transitions end-to-end, check rollback and error-recovery paths, and test edge cases in data shape and lifecycle assumptions. Prioritize findings by user-facing impact.',
      ReviewPerformance:
        'In addition to the normal pass, check for latent scaling risks — data structures that degrade at volume, or algorithms that are correct but unnecessarily expensive. Only report if you can estimate the impact. Do not speculate about edge cases or failure modes unrelated to performance.',
      ReviewSecurity:
        'In addition to the normal pass, trace data flows across trust boundaries end-to-end. Check for privilege escalation chains, indirect injection vectors, and failure modes that expose sensitive data. Report only issues with a complete threat narrative.',
      ReviewArchitecture:
        'Map the full dependency graph for changed modules. Check for structural anti-patterns, circular dependencies, and cross-cutting concerns.',
      ReviewFrontend:
        'Thorough React analysis: effect dependencies, memoization, virtualization. Full accessibility audit. State management pattern review. Cross-layer contract verification.',
      ReviewJudge:
        'This was a deep review with potentially complex findings. Cross-validate findings across reviewers for consistency. For each finding, verify the evidence supports the conclusion and the suggested fix is safe. Pay extra attention to overlapping findings across reviewers or same-role instances.',
    },
  },
};

export const REVIEW_STRATEGY_DEFINITIONS = REVIEW_STRATEGY_PROFILES;
export type ReviewStrategyDefinition = ReviewStrategyProfile;

export function getReviewStrategyProfile(
  strategyLevel: ReviewStrategyLevel,
): ReviewStrategyProfile {
  return REVIEW_STRATEGY_PROFILES[strategyLevel];
}

export type ReviewTeamCoreRoleKey =
  | 'businessLogic'
  | 'performance'
  | 'security'
  | 'architecture'
  | 'frontend'
  | 'judge';

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

export interface ReviewTeamStoredConfig {
  extra_subagent_ids: string[];
  strategy_level: ReviewStrategyLevel;
  member_strategy_overrides: Record<string, ReviewStrategyLevel>;
  reviewer_timeout_seconds: number;
  judge_timeout_seconds: number;
  reviewer_file_split_threshold: number;
  max_same_role_instances: number;
}

export interface ReviewTeamExecutionPolicy {
  reviewerTimeoutSeconds: number;
  judgeTimeoutSeconds: number;
  reviewerFileSplitThreshold: number;
  maxSameRoleInstances: number;
}

export interface ReviewTeamMember {
  id: string;
  subagentId: string;
  definitionKey?: ReviewTeamCoreRoleKey;
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
}

export interface ReviewTeam {
  id: string;
  name: string;
  description: string;
  warning: string;
  strategyLevel: ReviewStrategyLevel;
  memberStrategyOverrides: Record<string, ReviewStrategyLevel>;
  executionPolicy: ReviewTeamExecutionPolicy;
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
  reason?: 'disabled' | 'unavailable';
}

export interface ReviewTeamRunManifest {
  reviewMode: 'deep';
  workspacePath?: string;
  policySource: 'default-review-team-config';
  strategyLevel: ReviewStrategyLevel;
  executionPolicy: ReviewTeamExecutionPolicy;
  coreReviewers: ReviewTeamManifestMember[];
  qualityGateReviewer?: ReviewTeamManifestMember;
  enabledExtraReviewers: ReviewTeamManifestMember[];
  skippedReviewers: ReviewTeamManifestMember[];
}

const EXTRA_MEMBER_DEFAULTS = {
  roleName: 'Additional Specialist Reviewer',
  description:
    'User-added Sub-Agent that joins the deep review lineup with its own instructions, tools, and perspective.',
  responsibilities: [
    'Bring an extra independent review perspective into the same target scope.',
    'Stay tightly focused on the requested diff, commit, or workspace changes.',
    'Return concrete findings with clear fix suggestions or follow-up steps.',
  ],
  accentColor: '#64748b',
};

export const DEFAULT_REVIEW_TEAM_CORE_ROLES: ReviewTeamCoreRoleDefinition[] = [
  {
    key: 'businessLogic',
    subagentId: 'ReviewBusinessLogic',
    funName: 'Logic Reviewer',
    roleName: 'Business Logic Reviewer',
    description:
      'A workflow sleuth that inspects business rules, state transitions, recovery paths, and real-user correctness.',
    responsibilities: [
      'Verify workflows, state transitions, and domain rules still behave correctly.',
      'Check boundary cases, rollback paths, and data integrity assumptions.',
      'Focus on issues that can break user outcomes or product intent.',
    ],
    accentColor: '#2563eb',
  },
  {
    key: 'performance',
    subagentId: 'ReviewPerformance',
    funName: 'Performance Reviewer',
    roleName: 'Performance Reviewer',
    description:
      'A speed-focused profiler that hunts hot paths, unnecessary work, blocking calls, and scale-sensitive regressions.',
    responsibilities: [
      'Inspect hot paths, large loops, and unnecessary allocations or recomputation.',
      'Flag blocking work, N+1 patterns, and wasteful data movement.',
      'Keep performance advice practical and aligned with the existing architecture.',
    ],
    accentColor: '#d97706',
  },
  {
    key: 'security',
    subagentId: 'ReviewSecurity',
    funName: 'Security Reviewer',
    roleName: 'Security Reviewer',
    description:
      'A boundary guardian that scans for injection risks, trust leaks, privilege mistakes, and unsafe file or command handling.',
    responsibilities: [
      'Review trust boundaries, auth assumptions, and sensitive data handling.',
      'Look for injection, unsafe command execution, and exposure risks.',
      'Highlight concrete fixes that reduce risk without broad rewrites.',
    ],
    accentColor: '#dc2626',
  },
  {
    key: 'architecture',
    subagentId: 'ReviewArchitecture',
    funName: 'Architecture Reviewer',
    roleName: 'Architecture Reviewer',
    description:
      'A structural watchdog that checks module boundaries, dependency direction, API contract design, and abstraction integrity.',
    responsibilities: [
      'Detect layer boundary violations and wrong-direction imports.',
      'Verify API contracts, tool schemas, and transport messages stay consistent.',
      'Ensure platform-agnostic code does not leak platform-specific details.',
    ],
    accentColor: '#0891b2',
  },
  {
    key: 'frontend',
    subagentId: 'ReviewFrontend',
    funName: 'Frontend Reviewer',
    roleName: 'Frontend Reviewer',
    description:
      'A UI specialist that checks i18n synchronization, React performance patterns, accessibility, and frontend-backend contract alignment.',
    responsibilities: [
      'Verify i18n key completeness across all locales.',
      'Check React performance patterns (memoization, virtualization, effect dependencies).',
      'Flag accessibility violations and frontend-backend API contract drift.',
    ],
    accentColor: '#059669',
    conditional: true,
  },
  {
    key: 'judge',
    subagentId: 'ReviewJudge',
    funName: 'Review Arbiter',
    roleName: 'Review Quality Inspector',
    description:
      'An independent third-party arbiter that validates reviewer reports for logical consistency and evidence quality. It spot-checks specific code locations only when a claim needs verification, rather than re-reviewing the codebase from scratch.',
    responsibilities: [
      'Validate, merge, downgrade, or reject reviewer findings based on logical consistency and evidence quality.',
      'Filter out false positives and directionally-wrong optimization advice by examining reviewer reasoning.',
      'Spot-check specific code locations only when a reviewer claim needs verification.',
      'Ensure every surviving issue has an actionable fix or follow-up plan.',
    ],
    accentColor: '#7c3aed',
  },
];

const CORE_ROLE_IDS = new Set(
  DEFAULT_REVIEW_TEAM_CORE_ROLES.map((role) => role.subagentId),
);
const DISALLOWED_REVIEW_TEAM_MEMBER_IDS = new Set<string>([
  ...CORE_ROLE_IDS,
  'DeepReview',
  'ReviewFixer',
]);

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

function normalizeExecutionPolicy(
  raw: unknown,
): Pick<
  ReviewTeamStoredConfig,
  | 'reviewer_timeout_seconds'
  | 'judge_timeout_seconds'
  | 'reviewer_file_split_threshold'
  | 'max_same_role_instances'
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
  };
}

function normalizeStoredConfig(raw: unknown): ReviewTeamStoredConfig {
  const extraIds = Array.isArray((raw as { extra_subagent_ids?: unknown })?.extra_subagent_ids)
    ? (raw as { extra_subagent_ids: unknown[] }).extra_subagent_ids
      .filter((value): value is string => typeof value === 'string')
    : [];
  const executionPolicy = normalizeExecutionPolicy(raw);
  const config = raw as Partial<ReviewTeamStoredConfig> | undefined;

  return {
    extra_subagent_ids: dedupeIds(extraIds).filter((id) => !DISALLOWED_REVIEW_TEAM_MEMBER_IDS.has(id)),
    strategy_level: normalizeTeamStrategyLevel(config?.strategy_level),
    member_strategy_overrides: normalizeMemberStrategyOverrides(
      config?.member_strategy_overrides,
    ),
    ...executionPolicy,
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
  });
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
  });
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
): {
  model: string;
  configuredModel: string;
  modelFallbackReason?: ReviewModelFallbackReason;
} {
  const normalizedConfiguredModel = configuredModel?.trim() || '';
  const defaultModelSlot = getReviewStrategyProfile(strategyLevel).defaultModelSlot;

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
): ReviewTeamMember {
  const strategy = resolveMemberStrategy(storedConfig, definition.subagentId);
  const model = resolveMemberModel(
    info?.model || DEFAULT_REVIEW_TEAM_MODEL,
    strategy.strategyLevel,
    availableModelIds,
  );

  return {
    id: `core:${definition.subagentId}`,
    subagentId: definition.subagentId,
    definitionKey: definition.key,
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
  };
}

function buildExtraMember(
  info: SubagentInfo,
  storedConfig: ReviewTeamStoredConfig,
  availableModelIds?: Set<string>,
): ReviewTeamMember {
  const strategy = resolveMemberStrategy(storedConfig, info.id);
  const model = resolveMemberModel(
    info.model || DEFAULT_REVIEW_TEAM_MODEL,
    strategy.strategyLevel,
    availableModelIds,
  );

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
    available: true,
    locked: false,
    source: 'extra',
    subagentSource: info.subagentSource ?? 'builtin',
    accentColor: EXTRA_MEMBER_DEFAULTS.accentColor,
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

export function canUseSubagentAsReviewTeamMember(
  subagent: Pick<SubagentInfo, 'id' | 'isReadonly' | 'isReview'>,
): boolean {
  return subagent.isReview && subagent.isReadonly && canAddSubagentToReviewTeam(subagent.id);
}

export function resolveDefaultReviewTeam(
  subagents: SubagentInfo[],
  storedConfig: ReviewTeamStoredConfig,
  options: ResolveDefaultReviewTeamOptions = {},
): ReviewTeam {
  const byId = new Map(subagents.map((subagent) => [subagent.id, subagent]));
  const availableModelIds = options.availableModelIds
    ? new Set(options.availableModelIds)
    : undefined;
  const coreMembers = DEFAULT_REVIEW_TEAM_CORE_ROLES.map((definition) =>
    buildCoreMember(
      definition,
      byId.get(definition.subagentId),
      storedConfig,
      availableModelIds,
    ),
  );
  const extraMembers = storedConfig.extra_subagent_ids
    .map((subagentId) => byId.get(subagentId))
    .filter((subagent): subagent is SubagentInfo => Boolean(subagent))
    .filter(canUseSubagentAsReviewTeamMember)
    .map((subagent) => buildExtraMember(subagent, storedConfig, availableModelIds));

  return {
    id: DEFAULT_REVIEW_TEAM_ID,
    name: 'Code Review Team',
    description:
      'A multi-reviewer team for deep code review with mandatory logic, performance, security, architecture, conditional frontend, and quality-gate roles.',
    warning:
      'Deep review may take longer and usually consumes more tokens than a standard review.',
    strategyLevel: storedConfig.strategy_level,
    memberStrategyOverrides: storedConfig.member_strategy_overrides,
    executionPolicy: executionPolicyFromStoredConfig(storedConfig),
    members: [...coreMembers, ...extraMembers],
    coreMembers,
    extraMembers,
  };
}

export async function loadDefaultReviewTeam(
  workspacePath?: string,
): Promise<ReviewTeam> {
  const [storedConfig, subagents, rawModels] = await Promise.all([
    loadDefaultReviewTeamConfig(),
    SubagentAPI.listSubagents({ workspacePath }),
    configAPI.getConfig('ai.models').catch(() => undefined),
  ]);

  return resolveDefaultReviewTeam(subagents, storedConfig, {
    availableModelIds: extractAvailableModelIds(rawModels),
  });
}

export async function prepareDefaultReviewTeamForLaunch(
  workspacePath?: string,
): Promise<ReviewTeam> {
  const team = await loadDefaultReviewTeam(workspacePath);
  const missingCoreMembers = team.coreMembers.filter((member) => !member.available);

  if (missingCoreMembers.length > 0) {
    throw new Error(
      `Required code review team members are unavailable: ${missingCoreMembers
        .map((member) => member.subagentId)
        .join(', ')}`,
    );
  }

  const coreMembersToEnable = team.coreMembers.filter(
    (member) => member.available && !member.enabled,
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
    defaultModelSlot: strategyProfile.defaultModelSlot,
    strategyLevel: member.strategyLevel,
    strategySource: member.strategySource,
    strategyDirective: roleDirective || strategyProfile.promptDirective,
    locked: member.locked,
    source: member.source,
    subagentSource: member.subagentSource,
    ...(reason ? { reason } : {}),
  };
}

export function buildEffectiveReviewTeamManifest(
  team: ReviewTeam,
  options: {
    workspacePath?: string;
    policySource?: ReviewTeamRunManifest['policySource'];
  } = {},
): ReviewTeamRunManifest {
  const availableCoreMembers = team.coreMembers.filter((member) => member.available);
  const unavailableCoreMembers = team.coreMembers.filter((member) => !member.available);
  const coreReviewers = availableCoreMembers
    .filter((member) => member.definitionKey !== 'judge')
    .map((member) => toManifestMember(member));
  const qualityGateReviewer = availableCoreMembers.find(
    (member) => member.definitionKey === 'judge',
  );
  const enabledExtraReviewers = team.extraMembers
    .filter((member) => member.available && member.enabled)
    .map((member) => toManifestMember(member));
  const skippedReviewers = [
    ...team.extraMembers
      .filter((member) => !member.available || !member.enabled)
      .map((member) =>
        toManifestMember(member, member.available ? 'disabled' : 'unavailable'),
      ),
    ...unavailableCoreMembers.map((member) =>
      toManifestMember(member, 'unavailable'),
    ),
  ];

  return {
    reviewMode: 'deep',
    ...(options.workspacePath ? { workspacePath: options.workspacePath } : {}),
    policySource: options.policySource ?? 'default-review-team-config',
    strategyLevel: team.strategyLevel,
    executionPolicy: team.executionPolicy,
    coreReviewers,
    ...(qualityGateReviewer
      ? { qualityGateReviewer: toManifestMember(qualityGateReviewer) }
      : {}),
    enabledExtraReviewers,
    skippedReviewers,
  };
}

function formatResponsibilities(items: string[]): string {
  return items.map((item) => `    - ${item}`).join('\n');
}

function formatStrategyImpact(strategyLevel: ReviewStrategyLevel): string {
  const definition = getReviewStrategyProfile(strategyLevel);
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
    `- reviewer_timeout_seconds: ${team.executionPolicy.reviewerTimeoutSeconds}`,
    `- judge_timeout_seconds: ${team.executionPolicy.judgeTimeoutSeconds}`,
    `- reviewer_file_split_threshold: ${team.executionPolicy.reviewerFileSplitThreshold}`,
    `- max_same_role_instances: ${team.executionPolicy.maxSameRoleInstances}`,
  ].join('\n');
  const manifestBlock = [
    'Run manifest:',
    `- review_mode: ${manifest.reviewMode}`,
    `- team_strategy: ${manifest.strategyLevel}`,
    `- workspace_path: ${manifest.workspacePath || 'inherited from current session'}`,
    `- policy_source: ${manifest.policySource}`,
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
  const strategyRules = REVIEW_STRATEGY_LEVELS.map((level) => {
    const definition = getReviewStrategyProfile(level);
    const roleEntries = Object.entries(definition.roleDirectives) as [ReviewRoleDirectiveKey, string][];
    const roleLines = roleEntries.map(
      ([role, directive]) => `    - ${role}: ${directive}`,
    );
    return [
      `- ${level}: ${definition.summary}`,
      `  - ${formatStrategyImpact(level)}`,
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
    'Configured code review team:',
    members || '- No team members available.',
    'Execution policy:',
    executionPolicy,
    'Team execution rules:',
    '- Always run the four locked core reviewer roles first: ReviewBusinessLogic, ReviewPerformance, ReviewSecurity, and ReviewArchitecture.',
    '- Run ReviewJudge only after the reviewer batch finishes, as the quality-gate pass.',
    '- If the Frontend Reviewer is enabled, run it in parallel with the locked reviewers whenever the change contains frontend files (src/web-ui/, .tsx, .scss, .css, locales/).',
    '- If other extra reviewers are configured and enabled, run them in parallel with the locked reviewers whenever possible.',
    '- When a configured member entry provides model_id, pass model_id with that value to the matching Task call.',
    '- If reviewer_timeout_seconds is greater than 0, pass timeout_seconds with that value to every reviewer Task call.',
    '- If judge_timeout_seconds is greater than 0, pass timeout_seconds with that value to the ReviewJudge Task call.',
    '- If reviewer_file_split_threshold is greater than 0 and the target file count exceeds it, split files across multiple same-role reviewer instances (up to max_same_role_instances per role). Launch all split instances in the same parallel message.',
    '- When file splitting is active, each same-role instance must only review its assigned file group. Label instances in the Task description (e.g. "Security review [group 1/3]").',
    '- Do not run ReviewFixer during the review pass.',
    '- Wait for explicit user approval before starting any remediation.',
    '- The Review Quality Inspector acts as a third-party arbiter: it primarily examines reviewer reports for logical consistency and evidence quality, and only uses code inspection tools for targeted spot-checks when a specific claim needs verification.',
    'Review strategy rules:',
    `- Team strategy: ${team.strategyLevel}. ${formatStrategyImpact(team.strategyLevel)}`,
    commonStrategyRules,
    'Review strategy profiles:',
    strategyRules,
  ].join('\n');
}
