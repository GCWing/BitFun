import { beforeEach, describe, expect, it, vi } from 'vitest';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import {
  DEFAULT_REVIEW_TEAM_CONCURRENCY_POLICY,
  DEFAULT_REVIEW_TEAM_EXECUTION_POLICY,
  DEFAULT_REVIEW_TEAM_STRATEGY_LEVEL,
  FALLBACK_REVIEW_TEAM_DEFINITION,
  REVIEW_STRATEGY_DEFINITIONS,
  buildEffectiveReviewTeamManifest,
  buildReviewTeamPromptBlock,
  canUseSubagentAsReviewTeamMember,
  loadDefaultReviewTeamDefinition,
  loadDefaultReviewTeamConfig,
  loadReviewTeamProjectStrategyOverride,
  loadReviewTeamRateLimitStatus,
  lowerDefaultReviewTeamMaxParallelReviewers,
  prepareDefaultReviewTeamForLaunch,
  resolveDefaultReviewTeam,
  saveDefaultReviewTeamConcurrencyPolicy,
  saveReviewTeamProjectStrategyOverride,
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
    id,
    name: id,
    description: `${id} description`,
    isReadonly,
    isReview,
    toolCount: defaultTools.length,
    defaultTools,
    enabled,
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
      max_queue_wait_seconds: 999,
      allow_provider_capacity_queue: false,
      allow_bounded_auto_retry: true,
      auto_retry_elapsed_guard_seconds: 1,
    });

    const config = await loadDefaultReviewTeamConfig();
    const team = resolveDefaultReviewTeam(coreSubagents(), config);

    expect(team.concurrencyPolicy).toEqual({
      maxParallelInstances: 16,
      staggerSeconds: 0,
      maxQueueWaitSeconds: 600,
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

  it('lowers the next review max parallel reviewers without going below one', async () => {
    vi.mocked(configAPI.getConfig)
      .mockResolvedValueOnce(storedConfigWithExtra([], { max_parallel_reviewers: 3 }))
      .mockResolvedValueOnce(storedConfigWithExtra([], { max_parallel_reviewers: 1 }));

    await expect(lowerDefaultReviewTeamMaxParallelReviewers()).resolves.toMatchObject({
      maxParallelInstances: 2,
    });
    expect(configAPI.setConfig).toHaveBeenNthCalledWith(
      1,
      'ai.review_teams.default',
      expect.objectContaining({ max_parallel_reviewers: 2 }),
    );

    await expect(lowerDefaultReviewTeamMaxParallelReviewers()).resolves.toMatchObject({
      maxParallelInstances: 1,
    });
    expect(configAPI.setConfig).toHaveBeenNthCalledWith(
      2,
      'ai.review_teams.default',
      expect.objectContaining({ max_parallel_reviewers: 1 }),
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
      'ai.review_teams.rate_limit_status',
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

  it('loads project strategy overrides by normalized workspace path', async () => {
    vi.mocked(configAPI.getConfig).mockResolvedValueOnce({
      'd:/workspace/repo': 'deep',
      '/test-fixtures/project-a': 'quick',
      invalid: 'invalid',
    });

    await expect(
      loadReviewTeamProjectStrategyOverride('D:\\workspace\\repo'),
    ).resolves.toBe('deep');
    expect(configAPI.getConfig).toHaveBeenCalledWith(
      'ai.review_teams.project_strategy_overrides',
      { skipRetryOnNotFound: true },
    );
  });

  it('saves and clears project strategy overrides by normalized workspace path', async () => {
    vi.mocked(configAPI.getConfig)
      .mockResolvedValueOnce({
        'd:/workspace/repo': 'quick',
        '/test-fixtures/project-a': 'normal',
      })
      .mockResolvedValueOnce({
        'd:/workspace/repo': 'deep',
        '/test-fixtures/project-a': 'normal',
      });

    await saveReviewTeamProjectStrategyOverride('D:\\workspace\\repo', 'deep');
    expect(configAPI.setConfig).toHaveBeenNthCalledWith(
      1,
      'ai.review_teams.project_strategy_overrides',
      {
        'd:/workspace/repo': 'deep',
        '/test-fixtures/project-a': 'normal',
      },
    );

    await saveReviewTeamProjectStrategyOverride('D:\\workspace\\repo');
    expect(configAPI.setConfig).toHaveBeenNthCalledWith(
      2,
      'ai.review_teams.project_strategy_overrides',
      {
        '/test-fixtures/project-a': 'normal',
      },
    );
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
      subagentId: 'ReviewBusinessLogic',
      enabled: true,
      workspacePath: WORKSPACE_PATH,
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      subagentId: 'ReviewPerformance',
      enabled: true,
      workspacePath: WORKSPACE_PATH,
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      subagentId: 'ReviewSecurity',
      enabled: true,
      workspacePath: WORKSPACE_PATH,
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      subagentId: 'ReviewArchitecture',
      enabled: true,
      workspacePath: WORKSPACE_PATH,
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      subagentId: 'ReviewFrontend',
      enabled: true,
      workspacePath: WORKSPACE_PATH,
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
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

    expect(promptBlock).toContain('subagent_type: ExtraEnabled');
    expect(promptBlock).not.toContain('subagent_type: ExtraDisabled');
    expect(promptBlock).toContain('Run the active core reviewer roles first');
    expect(promptBlock).not.toContain('Always run the three locked reviewer roles');
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
          name: 'Code Review Team',
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
              accentColor: '#64748b',
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
    expect(promptBlock).toContain('subagent_type: ExtraReadonlyReview');
    expect(promptBlock).toContain('- ExtraReadonlyPlain: invalid_tooling');
    expect(promptBlock).toContain('- ExtraWritableReview: invalid_tooling');
    expect(promptBlock).toContain('- ExtraMissingReviewer: unavailable');
    expect(promptBlock).not.toContain('subagent_type: ExtraReadonlyPlain');
    expect(promptBlock).not.toContain('subagent_type: ExtraWritableReview');
    expect(promptBlock).not.toContain('subagent_type: ExtraMissingReviewer');
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
    expect(promptBlock).toContain('- ExtraMissingDiff: invalid_tooling');
    expect(promptBlock).toContain('- ExtraMissingRead: invalid_tooling');
    expect(promptBlock).not.toContain('subagent_type: ExtraMissingDiff');
    expect(promptBlock).not.toContain('subagent_type: ExtraMissingRead');
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

  it('keeps changed-file coverage metadata visible for reduced-depth scope profiles', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra([], { strategy_level: 'quick' }),
    );
    const files = [
      'src/crates/core/src/agentic/deep_review/report.rs',
      'src/apps/desktop/src/api/agentic_api.rs',
      'src/web-ui/src/app/scenes/agents/components/ReviewTeamPage.tsx',
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

  it('includes reduced-depth scope profile guidance in the prompt block', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra([], { strategy_level: 'quick' }),
    );

    const promptBlock = buildReviewTeamPromptBlock(team);

    expect(promptBlock).toContain('Scope profile:');
    expect(promptBlock).toContain('- review_depth: high_risk_only');
    expect(promptBlock).toContain('- max_dependency_hops: 0');
    expect(promptBlock).toContain('- optional_reviewer_policy: risk_matched_only');
    expect(promptBlock).toContain('- allow_broad_tool_exploration: no');
    expect(promptBlock).toContain('- coverage_expectation: High-risk-only pass.');
    expect(promptBlock).toContain('Reduced-depth profiles are not full-depth coverage.');
    expect(promptBlock).toContain('populate reliability_signals with reduced_scope');
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
      allowedTools: ['GetFileDiff', 'Read', 'Grep', 'Glob', 'LS', 'Git'],
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
    expect(promptBlock).toContain('Review work packets:');
    expect(promptBlock).toContain('"packet_id": "reviewer:ReviewBusinessLogic"');
    expect(promptBlock).toContain('"allowed_tools"');
    expect(promptBlock).toContain('- max_retries_per_role: 1');
    expect(promptBlock).toContain('set retry to true');
    expect(promptBlock).toContain('Each reviewer Task prompt must include the matching work packet verbatim.');
    expect(promptBlock).toContain('If the reviewer omits packet_id but the Task was launched from a packet, infer the packet_id from the Task description or work packet and mark packet_status_source as inferred.');
  });

  it('pre-generates a compact diff summary for reviewer orientation', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/web-ui/src/shared/services/reviewTeamService.ts',
        'src/web-ui/src/app/scenes/agents/components/ReviewTeamPage.tsx',
        'src/web-ui/src/locales/en-US/scenes/agents.json',
        'src/crates/core/src/agentic/deep_review_policy.rs',
        'src/crates/core/src/agentic/tools/implementations/task_tool.rs',
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
            'src/web-ui/src/app/scenes/agents/components/ReviewTeamPage.tsx',
            'src/web-ui/src/locales/en-US/scenes/agents.json',
          ],
        },
        {
          key: 'crate:core',
          fileCount: 2,
          sampleFiles: [
            'src/crates/core/src/agentic/deep_review_policy.rs',
            'src/crates/core/src/agentic/tools/implementations/task_tool.rs',
          ],
        },
      ],
    });
    expect(manifest.preReviewSummary.summary).toContain(
      '5 files, 420 changed lines across 2 workspace areas',
    );

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('Pre-generated diff summary:');
    expect(promptBlock).toContain('"key": "web-ui"');
    expect(promptBlock).toContain('Use the pre-generated diff summary');
  });

  it('builds a shared context cache plan for files consumed by multiple reviewers', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/web-ui/src/shared/services/reviewTeamService.ts',
        'src/crates/core/src/agentic/deep_review_policy.rs',
      ],
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, { target });
    const webUiCacheEntry = manifest.sharedContextCache.entries.find(
      (entry) => entry.path === 'src/web-ui/src/shared/services/reviewTeamService.ts',
    );

    expect(manifest.sharedContextCache).toMatchObject({
      source: 'work_packets',
      strategy: 'reuse_readonly_file_context_by_cache_key',
      omittedEntryCount: 0,
    });
    expect(webUiCacheEntry).toMatchObject({
      cacheKey: 'shared-context:1',
      workspaceArea: 'web-ui',
      recommendedTools: ['GetFileDiff', 'Read'],
      consumerPacketIds: expect.arrayContaining([
        'reviewer:ReviewBusinessLogic',
        'reviewer:ReviewPerformance',
        'reviewer:ReviewSecurity',
        'reviewer:ReviewArchitecture',
        'reviewer:ReviewFrontend',
      ]),
    });
    expect(webUiCacheEntry?.consumerPacketIds).not.toContain('judge:ReviewJudge');

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('Shared context cache plan:');
    expect(promptBlock).toContain('"cache_key": "shared-context:1"');
    expect(promptBlock).toContain('Use shared_context_cache entries');
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
      'src/crates/api-layer/src/review.rs',
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
          filePath: 'src/crates/api-layer/src/review.rs',
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

  it('injects the metadata-only evidence pack into the prompt as verifiable orientation', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const files = [
      'src/web-ui/src/locales/en-US/flow-chat.json',
      'src/crates/api-layer/src/review.rs',
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

    expect(promptBlock).toContain('Evidence pack:');
    expect(promptBlock).toContain('"content": "metadata_only"');
    expect(promptBlock).toContain('"changed_files"');
    expect(promptBlock).toContain('"contract_hints"');
    expect(promptBlock).toContain('Evidence pack hunk_hints and contract_hints are orientation only');
    expect(promptBlock).toContain('verify each hinted claim with GetFileDiff, Read, Grep, or Git before reporting it');
    expect(promptBlock).not.toContain('sourceText');
    expect(promptBlock).not.toContain('fullDiff');
    expect(promptBlock).not.toContain('modelOutput');
  });

  it('builds an incremental review cache plan for follow-up reviews', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/web-ui/src/shared/services/reviewTeamService.ts',
        'src/crates/core/src/agentic/deep_review_policy.rs',
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

    expect(manifest.incrementalReviewCache).toMatchObject({
      source: 'target_manifest',
      strategy: 'reuse_completed_packets_when_fingerprint_matches',
      filePaths: [
        'src/crates/core/src/agentic/deep_review_policy.rs',
        'src/web-ui/src/shared/services/reviewTeamService.ts',
      ],
      workspaceAreas: ['crate:core', 'web-ui'],
      lineCount: 128,
      lineCountSource: 'diff_stat',
      reviewerPacketIds: expect.arrayContaining([
        'reviewer:ReviewBusinessLogic',
        'reviewer:ReviewSecurity',
        'reviewer:ReviewFrontend',
      ]),
      invalidatesOn: expect.arrayContaining([
        'target_file_set_changed',
        'target_line_count_changed',
        'reviewer_roster_changed',
      ]),
    });
    expect(manifest.incrementalReviewCache.cacheKey).toMatch(/^incremental-review:/);
    expect(manifest.incrementalReviewCache.fingerprint).toHaveLength(8);
    expect(manifest.incrementalReviewCache.reviewerPacketIds).not.toContain('judge:ReviewJudge');

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('Incremental review cache plan:');
    expect(promptBlock).toContain('"strategy": "reuse_completed_packets_when_fingerprint_matches"');
    expect(promptBlock).toContain('Use incremental_review_cache only when the target fingerprint matches');
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
      largeDiffSummaryFirst: true,
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
        'src/crates/core/src/agentic/deep_review_policy.rs',
        'src/apps/desktop/src/api/review.rs',
        'src/web-ui/src/shared/services/reviewTeamService.ts',
        'src/crates/core/src/agentic/tools/implementations/task_tool.rs',
        'src/apps/desktop/src/api/agent.rs',
        'src/web-ui/src/app/scenes/agents/components/ReviewTeamPage.tsx',
        'src/crates/core/src/agentic/agents/deep_review_agent.rs',
        'src/apps/desktop/src/api/config.rs',
        'src/web-ui/src/locales/en-US/scenes/agents.json',
        'src/crates/core/src/agentic/agents/prompts/deep_review_agent.md',
        'src/apps/desktop/src/api/subagent.rs',
      ],
      'session_files',
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
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
        'src/web-ui/src/app/scenes/agents/components/ReviewTeamPage.tsx',
        'src/web-ui/src/locales/en-US/scenes/agents.json',
      ],
      [
        'src/crates/core/src/agentic/deep_review_policy.rs',
        'src/crates/core/src/agentic/tools/implementations/task_tool.rs',
        'src/crates/core/src/agentic/agents/deep_review_agent.rs',
        'src/crates/core/src/agentic/agents/prompts/deep_review_agent.md',
      ],
      [
        'src/apps/desktop/src/api/review.rs',
        'src/apps/desktop/src/api/agent.rs',
        'src/apps/desktop/src/api/config.rs',
        'src/apps/desktop/src/api/subagent.rs',
      ],
    ]);

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('Prefer module/workspace-area coherent file groups');
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
      maxQueueWaitSeconds: 60,
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
    expect(promptBlock).toContain('- max_parallel_instances: 4');
    expect(promptBlock).toContain('- max_queue_wait_seconds: 60');
    expect(promptBlock).toContain('Launch reviewer Tasks by launch_batch');
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
    expect(promptBlock).toContain('- max_parallel_instances: 2');
    expect(promptBlock).toContain('- stagger_seconds: 10');
  });

  it('skips the frontend reviewer when the resolved target has no frontend tags', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target: classifyReviewTargetFromFiles(
        ['src/crates/core/src/service/config/types.rs'],
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
      reviewTargetFilePaths: ['src/crates/core/src/agentic/deep_review_policy.rs'],
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
  });

  it('enables summary-first from prompt-byte pressure without hiding assigned files', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const files = Array.from(
      { length: 6 },
      (_, index) => `src/crates/core/src/agentic/large_change_${index}.rs`,
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
      maxPromptBytesPerReviewer: 96_000,
      promptByteEstimateSource: 'manifest_heuristic',
      promptByteLimitExceeded: true,
      largeDiffSummaryFirst: true,
    });
    expect(manifest.tokenBudget.estimatedPromptBytesPerReviewer).toBeGreaterThan(
      manifest.tokenBudget.maxPromptBytesPerReviewer ?? 0,
    );
    expect(manifest.tokenBudget.decisions).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'summary_first_full_scope',
          reason: 'prompt_bytes_exceeded',
        }),
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
    expect(promptBlock).toContain('- max_prompt_bytes_per_reviewer: 96000');
    expect(promptBlock).toContain('- prompt_byte_limit_exceeded: yes');
    expect(promptBlock).toContain('- token_budget_decisions: summary_first_full_scope');
    expect(promptBlock).toContain('Do not remove files from assigned_scope');
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
      (_, index) => `src/crates/core/src/agentic/small_${index}.rs`,
    );
    const target = classifyReviewTargetFromFiles(files, 'workspace_diff');

    const manifest = buildEffectiveReviewTeamManifest(team, {
      target,
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
      maxPromptBytesPerReviewer: 192_000,
      promptByteLimitExceeded: false,
      largeDiffSummaryFirst: false,
    });
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
    expect(promptBlock).toContain('- prompt_byte_limit_exceeded: no');
    expect(promptBlock).toContain('- token_budget_decisions: none');
  });

  it('predicts manifest timeouts from resolved target size', () => {
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
      reviewerTimeoutSeconds: 675,
      judgeTimeoutSeconds: 1350,
    });

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('- target_file_count: 25');
    expect(promptBlock).toContain('- target_line_count: unknown');
    expect(promptBlock).toContain('- reviewer_timeout_seconds: 675');
    expect(promptBlock).toContain('- judge_timeout_seconds: 1350');
  });

  it('includes diff line stats in predictive manifest timeouts', () => {
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
      reviewerTimeoutSeconds: 915,
      judgeTimeoutSeconds: 1830,
    });

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('- target_line_count: 800');
    expect(promptBlock).toContain('- target_line_count_source: diff_stat');
    expect(promptBlock).toContain('- reviewer_timeout_seconds: 915');
    expect(promptBlock).toContain('- judge_timeout_seconds: 1830');
  });

  it('adds an advisory risk-based strategy recommendation to the manifest and prompt', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );
    const target = classifyReviewTargetFromFiles(
      [
        'src/crates/core/src/service/auth/token_store.rs',
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
    expect(promptBlock).toContain('- recommended_strategy: deep');
    expect(promptBlock).toContain('- frontend_recommended_strategy: deep');
    expect(promptBlock).toContain('- backend_recommended_strategy: deep');
    expect(promptBlock).toContain('- strategy_authority: mismatch_warning');
    expect(promptBlock).toContain('- strategy_mismatch: yes');
    expect(promptBlock).toContain('- max_cyclomatic_complexity_delta_source: not_measured');
    expect(promptBlock).toContain('- strategy_recommendation_rationale: Large/high-risk change');
    expect(promptBlock).toContain('Risk recommendation is advisory');
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
          (_, index) => `src/crates/core/src/review/module_${index}.rs`,
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
    expect(manifest.coreReviewers).toHaveLength(4);
    expect(manifest.enabledExtraReviewers).toHaveLength(1);

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('- final_strategy: quick');
    expect(promptBlock).toContain('- strategy_user_override: quick');
    expect(promptBlock).toContain('- strategy_mismatch_severity: high');
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
    expect(promptBlock).toContain('- recommended_strategy: normal');
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
        subagentId: 'ReviewPerformance',
        strategyLevel: 'quick',
        strategySource: 'team',
        defaultModelSlot: 'fast',
        strategyDirective: REVIEW_STRATEGY_DEFINITIONS.quick.roleDirectives.ReviewPerformance,
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
    expect(manifest.enabledExtraReviewers[0]).toMatchObject({
      subagentId: 'ExtraEnabled',
      strategyLevel: 'normal',
      strategySource: 'member',
      defaultModelSlot: 'fast',
      strategyDirective: REVIEW_STRATEGY_DEFINITIONS.normal.promptDirective,
    });

    const promptBlock = buildReviewTeamPromptBlock(team, manifest);
    expect(promptBlock).toContain('- team_strategy: quick');
    expect(promptBlock).toContain('subagent_type: ReviewSecurity');
    expect(promptBlock).toContain('strategy: deep');
    expect(promptBlock).toContain('model_id: primary');
    expect(promptBlock).toContain(`prompt_directive: ${REVIEW_STRATEGY_DEFINITIONS.deep.roleDirectives.ReviewSecurity}`);
    expect(promptBlock).toContain('pass model_id with that value to the matching Task call');
    expect(promptBlock).toContain('Token/time impact: approximately 1.8-2.5x token usage and 1.5-2.5x runtime.');
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
    expect(promptBlock).toContain('- team_strategy: deep');
    expect(promptBlock).toContain('subagent_type: ReviewSecurity');
    expect(promptBlock).toContain('strategy: quick');
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

    expect(promptBlock).toContain('Run manifest:');
    expect(promptBlock).toContain('target_resolution: unknown');
    expect(promptBlock).toContain('- team_strategy: normal');
    expect(promptBlock).toContain(`- workspace_path: ${WORKSPACE_PATH}`);
    expect(promptBlock).toContain('quality_gate_reviewer: ReviewJudge');
    expect(promptBlock).toContain('enabled_extra_reviewers: ExtraEnabled');
    expect(promptBlock).toContain('skipped_reviewers:');
    expect(promptBlock).toContain('- ExtraDisabled: disabled');
    expect(promptBlock).not.toContain('subagent_type: ExtraDisabled');
    expect(promptBlock).toContain('Run only reviewers listed in core_reviewers and enabled_extra_reviewers.');
    expect(promptBlock).not.toContain('run it in parallel with the locked reviewers whenever the change contains frontend files');
  });

  it('tells DeepReview to wait for user approval before running ReviewFixer', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );

    const promptBlock = buildReviewTeamPromptBlock(team);

    expect(promptBlock).toContain('Do not run ReviewFixer during the review pass.');
    expect(promptBlock).toContain('Wait for explicit user approval before starting any remediation.');
  });
});
