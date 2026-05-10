import type {
  ReviewStrategyLevel,
  ReviewTeamCoreRoleDefinition,
  ReviewTeamDefinition,
  ReviewTokenBudgetMode,
} from './types';
import { REVIEW_STRATEGY_PROFILES } from './strategy';

export const DEFAULT_REVIEW_TEAM_ID = 'default-review-team';
export const DEFAULT_REVIEW_TEAM_CONFIG_PATH = 'ai.review_teams.default';
export const DEFAULT_REVIEW_TEAM_RATE_LIMIT_STATUS_CONFIG_PATH =
  'ai.review_teams.rate_limit_status';
export const DEFAULT_REVIEW_TEAM_PROJECT_STRATEGY_OVERRIDES_CONFIG_PATH =
  'ai.review_teams.project_strategy_overrides';
export const DEFAULT_REVIEW_TEAM_MODEL = 'fast';
export const DEFAULT_REVIEW_TEAM_STRATEGY_LEVEL = 'normal' as const;
export const DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL = 'inherit' as const;
export const DEFAULT_REVIEW_TEAM_EXECUTION_POLICY = {
  reviewerTimeoutSeconds: 600,
  judgeTimeoutSeconds: 600,
  reviewerFileSplitThreshold: 20,
  maxSameRoleInstances: 3,
  maxRetriesPerRole: 1,
} as const;
export const DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY = {
  maxParallelInstances: 4,
  staggerSeconds: 0,
  maxQueueWaitSeconds: 60,
  batchExtrasSeparately: true,
  allowProviderCapacityQueue: true,
  allowBoundedAutoRetry: false,
  autoRetryElapsedGuardSeconds: 180,
} as const;
export const MAX_PREDICTIVE_TIMEOUT_SECONDS = 3600;
export const MAX_PARALLEL_REVIEWER_INSTANCES = 16;
export const MAX_QUEUE_WAIT_SECONDS = 600;
export const MAX_AUTO_RETRY_ELAPSED_GUARD_SECONDS = 900;
export const PREDICTIVE_TIMEOUT_PER_FILE_SECONDS = 15;
export const PREDICTIVE_TIMEOUT_PER_100_LINES_SECONDS = 30;
export const PREDICTIVE_TIMEOUT_BASE_SECONDS: Record<ReviewStrategyLevel, number> = {
  quick: 180,
  normal: 300,
  deep: 600,
};
export const TOKEN_BUDGET_PROMPT_BYTE_LIMIT_BY_MODE: Record<ReviewTokenBudgetMode, number> = {
  economy: 64_000,
  balanced: 96_000,
  thorough: 192_000,
};
export const PROMPT_BYTE_ESTIMATE_BASE_BYTES = 12_000;
export const PROMPT_BYTE_ESTIMATE_PER_FILE_BYTES = 1_800;
export const PROMPT_BYTE_ESTIMATE_PER_CHANGED_LINE_BYTES = 120;
export const PROMPT_BYTE_ESTIMATE_UNKNOWN_LINES_PER_FILE = 80;

export const EXTRA_MEMBER_DEFAULTS = {
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

export const REVIEW_WORK_PACKET_ALLOWED_TOOLS = [
  'GetFileDiff',
  'Read',
  'Grep',
  'Glob',
  'LS',
  'Git',
] as const;

export const REVIEWER_WORK_PACKET_REQUIRED_OUTPUT_FIELDS = [
  'packet_id',
  'status',
  'verdict',
  'findings',
  'reviewer_summary',
] as const;

export const JUDGE_WORK_PACKET_REQUIRED_OUTPUT_FIELDS = [
  'packet_id',
  'status',
  'decision_summary',
  'validated_findings',
  'rejected_or_downgraded_notes',
  'coverage_notes',
] as const;

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

export const CORE_ROLE_IDS = new Set(
  DEFAULT_REVIEW_TEAM_CORE_ROLES.map((role) => role.subagentId),
);
export const DISALLOWED_REVIEW_TEAM_MEMBER_IDS = new Set<string>([
  ...CORE_ROLE_IDS,
  'DeepReview',
  'ReviewFixer',
]);

export const FALLBACK_REVIEW_TEAM_DEFINITION: ReviewTeamDefinition = {
  id: DEFAULT_REVIEW_TEAM_ID,
  name: 'Code Review Team',
  description:
    'A multi-reviewer team for deep code review with mandatory logic, performance, security, architecture, conditional frontend, and quality-gate roles.',
  warning:
    'Deep review may take longer and usually consumes more tokens than a standard review.',
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
