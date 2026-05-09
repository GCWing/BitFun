import { beforeEach, describe, expect, it, vi } from 'vitest';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import {
  DEFAULT_REVIEW_TEAM_EXECUTION_POLICY,
  DEFAULT_REVIEW_TEAM_STRATEGY_LEVEL,
  REVIEW_STRATEGY_DEFINITIONS,
  buildEffectiveReviewTeamManifest,
  buildReviewTeamPromptBlock,
  canUseSubagentAsReviewTeamMember,
  loadDefaultReviewTeamConfig,
  prepareDefaultReviewTeamForLaunch,
  resolveDefaultReviewTeam,
  type ReviewTeamStoredConfig,
} from './reviewTeamService';
import {
  SubagentAPI,
  type SubagentInfo,
} from '@/infrastructure/api/service-api/SubagentAPI';

vi.mock('@/infrastructure/api/service-api/ConfigAPI', () => ({
  configAPI: {
    getConfig: vi.fn(),
    setConfig: vi.fn(),
  },
}));

vi.mock('@/infrastructure/api/service-api/SubagentAPI', () => ({
  SubagentAPI: {
    listSubagents: vi.fn(),
    updateSubagentConfig: vi.fn(),
  },
}));

describe('reviewTeamService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

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
    ...overrides,
  });

  const subagent = (
    id: string,
    enabled = true,
    subagentSource: SubagentInfo['subagentSource'] = 'builtin',
    model = 'fast',
    isReadonly = true,
    isReview = id.startsWith('Review'),
  ): SubagentInfo => ({
    id,
    name: id,
    description: `${id} description`,
    isReadonly,
    isReview,
    toolCount: 1,
    defaultTools: ['Read'],
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

  it('propagates config errors that are not missing review team config paths', async () => {
    const error = new Error('Config service unavailable');
    vi.mocked(configAPI.getConfig).mockRejectedValueOnce(error);

    await expect(loadDefaultReviewTeamConfig()).rejects.toThrow(error.message);
  });

  it('only force-enables locked core members before launch', async () => {
    vi.mocked(configAPI.getConfig).mockResolvedValue(
      storedConfigWithExtra(['ExtraEnabled', 'ExtraDisabled']),
    );
    vi.mocked(SubagentAPI.listSubagents).mockResolvedValue([
      ...coreSubagents(false),
      subagent('ExtraEnabled', true, 'user', 'fast', true, true),
      subagent('ExtraDisabled', false, 'project', 'fast', true, true),
    ]);

    await prepareDefaultReviewTeamForLaunch('D:/workspace/project-a');

    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledTimes(6);
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      subagentId: 'ReviewBusinessLogic',
      enabled: true,
      workspacePath: 'D:/workspace/project-a',
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      subagentId: 'ReviewPerformance',
      enabled: true,
      workspacePath: 'D:/workspace/project-a',
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      subagentId: 'ReviewSecurity',
      enabled: true,
      workspacePath: 'D:/workspace/project-a',
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      subagentId: 'ReviewArchitecture',
      enabled: true,
      workspacePath: 'D:/workspace/project-a',
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      subagentId: 'ReviewFrontend',
      enabled: true,
      workspacePath: 'D:/workspace/project-a',
    });
    expect(SubagentAPI.updateSubagentConfig).toHaveBeenCalledWith({
      subagentId: 'ReviewJudge',
      enabled: true,
      workspacePath: 'D:/workspace/project-a',
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
    expect(promptBlock).toContain('Always run the four locked core reviewer roles');
    expect(promptBlock).not.toContain('Always run the three locked reviewer roles');
  });

  it('requires extra members to be explicitly marked for review and readonly', () => {
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
      storedConfigWithExtra(['ExtraReadonlyReview', 'ExtraReadonlyPlain', 'ExtraWritableReview']),
    );

    expect(team.extraMembers.map((member) => member.subagentId)).toEqual(['ExtraReadonlyReview']);

    const promptBlock = buildReviewTeamPromptBlock(team);
    expect(promptBlock).toContain('subagent_type: ExtraReadonlyReview');
    expect(promptBlock).not.toContain('ExtraReadonlyPlain');
    expect(promptBlock).not.toContain('ExtraWritableReview');
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
      workspacePath: 'D:/workspace/project-a',
      policySource: 'default-review-team-config',
    });

    expect(manifest.reviewMode).toBe('deep');
    expect(manifest.strategyLevel).toBe('normal');
    expect(manifest.workspacePath).toBe('D:/workspace/project-a');
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

  it('skips the conditional frontend reviewer when an explicit target has no frontend files', () => {
    const team = resolveDefaultReviewTeam(
      coreSubagents(),
      storedConfigWithExtra(),
    );

    const manifest = buildEffectiveReviewTeamManifest(team, {
      workspacePath: 'D:/workspace/project-a',
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
        reason: 'non_applicable',
      }),
    ]);
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
      workspacePath: 'D:/workspace/project-a',
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
        workspacePath: 'D:/workspace/project-a',
      }),
    );

    expect(promptBlock).toContain('Run manifest:');
    expect(promptBlock).toContain('- team_strategy: normal');
    expect(promptBlock).toContain('- workspace_path: D:/workspace/project-a');
    expect(promptBlock).toContain('quality_gate_reviewer: ReviewJudge');
    expect(promptBlock).toContain('enabled_extra_reviewers: ExtraEnabled');
    expect(promptBlock).toContain('skipped_reviewers:');
    expect(promptBlock).toContain('- ExtraDisabled: disabled');
    expect(promptBlock).not.toContain('subagent_type: ExtraDisabled');
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
