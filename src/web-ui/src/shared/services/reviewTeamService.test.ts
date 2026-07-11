import { beforeEach, describe, expect, it, vi } from 'vitest';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import {
  DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY,
  DEFAULT_REVIEW_TEAM_EXECUTION_POLICY,
  DEFAULT_REVIEW_TEAM_STRATEGY_LEVEL,
  FALLBACK_REVIEW_TEAM_DEFINITION,
  REVIEW_TEAM_MEMBER_ACCENT_DEFAULT,
  REVIEW_STRATEGY_DEFINITIONS,
  buildEffectiveReviewTeamManifest,
  buildReviewTeamPromptBlock,
  canUseSubagentAsReviewTeamMember,
  loadDefaultReviewTeamDefinition,
  loadDefaultReviewTeamConfig,
  loadReviewTeamRateLimitStatus,
  prepareDefaultReviewTeamForLaunch,
  resolveDefaultReviewTeam,
  saveDefaultReviewTeamConcurrencyPolicy,
  type ReviewTeamStoredConfig,
} from './reviewTeamService';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import {
  SubagentAPI,
  type SubagentInfo,
} from '@/infrastructure/api/service-api/SubagentAPI';
import {
  classifyReviewTargetFromFiles,
  createUnknownReviewTargetClassification,
} from './reviewTargetClassifier';

vi.mock('@/infrastructure/api/service-api/ConfigAPI', () => ({
  configAPI: {
    getConfig: vi.fn(),
    setConfig: vi.fn(),
  },
}));

vi.mock('@/infrastructure/api/service-api/SubagentAPI', () => ({
  SubagentAPI: {
    listSubagents: vi.fn(),
    listVisibleSubagents: vi.fn(),
    updateSubagentConfig: vi.fn(),
  },
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: {
    getDefaultReviewTeamDefinition: vi.fn(),
  },
}));

