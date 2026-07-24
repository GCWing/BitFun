import type {
  ReviewStrategyLevel,
  ReviewTeamCoreRoleDefinition,
  ReviewTeamDefinition,
  ReviewTeamExecutionPolicy,
  ReviewTokenBudgetMode,
} from './types';
import { REVIEW_STRATEGY_PROFILES } from './strategy';
import { UI_EXCEPTION_ACCENTS } from '@/shared/theme/uiExceptionAccents';

export const DEFAULT_REVIEW_TEAM_ID = 'default-review-team';
export const DEFAULT_REVIEW_TEAM_CONFIG_PATH = 'ai.review_teams.default';
export const DEFAULT_REVIEW_TEAM_RATE_LIMIT_STATUS_CONFIG_PATH =
  'ai.review_team_rate_limit_status';
export const DEFAULT_REVIEW_TEAM_MODEL = 'fast';
export const DEFAULT_REVIEW_TEAM_STRATEGY_LEVEL = 'normal' as const;
export const DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL = 'inherit' as const;
export const DEFAULT_REVIEW_TEAM_EXECUTION_POLICY = {
  reviewerTimeoutSeconds: 3600,
  judgeTimeoutSeconds: 2400,
  reviewerFileSplitThreshold: 20,
  maxSameRoleInstances: 3,
  maxRetriesPerRole: 1,
} as const;
export const REVIEW_STRATEGY_RUNTIME_BUDGETS: Record<
  ReviewStrategyLevel,
  {
    tokenBudgetMode: ReviewTokenBudgetMode;
    executionPolicy: Pick<
      ReviewTeamExecutionPolicy,
      | 'reviewerTimeoutSeconds'
      | 'judgeTimeoutSeconds'
      | 'reviewerFileSplitThreshold'
      | 'maxSameRoleInstances'
    >;
    maxExtraReviewers: number;
  }
> = {
  quick: {
    tokenBudgetMode: 'economy',
    executionPolicy: {
      reviewerTimeoutSeconds: 1200,
      judgeTimeoutSeconds: 900,
      reviewerFileSplitThreshold: 0,
      maxSameRoleInstances: 1,
    },
    maxExtraReviewers: 0,
  },
  normal: {
    tokenBudgetMode: 'balanced',
    executionPolicy: {
      reviewerTimeoutSeconds: 1800,
      judgeTimeoutSeconds: 1200,
      reviewerFileSplitThreshold: 0,
      maxSameRoleInstances: 1,
    },
    maxExtraReviewers: 1,
  },
  deep: {
    tokenBudgetMode: 'thorough',
    executionPolicy: {
      reviewerTimeoutSeconds: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.reviewerTimeoutSeconds,
      judgeTimeoutSeconds: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.judgeTimeoutSeconds,
      reviewerFileSplitThreshold:
        DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.reviewerFileSplitThreshold,
      maxSameRoleInstances: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.maxSameRoleInstances,
    },
    maxExtraReviewers: Number.MAX_SAFE_INTEGER,
  },
};
export const DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY = {
  maxParallelInstances: 4,
  staggerSeconds: 0,
  maxQueueWaitSeconds: 1200,
  batchExtrasSeparately: true,
  allowProviderCapacityQueue: true,
  allowBoundedAutoRetry: false,
  autoRetryElapsedGuardSeconds: 180,
} as const;
export const MAX_PREDICTIVE_TIMEOUT_SECONDS = 3600;
export const MAX_PARALLEL_REVIEWER_INSTANCES = 16;
export const MAX_QUEUE_WAIT_SECONDS = 3600;
export const MAX_AUTO_RETRY_ELAPSED_GUARD_SECONDS = 900;
export const PREDICTIVE_TIMEOUT_PER_FILE_SECONDS = 15;
export const PREDICTIVE_TIMEOUT_PER_100_LINES_SECONDS = 30;
export const PREDICTIVE_TIMEOUT_BASE_SECONDS: Record<ReviewStrategyLevel, number> = {
  quick: 180,
  normal: 300,
  deep: 600,
};
export const REVIEW_TEAM_MEMBER_ACCENT_DEFAULT = UI_EXCEPTION_ACCENTS.reviewTeam.memberDefault;