describe('reviewTeamService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const WORKSPACE_PATH = '/test-fixtures/project-a';

  const storedConfigWithExtra = (
    extraSubagentIds: string[] = [],
    overrides: Partial<ReviewTeamStoredConfig> = {},
  ): ReviewTeamStoredConfig => ({
    extra_subagent_ids: extraSubagentIds,
    strategy_level: DEFAULT_REVIEW_TEAM_STRATEGY_LEVEL,
    member_strategy_overrides: {},
    reviewer_timeout_seconds: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.reviewerTimeoutSeconds,
    judge_timeout_seconds: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.judgeTimeoutSeconds,
    reviewer_file_split_threshold: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.reviewerFileSplitThreshold,
    max_same_role_instances: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.maxSameRoleInstances,
    max_retries_per_role: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.maxRetriesPerRole,
    max_parallel_reviewers: DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.maxParallelInstances,
    max_queue_wait_seconds: DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.maxQueueWaitSeconds,
    allow_provider_capacity_queue: DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.allowProviderCapacityQueue,
    allow_bounded_auto_retry: DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.allowBoundedAutoRetry,
    auto_retry_elapsed_guard_seconds:
      DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.autoRetryElapsedGuardSeconds,
    ...overrides,
  });

  const subagent = (
    id: string,
    enabled = true,
    subagentSource: SubagentInfo['subagentSource'] = 'builtin',
    model = 'fast',
    isReadonly = true,
    isReview = id.startsWith('Review'),
    defaultTools = ['GetFileDiff', 'Read', 'Grep', 'Glob', 'LS'],
  ): SubagentInfo => ({
    key: `test::${id}`,
    id,
    name: id,
    description: `${id} description`,
    isReadonly,
    isReview,
    toolCount: defaultTools.length,
    defaultTools,
    defaultEnabled: enabled,
    effectiveEnabled: enabled,
    subagentSource,
    model,
  });

  const coreSubagents = (enabled = true): SubagentInfo[] => [
    subagent('ReviewBusinessLogic', enabled),
    subagent('ReviewPerformance', enabled),
    subagent('ReviewSecurity', enabled),
    subagent('ReviewArchitecture', enabled),
    subagent('ReviewFrontend', enabled),
    subagent('ReviewJudge', enabled),
  ];

  it('uses slow-provider-friendly review team defaults', () => {
    expect(DEFAULT_REVIEW_TEAM_EXECUTION_POLICY).toMatchObject({
      reviewerTimeoutSeconds: 3600,
      judgeTimeoutSeconds: 2400,
      reviewerFileSplitThreshold: 20,
      maxSameRoleInstances: 3,
      maxRetriesPerRole: 1,
    });
    expect(DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY).toMatchObject({
      maxParallelInstances: 4,
      staggerSeconds: 0,
      maxQueueWaitSeconds: 1200,
      batchExtrasSeparately: true,
      allowProviderCapacityQueue: true,
      allowBoundedAutoRetry: false,
      autoRetryElapsedGuardSeconds: 180,
    });
  });

  it('falls back to defaults when the persisted review team path is missing', async () => {
    vi.mocked(configAPI.getConfig).mockRejectedValueOnce(
      new Error("Config path 'ai.review_teams.default' not found"),
    );

    await expect(loadDefaultReviewTeamConfig()).resolves.toEqual({
      extra_subagent_ids: [],
      strategy_level: 'normal',
      member_strategy_overrides: {},
      reviewer_timeout_seconds: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.reviewerTimeoutSeconds,
      judge_timeout_seconds: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.judgeTimeoutSeconds,
      reviewer_file_split_threshold: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.reviewerFileSplitThreshold,
      max_same_role_instances: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.maxSameRoleInstances,
      max_retries_per_role: DEFAULT_REVIEW_TEAM_EXECUTION_POLICY.maxRetriesPerRole,
      max_parallel_reviewers: DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.maxParallelInstances,
      max_queue_wait_seconds: DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.maxQueueWaitSeconds,
      allow_provider_capacity_queue: DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.allowProviderCapacityQueue,
      allow_bounded_auto_retry: DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.allowBoundedAutoRetry,
      auto_retry_elapsed_guard_seconds:
        DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY.autoRetryElapsedGuardSeconds,
    });
  });

  it('defaults deep review launches to read-only mode without automatic fixing', async () => {
    vi.mocked(configAPI.getConfig).mockRejectedValueOnce(
      new Error("Config path 'ai.review_teams.default' not found"),
    );

    const config = await loadDefaultReviewTeamConfig();

    expect(config.strategy_level).toBe('normal');
  });

  it('normalizes team strategy and member strategy overrides', async () => {
    vi.mocked(configAPI.getConfig).mockResolvedValueOnce({
      extra_subagent_ids: ['ExtraOne'],
      strategy_level: 'deep',
      member_strategy_overrides: {
        ReviewSecurity: 'quick',
        ReviewJudge: 'deep',
        ExtraOne: 'normal',
        ExtraTwo: 'invalid',
      },
    });

    await expect(loadDefaultReviewTeamConfig()).resolves.toMatchObject({
      strategy_level: 'deep',
      member_strategy_overrides: {
        ReviewSecurity: 'quick',
        ReviewJudge: 'deep',
        ExtraOne: 'normal',
      },
    });
  });

  it('normalizes persisted capacity and retry settings into the team concurrency policy', async () => {
    vi.mocked(configAPI.getConfig).mockResolvedValueOnce({
      extra_subagent_ids: [],
      strategy_level: 'normal',
      member_strategy_overrides: {},
      max_parallel_reviewers: 99,
      max_queue_wait_seconds: 9999,
      allow_provider_capacity_queue: false,
      allow_bounded_auto_retry: true,
      auto_retry_elapsed_guard_seconds: 1,
    });

    const config = await loadDefaultReviewTeamConfig();
    const team = resolveDefaultReviewTeam(coreSubagents(), config);

    expect(team.concurrencyPolicy).toEqual({
      maxParallelInstances: 16,
      staggerSeconds: 0,
      maxQueueWaitSeconds: 3600,
      batchExtrasSeparately: true,
      allowProviderCapacityQueue: false,
      allowBoundedAutoRetry: true,
      autoRetryElapsedGuardSeconds: 30,
    });
  });

  it('saves capacity and retry settings without changing unrelated review team config', async () => {
    vi.mocked(configAPI.getConfig).mockResolvedValueOnce(
      storedConfigWithExtra(['ExtraReviewer'], {
        strategy_level: 'deep',
        member_strategy_overrides: { ReviewSecurity: 'quick' },
        reviewer_timeout_seconds: 300,
      }),
    );

    await saveDefaultReviewTeamConcurrencyPolicy({
      maxParallelInstances: 2,
      staggerSeconds: 20,
      maxQueueWaitSeconds: 45,
      batchExtrasSeparately: false,
      allowProviderCapacityQueue: false,
      allowBoundedAutoRetry: true,
      autoRetryElapsedGuardSeconds: 240,
    });

    expect(configAPI.setConfig).toHaveBeenCalledWith(
      'ai.review_teams.default',
      expect.objectContaining({
        extra_subagent_ids: ['ExtraReviewer'],
        strategy_level: 'deep',
        member_strategy_overrides: { ReviewSecurity: 'quick' },
        reviewer_timeout_seconds: 300,
        max_parallel_reviewers: 2,
        max_queue_wait_seconds: 45,
        allow_provider_capacity_queue: false,
        allow_bounded_auto_retry: true,
        auto_retry_elapsed_guard_seconds: 240,
      }),
    );
  });

  it('propagates config errors that are not missing review team config paths', async () => {
    const error = new Error('Config service unavailable');
    vi.mocked(configAPI.getConfig).mockRejectedValueOnce(error);

    await expect(loadDefaultReviewTeamConfig()).rejects.toThrow(error.message);
  });

  it('loads cached review team rate limit status when available', async () => {
    vi.mocked(configAPI.getConfig).mockResolvedValueOnce({
      remaining: 3.8,
    });

    await expect(loadReviewTeamRateLimitStatus()).resolves.toEqual({
      remaining: 3,
    });
    expect(configAPI.getConfig).toHaveBeenCalledWith(
      'ai.review_team_rate_limit_status',
      { skipRetryOnNotFound: true },
    );
  });

  it('ignores missing or invalid cached review team rate limit status', async () => {
    vi.mocked(configAPI.getConfig)
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce({ remaining: 'not-a-number' })
      .mockRejectedValueOnce(new Error('rate status unavailable'));

    await expect(loadReviewTeamRateLimitStatus()).resolves.toBeNull();
    await expect(loadReviewTeamRateLimitStatus()).resolves.toBeNull();
    await expect(loadReviewTeamRateLimitStatus()).resolves.toBeNull();
  });

  it('only force-enables locked core members before launch', async () => {
    vi.mocked(configAPI.getConfig).mockResolvedValue(
      storedConfigWithExtra(['ExtraEnabled', 'ExtraDisabled']),
    );
    vi.mocked(SubagentAPI.listVisibleSubagents).mockResolvedValue([
      ...coreSubagents(false),
      subagent('ExtraEnabled', true, 'user', 'fast', true, true),
      subagent('ExtraDisabled', false, 'project', 'fast', true, true),
    ]);

    await prepareDefaultReviewTeamForLaunch(WORKSPACE_PATH);

    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledTimes(6);
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      parentAgentType: 'DeepReview',
      subagentId: 'ReviewBusinessLogic',
      enabled: true,
      workspacePath: WORKSPACE_PATH,
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      parentAgentType: 'DeepReview',
      subagentId: 'ReviewPerformance',
      enabled: true,
      workspacePath: WORKSPACE_PATH,
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      parentAgentType: 'DeepReview',
      subagentId: 'ReviewSecurity',
      enabled: true,
      workspacePath: WORKSPACE_PATH,
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      parentAgentType: 'DeepReview',
      subagentId: 'ReviewArchitecture',
      enabled: true,
      workspacePath: WORKSPACE_PATH,
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      parentAgentType: 'DeepReview',
      subagentId: 'ReviewFrontend',
      enabled: true,
      workspacePath: WORKSPACE_PATH,
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      parentAgentType: 'DeepReview',
      subagentId: 'ReviewJudge',
      enabled: true,
      workspacePath: WORKSPACE_PATH,
    });
    expect(SubagentAPI.updateSubagentConfig).not.toHaveBeenCalledWith(
      expect.objectContaining({ subagentId: 'ExtraEnabled' }),
    );
    expect(SubagentAPI.updateSubagentConfig).not.toHaveBeenCalledWith(
      expect.objectContaining({ subagentId: 'ExtraDisabled' }),
    );
  });

  it('excludes disabled extra members from the launch prompt', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraEnabled', true, 'user', 'fast', true, true),
        subagent('ExtraDisabled', false, 'project', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraEnabled', 'ExtraDisabled']),
    );

    const promptBlock = buildReviewTeamPromptBlock(team);

    expect(promptBlock).toContain('"subagent_type": "ExtraEnabled"');
    expect(promptBlock).not.toContain('"subagent_type": "ExtraDisabled"');
    expect(promptBlock).toContain('Launch only active_packets');
  });

  it('can resolve the team from a backend-provided reviewer definition', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ReviewDocs'),
      ],
      storedConfigWithExtra(['ReviewDocs']),
      {
        definition: {
          id: 'default-review-team',
          name: 'Strict Review Coverage',
          description: 'Backend-defined team',
          warning: 'Review may take longer.',
          defaultModel: 'fast',
          defaultStrategyLevel: 'normal',
          defaultExecutionPolicy: {
            reviewerTimeoutSeconds: 300,
            judgeTimeoutSeconds: 240,
            reviewerFileSplitThreshold: 20,
            maxSameRoleInstances: 3,
            maxRetriesPerRole: 1,
          },
          disallowedExtraSubagentIds: [
            'ReviewBusinessLogic',
            'ReviewPerformance',
            'ReviewSecurity',
            'ReviewArchitecture',
            'ReviewFrontend',
            'ReviewDocs',
            'ReviewJudge',
            'DeepReview',
            'ReviewFixer',
          ],
          hiddenAgentIds: [
            'DeepReview',
            'ReviewBusinessLogic',
            'ReviewPerformance',
            'ReviewSecurity',
            'ReviewArchitecture',
            'ReviewFrontend',
            'ReviewDocs',
            'ReviewJudge',
          ],
          coreRoles: [
            ...[
              'ReviewBusinessLogic',
              'ReviewPerformance',
              'ReviewSecurity',
              'ReviewArchitecture',
              'ReviewFrontend',
              'ReviewJudge',
            ].map((id) => ({
              key: id === 'ReviewJudge' ? 'judge' : id.replace(/^Review/, '').replace(/^BusinessLogic$/, 'businessLogic').toLowerCase(),
              subagentId: id,
              funName: id,
              roleName: id,
              description: `${id} description`,
              responsibilities: [`${id} responsibility`],
              accentColor: REVIEW_TEAM_MEMBER_ACCENT_DEFAULT,
              conditional: id === 'ReviewFrontend',
            })),
            {
              key: 'docs',
              subagentId: 'ReviewDocs',
              funName: 'Docs Reviewer',
              roleName: 'Documentation Reviewer',
              description: 'Checks docs and release notes.',
              responsibilities: ['Verify documentation stays aligned.'],
              accentColor: '#0f766e',
            },
          ],
          strategyProfiles: {
            ...REVIEW_STRATEGY_DEFINITIONS,
            quick: {
              ...REVIEW_STRATEGY_DEFINITIONS.quick,
              roleDirectives: {
                ...REVIEW_STRATEGY_DEFINITIONS.quick.roleDirectives,
                ReviewDocs: 'Only check changed docs.',
              },
            },
          },
        },
      },
    );

    expect(team.coreMembers.map((member) => member.subagentId)).toContain('ReviewDocs');
    expect(team.extraMembers.map((member) => member.subagentId)).not.toContain('ReviewDocs');

    const manifest = buildEffectiveReviewTeamManifest(team, {
      tokenBudgetMode: 'balanced',
    });
    expect(manifest.coreReviewers).toContainEqual(
      expect.objectContaining({
        subagentId: 'ReviewDocs',
        strategyDirective: REVIEW_STRATEGY_DEFINITIONS.normal.promptDirective,
      }),
    );
  });

  it('falls back safely when backend reviewer definition fields are malformed', async () => {
    vi.mocked(agentAPI.getDefaultReviewTeamDefinition).mockResolvedValue({
      id: 42,
      name: null,
      description: ['bad'],
      warning: {},
      defaultModel: 99,
      defaultStrategyLevel: 'normal',
      defaultExecutionPolicy: {
        reviewerTimeoutSeconds: 300,
        judgeTimeoutSeconds: 240,
        reviewerFileSplitThreshold: 20,
        maxSameRoleInstances: 3,
        maxRetriesPerRole: 1,
      },
      coreRoles: [],
      strategyProfiles: {},
      disallowedExtraSubagentIds: ['ReviewDocs', 42],
      hiddenAgentIds: ['ReviewDocs', null],
    });

    await expect(loadDefaultReviewTeamDefinition()).resolves.toMatchObject({
      id: FALLBACK_REVIEW_TEAM_DEFINITION.id,
      name: FALLBACK_REVIEW_TEAM_DEFINITION.name,
      description: FALLBACK_REVIEW_TEAM_DEFINITION.description,
      warning: FALLBACK_REVIEW_TEAM_DEFINITION.warning,
      defaultModel: FALLBACK_REVIEW_TEAM_DEFINITION.defaultModel,
      disallowedExtraSubagentIds: ['ReviewDocs'],
      hiddenAgentIds: ['ReviewDocs'],
    });
  });

  it('keeps invalid configured extra members explainable in the run manifest', () => {
    const readonlyReviewExtra = subagent('ExtraReadonlyReview', true, 'user', 'fast', true, true);
    const readonlyPlainExtra = subagent('ExtraReadonlyPlain', true, 'user', 'fast', true, false);
    const writableReviewExtra = subagent('ExtraWritableReview', true, 'project', 'fast', false, true);

    expect(canUseSubagentAsReviewTeamMember(readonlyReviewExtra)).toBe(true);
    expect(canUseSubagentAsReviewTeamMember(readonlyPlainExtra)).toBe(false);
    expect(canUseSubagentAsReviewTeamMember(writableReviewExtra)).toBe(false);

    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        readonlyReviewExtra,
        readonlyPlainExtra,
        writableReviewExtra,
      ],
      storedConfigWithExtra([
        'ExtraReadonlyReview',
        'ExtraReadonlyPlain',
        'ExtraWritableReview',
        'ExtraMissingReviewer',
      ]),
    );

    expect(
      team.extraMembers
        .filter((member) => member.available)
        .map((member) => member.subagentId),
    ).toEqual(['ExtraReadonlyReview']);

    const manifest = buildEffectiveReviewTeamManifest(team);

    expect(manifest.skippedReviewers).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          subagentId: 'ExtraReadonlyPlain',
          reason: 'invalid_tooling',
        }),
        expect.objectContaining({
          subagentId: 'ExtraWritableReview',
          reason: 'invalid_tooling',
        }),
        expect.objectContaining({
          subagentId: 'ExtraMissingReviewer',
          reason: 'unavailable',
        }),
      ]),
    );

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"subagent_type": "ExtraReadonlyReview"');
    expect(promptBlock).not.toContain('ExtraReadonlyPlain');
    expect(promptBlock).not.toContain('ExtraWritableReview');
    expect(promptBlock).not.toContain('ExtraMissingReviewer');
  });

  it('requires extra review members to have the minimum review tools', () => {
    const readyReviewExtra = subagent('ExtraReadyReview', true, 'user', 'fast', true, true);
    const missingDiffExtra = subagent(
      'ExtraMissingDiff',
      true,
      'user',
      'fast',
      true,
      true,
      ['Read', 'Grep'],
    );
    const missingReadExtra = subagent(
      'ExtraMissingRead',
      true,
      'project',
      'fast',
      true,
      true,
      ['GetFileDiff', 'Grep'],
    );

    expect(canUseSubagentAsReviewTeamMember(readyReviewExtra)).toBe(true);
    expect(canUseSubagentAsReviewTeamMember(missingDiffExtra)).toBe(false);
    expect(canUseSubagentAsReviewTeamMember(missingReadExtra)).toBe(false);

    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        readyReviewExtra,
        missingDiffExtra,
        missingReadExtra,
      ],
      storedConfigWithExtra(['ExtraReadyReview', 'ExtraMissingDiff', 'ExtraMissingRead']),
    );

    expect(
      team.extraMembers
        .filter((member) => member.available)
        .map((member) => member.subagentId),
    ).toEqual(['ExtraReadyReview']);

    const manifest = buildEffectiveReviewTeamManifest(team);

    expect(manifest.enabledExtraReviewers.map((member) => member.subagentId)).toEqual([
      'ExtraReadyReview',
    ]);
    expect(manifest.skippedReviewers).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          subagentId: 'ExtraMissingDiff',
          reason: 'invalid_tooling',
        }),
        expect.objectContaining({
          subagentId: 'ExtraMissingRead',
          reason: 'invalid_tooling',
        }),
      ]),
    );

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).not.toContain('ExtraMissingDiff');
    expect(promptBlock).not.toContain('ExtraMissingRead');
  });

  it('builds an explicit run manifest for enabled, skipped, and quality-gate reviewers', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraEnabled', true, 'user', 'fast', true, true),
        subagent('ExtraDisabled', false, 'project', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraEnabled', 'ExtraDisabled']),
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      workspacePath: WORKSPACE_PATH,
      policySource: 'default-review-team-config',
    });

    expect(manifest.reviewMode).toBe('deep');
    expect(manifest.strategyLevel).toBe('normal');
    expect(manifest.workspacePath).toBe(WORKSPACE_PATH);
    expect(manifest.policySource).toBe('default-review-team-config');
    expect(manifest.coreReviewers.map((member) => member.subagentId)).toEqual([
      'ReviewBusinessLogic',
      'ReviewPerformance',
      'ReviewSecurity',
      'ReviewArchitecture',
      'ReviewFrontend',
    ]);
    expect(manifest.qualityGateReviewer?.subagentId).toBe('ReviewJudge');
    expect(manifest.enabledExtraReviewers.map((member) => member.subagentId)).toEqual([
      'ExtraEnabled',
    ]);
    expect(manifest.skippedReviewers).toEqual([
      expect.objectContaining({
        subagentId: 'ExtraDisabled',
        reason: 'disabled',
      }),
    ]);
  });

  it('builds a bounded targeted manifest for an L2 review plan', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraEnabled', true, 'user', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraEnabled']),
    );
    const target = classifyReviewTargetFromFiles(
      ['src/apps/desktop/src/api/agentic_api.rs'],
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      qualityDecision: {
        level: 'l2',
        executionMode: 'strict',
        strategyLevel: 'normal',
        reason: 'risk_score',
        score: 8,
        requiresConsent: true,
      },
      maxCoreReviewers: 3,
      maxExtraReviewers: 0,
      includeQualityGate: false,
    });

    expect(manifest.coreReviewers.map((member) => member.subagentId)).toEqual([
      'ReviewBusinessLogic',
      'ReviewFrontend',
      'ReviewArchitecture',
    ]);
    expect(manifest.qualityGateReviewer).toBeUndefined();
    expect(manifest.qualityDecision).toMatchObject({ level: 'l2', score: 8 });
    expect(manifest.strategyDecision.userOverride).toBeUndefined();
    expect(manifest.enabledExtraReviewers).toEqual([]);
    expect(manifest.workPackets).toHaveLength(3);
    expect(manifest.skippedReviewers).toEqual(expect.arrayContaining([
      expect.objectContaining({
        subagentId: 'ReviewPerformance',
        reason: 'budget_limited',
      }),
      expect.objectContaining({
        subagentId: 'ReviewSecurity',
        reason: 'budget_limited',
      }),
      expect.objectContaining({
        subagentId: 'ExtraEnabled',
        reason: 'budget_limited',
      }),
    ]));
  });

  it('maps review strategies to explicit scope profiles in the run manifest', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );

    expect(buildEffectiveReviewTeamManifest(team, { strategyOverride: 'quick' }).scopeProfile)
      .toMatchObject({
        reviewDepth: 'high_risk_only',
        maxDependencyHops: 0,
        optionalReviewerPolicy: 'risk_matched_only',
        allowBroadToolExploration: false,
      });
    expect(buildEffectiveReviewTeamManifest(team, { strategyOverride: 'normal' }).scopeProfile)
      .toMatchObject({
        reviewDepth: 'risk_expanded',
        maxDependencyHops: 1,
        optionalReviewerPolicy: 'configured',
        allowBroadToolExploration: false,
      });
    expect(buildEffectiveReviewTeamManifest(team, { strategyOverride: 'deep' }).scopeProfile)
      .toMatchObject({
        reviewDepth: 'full_depth',
        maxDependencyHops: 'policy_limited',
        optionalReviewerPolicy: 'full',
        allowBroadToolExploration: true,
      });
  });

  it('keeps changed-file coverage metadata visible for focused-scope profiles', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra([], { strategy_level: 'quick' }),
    );
    const files = [
      'src/crates/assembly/core/src/agentic/deep_review/report.rs',
      'src/apps/desktop/src/api/agentic_api.rs',
      'src/web-ui/src/flow_chat/deep-review/action-bar/CapacityQueueNotice.tsx',
    ];

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target: classifyReviewTargetFromFiles(files, 'workspace_diff'),
    });

    expect(manifest.scopeProfile.reviewDepth).toBe('high_risk_only');
    expect(manifest.target.files.map((file) => file.normalizedPath)).toEqual(files);
    expect(
      manifest.workPackets
        ?.filter((packet) => packet.phase === 'reviewer')
        .every((packet) => packet.assignedScope.files.every((file) => files.includes(file))),
    ).toBe(true);
    expect(
      manifest.workPackets
        ?.filter((packet) => packet.phase === 'reviewer')
        .some((packet) => files.every((file) => packet.assignedScope.files.includes(file))),
    ).toBe(true);
  });

  it('includes focused-scope profile guidance in the prompt block', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra([], { strategy_level: 'quick' }),
    );

    const promptBlock = buildReviewTeamPromptBlock(team);

    expect(promptBlock).toContain('"review_depth": "high_risk_only"');
    expect(promptBlock).toContain('"max_dependency_hops": 0');
    expect(promptBlock).toContain('"coverage_expectation": "High-risk-only pass.');
    expect(promptBlock).not.toContain('optional_reviewer_policy');
    expect(promptBlock).not.toContain('allow_broad_tool_exploration');
    expect(promptBlock).toContain('evidence must remain an explicit coverage limitation');
  });

  it('generates structured work packets for active reviewers and the judge', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraEnabled', true, 'user', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraEnabled']),
    );
    const target = classifyReviewTargetFromFiles(
      ['src/web-ui/src/components/ReviewPanel.tsx'],
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      workspacePath: WORKSPACE_PATH,
      target,
    });

    const logicPacket = manifest.workPackets?.find(
      (packet) => packet.subagentId === 'ReviewBusinessLogic',
    );
    const judgePacket = manifest.workPackets?.find(
      (packet) => packet.subagentId === 'ReviewJudge',
    );

    expect(logicPacket).toMatchObject({
      packetId: 'reviewer:ReviewBusinessLogic',
      phase: 'reviewer',
      subagentId: 'ReviewBusinessLogic',
      roleName: 'Business Logic Reviewer',
      assignedScope: {
        kind: 'review_target',
        fileCount: 1,
        files: ['src/web-ui/src/components/ReviewPanel.tsx'],
      },
      allowedTools: ['GetFileDiff', 'Read', 'Grep', 'Glob', 'LS'],
      timeoutSeconds: manifest.executionPolicy.reviewerTimeoutSeconds,
      requiredOutputFields: expect.arrayContaining([
        'packet_id',
        'status',
        'findings',
      ]),
    });
    expect(judgePacket).toMatchObject({
      packetId: 'judge:ReviewJudge',
      phase: 'judge',
      subagentId: 'ReviewJudge',
      timeoutSeconds: manifest.executionPolicy.judgeTimeoutSeconds,
      requiredOutputFields: expect.arrayContaining([
        'packet_id',
        'status',
        'validated_findings',
      ]),
    });
    expect(manifest.workPackets?.map((packet) => packet.subagentId)).not.toContain(
      'ExtraDisabled',
    );
    expect(manifest.executionPolicy.maxRetriesPerRole).toBe(1);

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"active_packets"');
    expect(promptBlock).toContain('"packet_id": "reviewer:ReviewBusinessLogic"');
    expect(promptBlock).toContain('"allowed_tools"');
    expect(promptBlock).toContain('"max_retries_per_role": 1');
    expect(promptBlock).toContain('Retry a failed or timed-out role only when evidence is still missing');
    expect(promptBlock).toContain('Build each reviewer prompt from its active packet plus the referenced scope_group');
    expect(promptBlock).toContain('Infer missing packet_id only from the scheduled packet and mark it inferred.');
  });

  it('intersects extra reviewer tools with the read-only review tool set', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent(
          'ExtraEnabled',
          true,
          'user',
          'fast',
          true,
          true,
          ['Read', 'Grep', 'Git', 'Edit', 'Exec'],
        ),
      ],
      storedConfigWithExtra(['ExtraEnabled']),
    );

    expect(team.members.find((member) => member.subagentId === 'ExtraEnabled')?.allowedTools)
      .toEqual(['Read', 'Grep']);
  });

  it('adds a locale-only guardrail for i18n-only frontend review targets', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/web-ui/src/locales/zh-TW/flow-chat.json',
        'src/crates/assembly/core/locales/zh-TW.ftl',
        'BitFun-Installer/src/i18n/locales/zh-TW.json',
      ],
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      workspacePath: WORKSPACE_PATH,
      target,
    });
    const promptBlock = buildReviewTeamPromptBlock(team, manifest);

    expect(promptBlock).not.toContain('Locale-only review guardrail:');
    expect(promptBlock).not.toContain('placeholder parity');
    expect(promptBlock).not.toContain('Do not broaden into React performance');
    expect(promptBlock).toContain('BitFun-Installer/src/i18n/locales/zh-TW.json');
    expect(promptBlock.match(/BitFun-Installer\/src\/i18n\/locales\/zh-TW\.json/g)).toHaveLength(1);
    expect(promptBlock).not.toContain('BitFun-Installer/src/i18n/BitFun-Installer/src/i18n/locales/zh-TW.json');
  });

  it('does not add the locale-only guardrail for mixed locale and component targets', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/web-ui/src/locales/zh-TW/flow-chat.json',
        'src/web-ui/src/flow_chat/deep-review/action-bar/CapacityQueueNotice.tsx',
      ],
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      workspacePath: WORKSPACE_PATH,
      target,
    });
    const promptBlock = buildReviewTeamPromptBlock(team, manifest);

    expect(promptBlock).not.toContain('Locale-only review guardrail:');
  });

  it('pre-generates a compact diff summary for reviewer orientation', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/web-ui/src/shared/services/reviewTeamService.ts',
        'src/web-ui/src/flow_chat/deep-review/action-bar/CapacityQueueNotice.tsx',
        'src/web-ui/src/locales/en-US/scenes/agents.json',
        'src/crates/assembly/core/src/agentic/deep_review_policy.rs',
        'src/crates/assembly/core/src/agentic/tools/implementations/task_tool.rs',
      ],
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      changeStats: {
        totalLinesChanged: 420,
        lineCountSource: 'diff_stat',
      },
    });

    expect(manifest.preReviewSummary).toMatchObject({
      source: 'target_manifest',
      fileCount: 5,
      lineCount: 420,
      lineCountSource: 'diff_stat',
      workspaceAreas: [
        {
          key: 'web-ui',
          fileCount: 3,
          sampleFiles: [
            'src/web-ui/src/shared/services/reviewTeamService.ts',
            'src/web-ui/src/flow_chat/deep-review/action-bar/CapacityQueueNotice.tsx',
            'src/web-ui/src/locales/en-US/scenes/agents.json',
          ],
        },
        {
          key: 'crate:core',
          fileCount: 2,
          sampleFiles: [
            'src/crates/assembly/core/src/agentic/deep_review_policy.rs',
            'src/crates/assembly/core/src/agentic/tools/implementations/task_tool.rs',
          ],
        },
      ],
    });
    expect(manifest.preReviewSummary.summary).toContain(
      '5 files, 420 changed lines across 2 workspace areas',
    );

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).not.toContain('Pre-generated diff summary:');
    expect(promptBlock).not.toContain('"key": "web-ui"');
    expect(promptBlock).toContain('"changed_line_count": 420');
  });

  it('does not serialize speculative shared-context cache metadata', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/web-ui/src/shared/services/reviewTeamService.ts',
        'src/crates/assembly/core/src/agentic/deep_review_policy.rs',
      ],
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, { target });
    expect(manifest.sharedContextCache).toBeUndefined();
    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).not.toContain('Shared context cache plan:');
    expect(promptBlock).not.toContain('shared_context_cache');
  });

  it('builds a metadata-only evidence pack without source, diff, or model output', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraEnabled', true, 'user', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraEnabled']),
    );
    const files = [
      'src/web-ui/src/locales/en-US/flow-chat.json',
      'src/apps/desktop/src/api/agentic_api.rs',
      'src/crates/adapters/api-layer/src/review.rs',
      'package.json',
    ];

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target: classifyReviewTargetFromFiles(files, 'workspace_diff'),
      changeStats: {
        fileCount: files.length,
        totalLinesChanged: 120,
        lineCountSource: 'diff_stat',
      },
      strategyOverride: 'quick',
    });

    expect(manifest.evidencePack).toMatchObject({
      version: 1,
      source: 'target_manifest',
      changedFiles: files,
      diffStat: {
        fileCount: files.length,
        totalChangedLines: 120,
        lineCountSource: 'diff_stat',
      },
      riskFocusTags: manifest.scopeProfile?.riskFocusTags,
      packetIds: expect.arrayContaining([
        'reviewer:ReviewBusinessLogic',
        'judge:ReviewJudge',
      ]),
      privacy: {
        content: 'metadata_only',
      },
    });
    expect(manifest.evidencePack?.contractHints).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'i18n_key',
          filePath: 'src/web-ui/src/locales/en-US/flow-chat.json',
        }),
        expect.objectContaining({
          kind: 'tauri_command',
          filePath: 'src/apps/desktop/src/api/agentic_api.rs',
        }),
        expect.objectContaining({
          kind: 'api_contract',
          filePath: 'src/crates/adapters/api-layer/src/review.rs',
        }),
        expect.objectContaining({
          kind: 'config_key',
          filePath: 'package.json',
        }),
      ]),
    );
    expect(manifest.evidencePack?.hunkHints).toEqual(
      files.map((filePath) => ({
        filePath,
        changedLineCount: 30,
        lineCountSource: 'diff_stat',
      })),
    );

    const serializedEvidencePack = JSON.stringify(manifest.evidencePack);
    expect(serializedEvidencePack).not.toContain('promptDirective');
    expect(serializedEvidencePack).not.toContain('allowedTools');
    expect(serializedEvidencePack).not.toContain('fullDiff');
    expect(serializedEvidencePack).not.toContain('sourceText');
    expect(serializedEvidencePack).not.toContain('modelOutput');
  });

  it('keeps the evidence pack out of the repeated launch prompt', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const files = [
      'src/web-ui/src/locales/en-US/flow-chat.json',
      'src/crates/adapters/api-layer/src/review.rs',
    ];
    const manifest = buildEffectiveReviewTeamManifest(team, {
      target: classifyReviewTargetFromFiles(files, 'workspace_diff'),
      changeStats: {
        fileCount: files.length,
        totalLinesChanged: 20,
        lineCountSource: 'diff_stat',
      },
    });

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);

    expect(promptBlock).not.toContain('Evidence pack:');
    expect(promptBlock).not.toContain('"content": "metadata_only"');
    expect(promptBlock).not.toContain('"contract_hints"');
    expect(promptBlock).not.toContain('"Git"');
    expect(promptBlock).not.toContain('sourceText');
    expect(promptBlock).not.toContain('fullDiff');
    expect(promptBlock).not.toContain('modelOutput');
  });

  it('does not write a speculative incremental review cache plan', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/web-ui/src/shared/services/reviewTeamService.ts',
        'src/crates/assembly/core/src/agentic/deep_review_policy.rs',
      ],
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      changeStats: {
        totalLinesChanged: 128,
        lineCountSource: 'diff_stat',
      },
    });

    expect(manifest.incrementalReviewCache).toBeUndefined();
    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).not.toContain('Incremental review cache plan:');
    expect(promptBlock).not.toContain('incremental_review_cache');
  });

  it('splits reviewer work packets across file groups for large targets', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra([], {
        reviewer_file_split_threshold: 10,
        max_same_role_instances: 3,
      }),
    );
    const target = classifyReviewTargetFromFiles(
      Array.from(
        { length: 25 },
        (_, index) => `src/web-ui/src/components/ReviewPanel${index}.tsx`,
      ),
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      strategyOverride: 'deep',
      concurrencyPolicy: {
        maxParallelInstances: 16,
      },
    });
    const logicPackets = manifest.workPackets?.filter(
      (packet) => packet.subagentId === 'ReviewBusinessLogic',
    );
    const judgePackets = manifest.workPackets?.filter(
      (packet) => packet.subagentId === 'ReviewJudge',
    );

    expect(logicPackets).toHaveLength(3);
    expect(logicPackets?.map((packet) => packet.packetId)).toEqual([
      'reviewer:ReviewBusinessLogic:group-1-of-3',
      'reviewer:ReviewBusinessLogic:group-2-of-3',
      'reviewer:ReviewBusinessLogic:group-3-of-3',
    ]);
    expect(logicPackets?.map((packet) => packet.assignedScope.fileCount)).toEqual([
      9,
      8,
      8,
    ]);
    expect(logicPackets?.[0].assignedScope).toMatchObject({
      groupIndex: 1,
      groupCount: 3,
    });
    expect(logicPackets?.[0].assignedScope.files.slice(0, 2)).toEqual([
      'src/web-ui/src/components/ReviewPanel0.tsx',
      'src/web-ui/src/components/ReviewPanel1.tsx',
    ]);
    expect(logicPackets?.[0].assignedScope.files.at(-1)).toBe(
      'src/web-ui/src/components/ReviewPanel8.tsx',
    );
    expect(judgePackets).toHaveLength(1);
    expect(judgePackets?.[0].assignedScope).toMatchObject({
      fileCount: 25,
    });
    expect(judgePackets?.[0].assignedScope.groupCount).toBeUndefined();
    expect(manifest.tokenBudget).toMatchObject({
      estimatedReviewerCalls: 16,
      maxFilesPerReviewer: 10,
      largeDiffSummaryFirst: false,
    });

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"packet_id": "reviewer:ReviewBusinessLogic:group-1-of-3"');
    expect(promptBlock).toContain('"group_index": 1');
    expect(promptBlock).toContain('"group_count": 3');
  });

  it('keeps split reviewer work packets grouped by workspace area when possible', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra([], {
        reviewer_file_split_threshold: 4,
        max_same_role_instances: 3,
      }),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/web-ui/src/components/ReviewPanel.tsx',
        'src/crates/assembly/core/src/agentic/deep_review_policy.rs',
        'src/apps/desktop/src/api/review.rs',
        'src/web-ui/src/shared/services/reviewTeamService.ts',
        'src/crates/assembly/core/src/agentic/tools/implementations/task_tool.rs',
        'src/apps/desktop/src/api/agent.rs',
        'src/web-ui/src/flow_chat/deep-review/action-bar/CapacityQueueNotice.tsx',
        'src/crates/assembly/core/src/agentic/agents/deep_review_agent.rs',
        'src/apps/desktop/src/api/config.rs',
        'src/web-ui/src/locales/en-US/scenes/agents.json',
        'src/crates/assembly/core/src/agentic/agents/prompts/deep_review_agent.md',
        'src/apps/desktop/src/api/subagent.rs',
      ],
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      strategyOverride: 'deep',
      concurrencyPolicy: {
        maxParallelInstances: 16,
      },
    });
    const logicPackets = manifest.workPackets?.filter(
      (packet) => packet.subagentId === 'ReviewBusinessLogic',
    );

    expect(logicPackets).toHaveLength(3);
    expect(logicPackets?.map((packet) => packet.assignedScope.files)).toEqual([
      [
        'src/web-ui/src/components/ReviewPanel.tsx',
        'src/web-ui/src/shared/services/reviewTeamService.ts',
        'src/web-ui/src/flow_chat/deep-review/action-bar/CapacityQueueNotice.tsx',
        'src/web-ui/src/locales/en-US/scenes/agents.json',
      ],
      [
        'src/crates/assembly/core/src/agentic/deep_review_policy.rs',
        'src/crates/assembly/core/src/agentic/tools/implementations/task_tool.rs',
        'src/crates/assembly/core/src/agentic/agents/deep_review_agent.rs',
        'src/crates/assembly/core/src/agentic/agents/prompts/deep_review_agent.md',
      ],
      [
        'src/apps/desktop/src/api/review.rs',
        'src/apps/desktop/src/api/agent.rs',
        'src/apps/desktop/src/api/config.rs',
        'src/apps/desktop/src/api/subagent.rs',
      ],
    ]);

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"scope_groups"');
    expect(promptBlock).toContain('"scope_id": "scope-1"');
    expect(promptBlock).not.toContain('assigned_scope');
  });

  it('caps file splitting and launch batches by concurrency policy', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra([], {
        reviewer_file_split_threshold: 10,
        max_same_role_instances: 3,
      }),
    );
    const target = classifyReviewTargetFromFiles(
      Array.from(
        { length: 25 },
        (_, index) => `src/web-ui/src/components/ReviewPanel${index}.tsx`,
      ),
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, { target });
    const reviewerPackets = manifest.workPackets?.filter(
      (packet) => packet.phase === 'reviewer',
    ) ?? [];
    const logicPackets = reviewerPackets.filter(
      (packet) => packet.subagentId === 'ReviewBusinessLogic',
    );

    expect(manifest.concurrencyPolicy).toMatchObject({
      maxParallelInstances: 4,
      staggerSeconds: 0,
      maxQueueWaitSeconds: 1200,
      batchExtrasSeparately: true,
    });
    expect(logicPackets).toHaveLength(1);
    expect(logicPackets[0].assignedScope.groupCount).toBeUndefined();
    expect(reviewerPackets).toHaveLength(5);
    expect(reviewerPackets.slice(0, 4).map((packet) => packet.launchBatch)).toEqual([1, 1, 1, 1]);
    expect(reviewerPackets[4].launchBatch).toBe(2);
    expect(manifest.qualityGateReviewer && manifest.workPackets?.find(
      (packet) => packet.subagentId === manifest.qualityGateReviewer?.subagentId,
    )?.launchBatch).toBe(3);

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"max_parallel_instances": 4');
    expect(promptBlock).not.toContain('max_queue_wait_seconds');
    expect(promptBlock).toContain('in launch_batch order');
    expect(promptBlock).toContain('"launch_batch": 2');
  });

  it('keeps extra reviewers in a separate launch batch when requested', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ReviewProductExtra'),
      ],
      storedConfigWithExtra(['ReviewProductExtra'], {
        max_parallel_reviewers: 6,
      }),
    );
    const target = classifyReviewTargetFromFiles([
      'src/web-ui/src/components/ReviewPanel.tsx',
      'src/web-ui/src/components/ReviewPanel.css',
    ], 'session_files');

    const manifest = buildEffectiveReviewTeamManifest(team, { target });
    const reviewerPackets = manifest.workPackets?.filter(
      (packet) => packet.phase === 'reviewer',
    ) ?? [];
    const corePackets = reviewerPackets.filter(
      (packet) => packet.subagentId !== 'ReviewProductExtra',
    );
    const extraPackets = reviewerPackets.filter(
      (packet) => packet.subagentId === 'ReviewProductExtra',
    );

    expect(corePackets).toHaveLength(5);
    expect(extraPackets).toHaveLength(1);
    expect(corePackets.map((packet) => packet.launchBatch)).toEqual([1, 1, 1, 1, 1]);
    expect(extraPackets.map((packet) => packet.launchBatch)).toEqual([2]);
    expect(manifest.qualityGateReviewer && manifest.workPackets?.find(
      (packet) => packet.subagentId === manifest.qualityGateReviewer?.subagentId,
    )?.launchBatch).toBe(3);
  });

  it('reduces reviewer concurrency when rate limit remaining is tight', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra([], {
        reviewer_file_split_threshold: 10,
        max_same_role_instances: 3,
      }),
    );
    const target = classifyReviewTargetFromFiles(
      Array.from(
        { length: 25 },
        (_, index) => `src/web-ui/src/components/ReviewPanel${index}.tsx`,
      ),
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      rateLimitStatus: { remaining: 2 },
    });
    const reviewerPackets = manifest.workPackets?.filter(
      (packet) => packet.phase === 'reviewer',
    ) ?? [];

    expect(manifest.concurrencyPolicy).toMatchObject({
      maxParallelInstances: 2,
      staggerSeconds: 10,
      batchExtrasSeparately: true,
    });
    expect(reviewerPackets.map((packet) => packet.launchBatch)).toEqual([1, 1, 2, 2, 3]);
    expect(manifest.qualityGateReviewer && manifest.workPackets?.find(
      (packet) => packet.subagentId === manifest.qualityGateReviewer?.subagentId,
    )?.launchBatch).toBe(4);

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"max_parallel_instances": 2');
    expect(promptBlock).not.toContain('stagger_seconds');
  });

  it('skips the frontend reviewer when the resolved target has no frontend tags', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target: classifyReviewTargetFromFiles(
        ['src/crates/assembly/core/src/service/config/types.rs'],
        'session_files',
      ),
    });

    expect(manifest.target.resolution).toBe('resolved');
    expect(manifest.target.tags).toEqual(['backend_core']);
    expect(manifest.coreReviewers.map((member) => member.subagentId)).toEqual([
      'ReviewBusinessLogic',
      'ReviewPerformance',
      'ReviewSecurity',
      'ReviewArchitecture',
    ]);
    expect(manifest.skippedReviewers).toEqual([
      expect.objectContaining({
        subagentId: 'ReviewFrontend',
        reason: 'not_applicable',
      }),
    ]);
  });

  it('keeps explicit file-path targets compatible with conditional frontend reviewer gating', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      workspacePath: WORKSPACE_PATH,
      reviewTargetFilePaths: ['src/crates/assembly/core/src/agentic/deep_review_policy.rs'],
    });

    expect(manifest.coreReviewers.map((member) => member.subagentId)).toEqual([
      'ReviewBusinessLogic',
      'ReviewPerformance',
      'ReviewSecurity',
      'ReviewArchitecture',
    ]);
    expect(manifest.skippedReviewers).toEqual([
      expect.objectContaining({
        subagentId: 'ReviewFrontend',
        reason: 'not_applicable',
      }),
    ]);
  });

  it('runs the frontend reviewer for frontend and contract targets', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target: classifyReviewTargetFromFiles(
        ['src/apps/desktop/src/api/agentic_api.rs'],
        'session_files',
      ),
    });

    expect(manifest.target.tags).toEqual(
      expect.arrayContaining(['desktop_contract', 'frontend_contract']),
    );
    expect(manifest.coreReviewers.map((member) => member.subagentId)).toContain(
      'ReviewFrontend',
    );
    expect(manifest.skippedReviewers).not.toEqual([
      expect.objectContaining({ subagentId: 'ReviewFrontend' }),
    ]);
  });

  it('runs conditional reviewers conservatively for unknown targets', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target: createUnknownReviewTargetClassification('manual_prompt'),
    });

    expect(manifest.target.resolution).toBe('unknown');
    expect(manifest.coreReviewers.map((member) => member.subagentId)).toContain(
      'ReviewFrontend',
    );
  });

  it('adds a balanced token budget to the run manifest by default', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraEnabled', true, 'user', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraEnabled']),
    );

    const manifest = buildEffectiveReviewTeamManifest(team);

    expect(manifest.tokenBudget).toMatchObject({
      mode: 'balanced',
      estimatedReviewerCalls: 7,
      maxExtraReviewers: 1,
      skippedReviewerIds: [],
    });
    expect(manifest.tokenBudget.estimatedPromptBytesTotal).toBeUndefined();
    expect(manifest.tokenBudget.estimatedPromptBytesPerReviewer).toBeUndefined();
  });

  it('keeps quick strategy bounded and reduced-scope for slow-model friendly launches', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraEnabled', true, 'user', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraEnabled']),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/crates/assembly/core/src/service/auth/token_store.rs',
        'src/crates/adapters/api-layer/src/review.rs',
        'src/web-ui/src/components/ReviewPanel.tsx',
      ],
      'workspace_diff',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      strategyOverride: 'quick',
      changeStats: {
        fileCount: 3,
        totalLinesChanged: 220,
        lineCountSource: 'diff_stat',
      },
    });

    expect(manifest.tokenBudget).toMatchObject({
      mode: 'economy',
      maxExtraReviewers: 0,
      skippedReviewerIds: ['ExtraEnabled'],
    });
    expect(manifest.tokenBudget.maxReviewerCalls).toBe(
      manifest.tokenBudget.estimatedReviewerCalls,
    );
    expect(manifest.enabledExtraReviewers).toEqual([]);
    expect(manifest.executionPolicy).toMatchObject({
      reviewerTimeoutSeconds: 1200,
      judgeTimeoutSeconds: 900,
      reviewerFileSplitThreshold: 0,
      maxSameRoleInstances: 1,
    });
    expect(manifest.coreReviewers.map((member) => member.subagentId)).toEqual([
      'ReviewBusinessLogic',
      'ReviewSecurity',
      'ReviewArchitecture',
      'ReviewFrontend',
    ]);
    expect(manifest.scopeProfile).toMatchObject({
      reviewDepth: 'high_risk_only',
      optionalReviewerPolicy: 'risk_matched_only',
    });
    expect(manifest.workPackets.every((packet) =>
      packet.phase === 'judge' || packet.timeoutSeconds === 1200
    )).toBe(true);

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"review_depth": "high_risk_only"');
    expect(promptBlock).toContain('evidence must remain an explicit coverage limitation');
  });

  it('keeps normal strategy from expanding into long split-heavy launches on large targets', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraOne', true, 'user', 'fast', true, true),
        subagent('ExtraTwo', true, 'user', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraOne', 'ExtraTwo']),
    );
    const files = Array.from(
      { length: 48 },
      (_, index) => `src/crates/assembly/core/src/agentic/large_change_${index}.rs`,
    );
    const target = classifyReviewTargetFromFiles(files, 'workspace_diff');

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      strategyOverride: 'normal',
      concurrencyPolicy: {
        maxParallelInstances: 16,
      },
      changeStats: {
        fileCount: files.length,
        totalLinesChanged: 3200,
        lineCountSource: 'diff_stat',
      },
    });

    expect(manifest.tokenBudget).toMatchObject({
      mode: 'balanced',
      maxExtraReviewers: 1,
      skippedReviewerIds: ['ExtraTwo'],
      largeDiffSummaryFirst: false,
    });
    expect(manifest.enabledExtraReviewers.map((member) => member.subagentId)).toEqual([
      'ExtraOne',
    ]);
    expect(manifest.executionPolicy).toMatchObject({
      reviewerTimeoutSeconds: 1800,
      judgeTimeoutSeconds: 1200,
      reviewerFileSplitThreshold: 0,
      maxSameRoleInstances: 1,
    });
    expect(
      manifest.workPackets.filter(
        (packet) =>
          packet.phase === 'reviewer' &&
          packet.subagentId === 'ReviewBusinessLogic',
      ),
    ).toHaveLength(1);
  });

  it('keeps deep strategy thorough with long budget and same-role splitting', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      Array.from(
        { length: 25 },
        (_, index) => `src/web-ui/src/components/ReviewPanel${index}.tsx`,
      ),
      'workspace_diff',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      strategyOverride: 'deep',
      tokenBudgetMode: undefined,
      concurrencyPolicy: {
        maxParallelInstances: 16,
      },
      changeStats: {
        fileCount: 25,
        totalLinesChanged: 800,
        lineCountSource: 'diff_stat',
      },
    });

    expect(manifest.tokenBudget).toMatchObject({
      mode: 'thorough',
      maxExtraReviewers: 0,
    });
    expect(manifest.executionPolicy).toMatchObject({
      reviewerTimeoutSeconds: 3600,
      judgeTimeoutSeconds: 2400,
      reviewerFileSplitThreshold: 20,
      maxSameRoleInstances: 3,
    });
    expect(
      manifest.workPackets.filter(
        (packet) =>
          packet.phase === 'reviewer' &&
          packet.subagentId === 'ReviewBusinessLogic',
      ),
    ).toHaveLength(2);
  });

  it('does not invent prompt-byte pressure or hide assigned files', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const files = Array.from(
      { length: 6 },
      (_, index) => `src/crates/assembly/core/src/agentic/large_change_${index}.rs`,
    );
    const target = classifyReviewTargetFromFiles(files, 'workspace_diff');

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      changeStats: {
        fileCount: files.length,
        totalLinesChanged: 5000,
        lineCountSource: 'diff_stat',
      },
    });

    expect(manifest.tokenBudget).toMatchObject({
      largeDiffSummaryFirst: false,
    });
    expect(manifest.tokenBudget.maxPromptBytesPerReviewer).toBeUndefined();
    expect(manifest.tokenBudget.estimatedPromptBytesPerReviewer).toBeUndefined();
    expect(manifest.tokenBudget.promptByteEstimateSource).toBeUndefined();
    expect(manifest.tokenBudget.promptByteLimitExceeded).toBeUndefined();
    expect(manifest.tokenBudget.decisions).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({ kind: 'summary_first_full_scope' }),
      ]),
    );
    const reviewerPackets = manifest.workPackets.filter(
      (packet) => packet.phase === 'reviewer',
    );
    expect(reviewerPackets).not.toHaveLength(0);
    for (const packet of reviewerPackets) {
      expect(packet.assignedScope.files).toEqual(files);
    }

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).not.toContain('token_budget_decisions');
    expect(promptBlock).not.toContain('prompt_byte');
  });

  it('keeps summary-first disabled when split guardrails fit the prompt-byte budget', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra([], {
        reviewer_file_split_threshold: 4,
        max_same_role_instances: 2,
      }),
    );
    const files = Array.from(
      { length: 5 },
      (_, index) => `src/crates/assembly/core/src/agentic/small_${index}.rs`,
    );
    const target = classifyReviewTargetFromFiles(files, 'workspace_diff');

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      strategyOverride: 'deep',
      tokenBudgetMode: 'thorough',
      concurrencyPolicy: {
        maxParallelInstances: 8,
      },
      changeStats: {
        fileCount: files.length,
        totalLinesChanged: 25,
        lineCountSource: 'diff_stat',
      },
    });

    expect(manifest.tokenBudget).toMatchObject({
      maxFilesPerReviewer: 4,
      largeDiffSummaryFirst: false,
    });
    expect(manifest.tokenBudget.maxPromptBytesPerReviewer).toBeUndefined();
    expect(manifest.tokenBudget.promptByteLimitExceeded).toBeUndefined();
    expect(manifest.workPackets.filter((packet) => packet.phase === 'reviewer'))
      .toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            assignedScope: expect.objectContaining({
              groupCount: 2,
            }),
          }),
        ]),
      );

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).not.toContain('token_budget_decisions');
    expect(promptBlock).not.toContain('prompt_byte');
  });

  it('keeps normal manifest timeouts bounded for resolved target size', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      Array.from(
        { length: 25 },
        (_, index) => `src/web-ui/src/components/ReviewPanel${index}.tsx`,
      ),
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, { target });

    expect(manifest.changeStats).toMatchObject({
      fileCount: 25,
      lineCountSource: 'unknown',
    });
    expect(manifest.executionPolicy).toMatchObject({
      reviewerTimeoutSeconds: 1800,
      judgeTimeoutSeconds: 1200,
    });

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"file_count": 25');
    expect(promptBlock).toContain('"changed_line_count": null');
    expect(promptBlock).toContain('"timeout_seconds": 1800');
    expect(promptBlock).toContain('"timeout_seconds": 1200');
  });

  it('keeps normal manifest timeouts bounded even with diff line stats', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      Array.from(
        { length: 25 },
        (_, index) => `src/web-ui/src/components/ReviewPanel${index}.tsx`,
      ),
      'workspace_diff',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      changeStats: {
        fileCount: 25,
        totalLinesChanged: 800,
        lineCountSource: 'diff_stat',
      },
    });

    expect(manifest.changeStats).toMatchObject({
      fileCount: 25,
      totalLinesChanged: 800,
      lineCountSource: 'diff_stat',
    });
    expect(manifest.executionPolicy).toMatchObject({
      reviewerTimeoutSeconds: 1800,
      judgeTimeoutSeconds: 1200,
    });

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"changed_line_count": 800');
    expect(promptBlock).toContain('"changed_line_count_source": "diff_stat"');
    expect(promptBlock).toContain('"timeout_seconds": 1800');
    expect(promptBlock).toContain('"timeout_seconds": 1200');
  });

  it('keeps advisory risk recommendations in the manifest but out of the launch prompt', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/crates/assembly/core/src/service/auth/token_store.rs',
        'src/apps/desktop/src/api/agentic_api.rs',
        ...Array.from(
          { length: 18 },
          (_, index) => `src/web-ui/src/components/ReviewPanel${index}.tsx`,
        ),
      ],
      'workspace_diff',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      changeStats: {
        fileCount: 20,
        totalLinesChanged: 1400,
        lineCountSource: 'diff_stat',
      },
    });

    expect(manifest.strategyLevel).toBe('normal');
    expect(manifest.strategyRecommendation).toMatchObject({
      strategyLevel: 'deep',
      factors: {
        fileCount: 20,
        totalLinesChanged: 1400,
        securityFileCount: 1,
      },
    });
    expect(manifest.strategyRecommendation?.rationale).toContain('Large/high-risk change');
    expect(manifest.strategyDecision).toMatchObject({
      authority: 'mismatch_warning',
      teamDefaultStrategy: 'normal',
      finalStrategy: 'normal',
      mismatch: true,
      mismatchSeverity: 'medium',
      frontendRecommendation: {
        strategyLevel: 'deep',
      },
      backendRecommendation: {
        strategyLevel: 'deep',
        factors: {
          fileCount: 20,
          totalLinesChanged: 1400,
          filesInSecurityPaths: 1,
          maxCyclomaticComplexityDelta: 0,
          maxCyclomaticComplexityDeltaSource: 'not_measured',
        },
      },
    });

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"selected_strategy": "normal"');
    expect(promptBlock).not.toContain('recommended_strategy');
    expect(promptBlock).not.toContain('strategy_mismatch');
    expect(promptBlock).not.toContain('max_cyclomatic_complexity_delta_source');
    expect(promptBlock).not.toContain('Large/high-risk change');
  });

  it('records explicit strategy override as final strategy metadata without expanding reviewer roster', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraEnabled', true, 'user', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraEnabled']),
    );
    const target = classifyReviewTargetFromFiles(
      [
        ...Array.from(
          { length: 24 },
          (_, index) => `src/crates/assembly/core/src/review/module_${index}.rs`,
        ),
      ],
      'workspace_diff',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
      strategyOverride: 'quick',
      changeStats: {
        fileCount: 24,
        totalLinesChanged: 1800,
        lineCountSource: 'diff_stat',
      },
    });

    expect(manifest.strategyDecision).toMatchObject({
      authority: 'mismatch_warning',
      teamDefaultStrategy: 'normal',
      userOverride: 'quick',
      finalStrategy: 'quick',
      mismatch: true,
      mismatchSeverity: 'high',
      backendRecommendation: {
        strategyLevel: 'deep',
      },
    });
    expect(manifest.coreReviewers.map((member) => member.subagentId)).toEqual([
      'ReviewBusinessLogic',
    ]);
    expect(manifest.enabledExtraReviewers).toEqual([]);
    expect(manifest.tokenBudget).toMatchObject({
      mode: 'economy',
      skippedReviewerIds: ['ExtraEnabled'],
    });

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"selected_strategy": "quick"');
    expect(promptBlock).not.toContain('strategy_user_override');
    expect(promptBlock).not.toContain('strategy_mismatch');
  });

  it('keeps unknown targets at a conservative normal recommendation', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target: createUnknownReviewTargetClassification('manual_prompt'),
    });

    expect(manifest.strategyRecommendation).toMatchObject({
      strategyLevel: 'normal',
      score: 0,
    });
    expect(manifest.strategyRecommendation?.rationale).toContain('unresolved target');

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"selected_strategy": "normal"');
    expect(promptBlock).not.toContain('recommended_strategy');
  });

  it('preserves explicit zero timeout policy when predicting manifest timeouts', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra([], {
        reviewer_timeout_seconds: 0,
        judge_timeout_seconds: 0,
      }),
    );
    const target = classifyReviewTargetFromFiles(
      ['src/web-ui/src/components/ReviewPanel.tsx'],
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, { target });

    expect(manifest.executionPolicy).toMatchObject({
      reviewerTimeoutSeconds: 0,
      judgeTimeoutSeconds: 0,
    });
  });

  it('marks excess extra reviewers as budget-limited in economy mode', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraOne', true, 'user', 'fast', true, true),
        subagent('ExtraTwo', true, 'user', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraOne', 'ExtraTwo']),
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      tokenBudgetMode: 'economy',
    });

    expect(manifest.enabledExtraReviewers).toEqual([]);
    expect(manifest.skippedReviewers).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          subagentId: 'ExtraOne',
          reason: 'budget_limited',
        }),
        expect.objectContaining({
          subagentId: 'ExtraTwo',
          reason: 'budget_limited',
        }),
      ]),
    );
    expect(manifest.tokenBudget).toMatchObject({
      mode: 'economy',
      maxExtraReviewers: 0,
      skippedReviewerIds: ['ExtraOne', 'ExtraTwo'],
    });
  });

  it('applies per-member strategy overrides in the launch manifest and prompt', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraEnabled', true, 'user', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraEnabled'], {
        strategy_level: 'quick',
        member_strategy_overrides: {
          ReviewSecurity: 'deep',
          ExtraEnabled: 'normal',
        },
      }),
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      workspacePath: WORKSPACE_PATH,
    });

    expect(manifest.strategyLevel).toBe('quick');
    expect(manifest.coreReviewers).toEqual([
      expect.objectContaining({
        subagentId: 'ReviewBusinessLogic',
        strategyLevel: 'quick',
        strategySource: 'team',
        defaultModelSlot: 'fast',
        strategyDirective: REVIEW_STRATEGY_DEFINITIONS.quick.roleDirectives.ReviewBusinessLogic,
      }),
      expect.objectContaining({
        subagentId: 'ReviewSecurity',
        strategyLevel: 'deep',
        strategySource: 'member',
        model: 'primary',
        defaultModelSlot: 'primary',
        strategyDirective: REVIEW_STRATEGY_DEFINITIONS.deep.roleDirectives.ReviewSecurity,
      }),
      expect.objectContaining({
        subagentId: 'ReviewArchitecture',
        strategyLevel: 'quick',
        strategySource: 'team',
        defaultModelSlot: 'fast',
        strategyDirective: REVIEW_STRATEGY_DEFINITIONS.quick.roleDirectives.ReviewArchitecture,
      }),
      expect.objectContaining({
        subagentId: 'ReviewFrontend',
        strategyLevel: 'quick',
        strategySource: 'team',
        defaultModelSlot: 'fast',
        strategyDirective: REVIEW_STRATEGY_DEFINITIONS.quick.roleDirectives.ReviewFrontend,
      }),
    ]);
    expect(manifest.enabledExtraReviewers).toEqual([]);
    expect(manifest.skippedReviewers).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          subagentId: 'ExtraEnabled',
          reason: 'budget_limited',
          strategyLevel: 'normal',
          strategySource: 'member',
        }),
      ]),
    );

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"selected_strategy": "quick"');
    expect(promptBlock).toContain('Prepared Review execution plan');
    expect(promptBlock).toContain('Execution rules:');
    expect(promptBlock).toContain('"subagent_type": "ReviewSecurity"');
    expect(promptBlock).toContain('"strategy": "deep"');
    expect(promptBlock).toContain('"model_id": "primary"');
    expect(promptBlock).toContain(`"prompt_directive": ${JSON.stringify(REVIEW_STRATEGY_DEFINITIONS.deep.roleDirectives.ReviewSecurity)}`);
    expect(promptBlock).not.toContain('Token/time impact');
  });

  it('applies a project strategy override to the launch manifest without changing member overrides', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraEnabled', true, 'user', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraEnabled'], {
        strategy_level: 'normal',
        member_strategy_overrides: {
          ReviewSecurity: 'quick',
        },
      }),
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      workspacePath: WORKSPACE_PATH,
      strategyOverride: 'deep',
    });

    expect(manifest.strategyLevel).toBe('deep');
    expect(manifest.coreReviewers).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          subagentId: 'ReviewBusinessLogic',
          strategyLevel: 'deep',
          strategySource: 'team',
          defaultModelSlot: 'primary',
        }),
        expect.objectContaining({
          subagentId: 'ReviewSecurity',
          strategyLevel: 'quick',
          strategySource: 'member',
          defaultModelSlot: 'fast',
        }),
      ]),
    );
    expect(manifest.enabledExtraReviewers[0]).toMatchObject({
      subagentId: 'ExtraEnabled',
      strategyLevel: 'deep',
      strategySource: 'team',
      defaultModelSlot: 'primary',
    });

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('"selected_strategy": "deep"');
    expect(promptBlock).toContain('"subagent_type": "ReviewSecurity"');
    expect(promptBlock).toContain('"strategy": "quick"');
  });

  it('falls back removed concrete reviewer models to the strategy default model slot', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraDeletedModel', true, 'user', 'deleted-model', true, true),
        subagent('ExtraCustomModel', true, 'user', 'model-kept', true, true),
      ],
      storedConfigWithExtra(['ExtraDeletedModel', 'ExtraCustomModel'], {
        strategy_level: 'deep',
      }),
      { availableModelIds: ['model-kept'] },
    );

    const manifest = buildEffectiveReviewTeamManifest(team);
    const deletedModelMember = manifest.enabledExtraReviewers.find(
      (member) => member.subagentId === 'ExtraDeletedModel',
    );
    const customModelMember = manifest.enabledExtraReviewers.find(
      (member) => member.subagentId === 'ExtraCustomModel',
    );

    expect(deletedModelMember).toMatchObject({
      model: 'primary',
      configuredModel: 'deleted-model',
      modelFallbackReason: 'model_removed',
      strategyLevel: 'deep',
    });
    expect(customModelMember).toMatchObject({
      model: 'model-kept',
      configuredModel: 'model-kept',
      modelFallbackReason: undefined,
    });
  });

  it('renders the run manifest without scheduling disabled extra reviewers', () => {
    const team = resolveDefaultReviewTeam(
      [
        ...coreSubagents(),
        subagent('ExtraEnabled', true, 'user', 'fast', true, true),
        subagent('ExtraDisabled', false, 'project', 'fast', true, true),
      ],
      storedConfigWithExtra(['ExtraEnabled', 'ExtraDisabled']),
    );

    const promptBlock = buildReviewTeamPromptBlock(
      team,
      buildEffectiveReviewTeamManifest(team, {
        workspacePath: WORKSPACE_PATH,
      }),
    );

    expect(promptBlock).toContain('Prepared Review execution plan');
    expect(promptBlock).toContain('"resolution": "unknown"');
    expect(promptBlock).toContain('"selected_strategy": "normal"');
    expect(promptBlock).not.toContain(WORKSPACE_PATH);
    expect(promptBlock).toContain('"subagent_type": "ExtraEnabled"');
    expect(promptBlock).not.toContain('ExtraDisabled');
    expect(promptBlock).toContain('Launch only active_packets');
    expect(promptBlock).not.toContain('Configured code review team:');
    expect(promptBlock).not.toContain('Team execution rules:');
    expect(promptBlock).not.toContain('run it in parallel with the locked reviewers whenever the change contains frontend files');
  });

  it('distinguishes immutable Git ranges from mutable workspace evidence', () => {
    const team = resolveDefaultReviewTeam(coreSubagents(), storedConfigWithExtra());
    const target = classifyReviewTargetFromFiles(['src/lib.rs'], 'session_files');
    const baseEvidence = {
      version: 1 as const,
      fingerprint: '0123456789abcdef',
      completeness: 'complete' as const,
      workspaceBinding: 'matching_clean' as const,
      files: [{
        path: 'src/lib.rs',
        status: 'modified' as const,
        completeness: 'complete' as const,
      }],
      limitations: [],
    };

    const workspacePrompt = buildReviewTeamPromptBlock(
      team,
      buildEffectiveReviewTeamManifest(team, {
        workspacePath: WORKSPACE_PATH,
        target,
        targetEvidence: { ...baseEvidence, source: 'workspace' },
      }),
    );
    const gitRangePrompt = buildReviewTeamPromptBlock(
      team,
      buildEffectiveReviewTeamManifest(team, {
        workspacePath: WORKSPACE_PATH,
        target,
        targetEvidence: {
          ...baseEvidence,
          source: 'git_range',
          baseRevision: '1'.repeat(40),
          headRevision: '2'.repeat(40),
        },
      }),
    );

    expect(workspacePrompt).toContain('"source": "workspace"');
    expect(workspacePrompt).not.toContain('immutable');
    expect(gitRangePrompt).toContain('"source": "git_range"');
    expect(gitRangePrompt).toContain(`"base_revision": "${'1'.repeat(40)}"`);
    expect(gitRangePrompt).toContain(`"head_revision": "${'2'.repeat(40)}"`);
    expect(workspacePrompt).toContain('Do not reinterpret, widen, or replace the prepared target.');
    expect(gitRangePrompt).toContain('Do not reinterpret, widen, or replace the prepared target.');
  });

  it('tells DeepReview to wait for user approval before running ReviewFixer', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );

    const promptBlock = buildReviewTeamPromptBlock(team);

    expect(promptBlock).toContain('Remain read-only. Do not launch ReviewFixer or start remediation without explicit user approval.');
  });
});