export const EXTRA_MEMBER_DEFAULTS = {
  roleName: 'Additional Review Check',
  description: 'An optional independent check for a specific concern chosen by the user.',
  responsibilities: [
    'Add another independent view of the current change.',
    'Check only the requested changes and selected files.',
    'Return concrete findings with clear fixes or follow-up steps.',
  ],
  accentColor: REVIEW_TEAM_MEMBER_ACCENT_DEFAULT,
};

export const REVIEW_WORK_PACKET_ALLOWED_TOOLS = [
  'GetFileDiff',
  'Read',
  'Grep',
  'Glob',
  'LS',
] as const;

export const DEFAULT_REVIEW_TEAM_CORE_ROLES: ReviewTeamCoreRoleDefinition[] = [
  {
    key: 'worker',
    subagentId: 'ReviewWorker',
    funName: 'Focused Review',
    roleName: 'On-demand Review Check',
    description:
      'A read-only check whose focus and scope are chosen for the current change when more evidence would be useful.',
    responsibilities: [
      'Check only the question assigned by the main review.',
      'Stay within the selected scope and support conclusions with concrete evidence.',
      'Do not modify files or repeat work already completed by the main review.',
    ],
    accentColor: UI_EXCEPTION_ACCENTS.reviewTeam.worker,
  },
  {
    key: 'judge',
    subagentId: 'ReviewJudge',
    funName: 'Independent Review Check',
    roleName: 'Review Quality Check',
    description:
      'A read-only independent check used only when a serious finding, conflicting evidence, or an uncertain conclusion needs validation.',
    responsibilities: [
      'Confirm or reject disputed findings using concrete evidence.',
      'Check only the claims that need independent validation.',
      'Make sure each retained issue has a safe, practical next step.',
    ],
    accentColor: UI_EXCEPTION_ACCENTS.reviewTeam.judge,
  },
];

export const CORE_ROLE_IDS = new Set(
  DEFAULT_REVIEW_TEAM_CORE_ROLES.map((role) => role.subagentId),
);
export const LEGACY_REVIEW_WORKER_AGENT_IDS = [
  'ReviewBusinessLogic',
  'ReviewPerformance',
  'ReviewSecurity',
  'ReviewArchitecture',
  'ReviewFrontend',
  'ReviewGeneral',
] as const;
export const DISALLOWED_REVIEW_TEAM_MEMBER_IDS = new Set<string>([
  ...CORE_ROLE_IDS,
  'DeepReview',
  'ReviewFixer',
  ...LEGACY_REVIEW_WORKER_AGENT_IDS,
].sort());

export const FALLBACK_REVIEW_TEAM_DEFINITION: ReviewTeamDefinition = {
  id: DEFAULT_REVIEW_TEAM_ID,
  name: 'Code Review',
  description:
    'One main review that can request focused independent checks when more evidence is needed.',
  warning:
    'Strict review may take longer and usually consumes more tokens than a standard review.',
  defaultModel: DEFAULT_REVIEW_TEAM_MODEL,
  defaultStrategyLevel: DEFAULT_REVIEW_TEAM_STRATEGY_LEVEL,
  defaultExecutionPolicy: {
    ...DEFAULT_REVIEW_TEAM_EXECUTION_POLICY,
  },
  coreRoles: DEFAULT_REVIEW_TEAM_CORE_ROLES,
  strategyProfiles: REVIEW_STRATEGY_PROFILES,
  disallowedExtraSubagentIds: [...DISALLOWED_REVIEW_TEAM_MEMBER_IDS],
  hiddenAgentIds: [
    'DeepReview',
    ...DEFAULT_REVIEW_TEAM_CORE_ROLES.map((role) => role.subagentId),
  ],
};
