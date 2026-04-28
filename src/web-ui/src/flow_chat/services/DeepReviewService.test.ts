import { describe, expect, it, vi, beforeEach } from 'vitest';
import {
  DEEP_REVIEW_SLASH_COMMAND,
  buildDeepReviewLaunchFromSlashCommand,
  buildDeepReviewPreviewFromSessionFiles,
  buildDeepReviewPromptFromSessionFiles,
  buildDeepReviewPromptFromSlashCommand,
  getDeepReviewLaunchErrorMessage,
  isDeepReviewSlashCommand,
  launchDeepReviewSession,
} from './DeepReviewService';
import { buildEffectiveReviewTeamManifest } from '@/shared/services/reviewTeamService';

const mockDeleteSession = vi.fn();
const mockCreateBtwChildSession = vi.fn();
const mockOpenBtwSessionInAuxPane = vi.fn();
const mockCloseBtwSessionInAuxPane = vi.fn();
const mockSendMessage = vi.fn();
const mockDiscardLocalSession = vi.fn();
const mockInsertReviewSessionSummaryMarker = vi.fn();
const mockGitGetStatus = vi.fn();
const mockGitGetChangedFiles = vi.fn();
const mockGitGetDiff = vi.fn();
const mockLoadDefaultReviewTeam = vi.fn();
const mockPrepareDefaultReviewTeamForLaunch = vi.fn();
const mockLoadReviewTeamRateLimitStatus = vi.fn();
const mockLoadReviewTeamProjectStrategyOverride = vi.fn();

vi.mock('@/infrastructure/api', () => ({
  agentAPI: {
    deleteSession: (...args: any[]) => mockDeleteSession(...args),
  },
  gitAPI: {
    getStatus: (...args: any[]) => mockGitGetStatus(...args),
    getChangedFiles: (...args: any[]) => mockGitGetChangedFiles(...args),
    getDiff: (...args: any[]) => mockGitGetDiff(...args),
  },
}));

vi.mock('./BtwThreadService', () => ({
  createBtwChildSession: (...args: any[]) => mockCreateBtwChildSession(...args),
}));

vi.mock('./openBtwSession', () => ({
  closeBtwSessionInAuxPane: (...args: any[]) => mockCloseBtwSessionInAuxPane(...args),
  openBtwSessionInAuxPane: (...args: any[]) => mockOpenBtwSessionInAuxPane(...args),
}));

vi.mock('./FlowChatManager', () => ({
  FlowChatManager: {
    getInstance: () => ({
      sendMessage: (...args: any[]) => mockSendMessage(...args),
      discardLocalSession: (...args: any[]) => mockDiscardLocalSession(...args),
    }),
  },
}));

const mockSessionsMap = new Map();
vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({ sessions: mockSessionsMap }),
  },
}));

vi.mock('./ReviewSessionMarkerService', () => ({
  insertReviewSessionSummaryMarker: (...args: any[]) => mockInsertReviewSessionSummaryMarker(...args),
}));

vi.mock('@/shared/services/reviewTeamService', () => ({
  loadDefaultReviewTeam: (...args: any[]) => mockLoadDefaultReviewTeam(...args),
  prepareDefaultReviewTeamForLaunch: (...args: any[]) => mockPrepareDefaultReviewTeamForLaunch(...args),
  loadReviewTeamRateLimitStatus: (...args: any[]) => mockLoadReviewTeamRateLimitStatus(...args),
  loadReviewTeamProjectStrategyOverride: (...args: any[]) => mockLoadReviewTeamProjectStrategyOverride(...args),
  buildEffectiveReviewTeamManifest: vi.fn(() => ({ reviewers: [] })),
  buildReviewTeamPromptBlock: vi.fn(() => 'Review team manifest.'),
}));

describe('DeepReviewService slash command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockLoadDefaultReviewTeam.mockResolvedValue({ members: [] });
    mockPrepareDefaultReviewTeamForLaunch.mockResolvedValue({ members: [] });
    mockLoadReviewTeamRateLimitStatus.mockResolvedValue(null);
    mockLoadReviewTeamProjectStrategyOverride.mockResolvedValue(undefined);
    mockGitGetStatus.mockResolvedValue({
      staged: [],
      unstaged: [],
      untracked: [],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    mockGitGetChangedFiles.mockResolvedValue([]);
    mockGitGetDiff.mockResolvedValue('');
  });

  it('uses /DeepReview as the canonical command', () => {
    expect(DEEP_REVIEW_SLASH_COMMAND).toBe('/DeepReview');
  });

  it('recognizes canonical deep review commands and rejects near matches', () => {
    expect(isDeepReviewSlashCommand('/DeepReview')).toBe(true);
    expect(isDeepReviewSlashCommand('/DeepReview review commit abc123')).toBe(true);
    expect(isDeepReviewSlashCommand('/deepreview review commit abc123')).toBe(false);
    expect(isDeepReviewSlashCommand('/DeepReviewer review commit abc123')).toBe(false);
  });

  it('strips the canonical command before building the focus block', async () => {
    const prompt = await buildDeepReviewPromptFromSlashCommand(
      '/DeepReview review commit abc123 for security',
      'D:\\workspace\\repo',
    );

    expect(prompt).toContain('Original command:\n/DeepReview review commit abc123 for security');
    expect(prompt).toContain('User-provided focus or target:\nreview commit abc123 for security');
    expect(prompt).not.toContain('User-provided focus or target:\n/DeepReview');
  });

  it('classifies explicit slash-command file paths before building the review team manifest', async () => {
    await buildDeepReviewPromptFromSlashCommand(
      '/DeepReview src/web-ui/src/App.tsx src/crates/core/src/service/config/types.rs for regressions',
      'D:\\workspace\\repo',
    );

    expect(buildEffectiveReviewTeamManifest).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        workspacePath: 'D:\\workspace\\repo',
        target: expect.objectContaining({
          source: 'slash_command_explicit_files',
          resolution: 'resolved',
          tags: expect.arrayContaining(['frontend_ui', 'backend_core']),
        }),
      }),
    );
  });

  it('classifies workspace diff files for a slash command without an explicit target', async () => {
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [{ path: 'src/web-ui/src/App.tsx', status: 'modified' }],
      unstaged: [{ path: 'src/crates/core/src/service/config/types.rs', status: 'modified' }],
      untracked: ['src/web-ui/src/newFeature.tsx'],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });

    await buildDeepReviewPromptFromSlashCommand(
      '/DeepReview',
      'D:\\workspace\\repo',
    );

    expect(mockGitGetStatus).toHaveBeenCalledWith('D:\\workspace\\repo');
    expect(buildEffectiveReviewTeamManifest).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        workspacePath: 'D:\\workspace\\repo',
        target: expect.objectContaining({
          source: 'workspace_diff',
          resolution: 'resolved',
          tags: expect.arrayContaining(['frontend_ui', 'backend_core']),
        }),
      }),
    );
  });

  it('passes workspace diff line stats into the review manifest', async () => {
    mockGitGetStatus.mockResolvedValueOnce({
      staged: [{ path: 'src/web-ui/src/App.tsx', status: 'modified' }],
      unstaged: [{ path: 'src/crates/core/src/service/config/types.rs', status: 'modified' }],
      untracked: [],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
    mockGitGetDiff.mockResolvedValueOnce([
      'diff --git a/src/crates/core/src/service/config/types.rs b/src/crates/core/src/service/config/types.rs',
      '@@ -1,2 +1,3 @@',
      '-old core line',
      '+new core line',
      '+another core line',
      'diff --git a/src/web-ui/src/App.tsx b/src/web-ui/src/App.tsx',
      '@@ -5,3 +5,2 @@',
      '-removed ui line',
      '+added ui line',
    ].join('\n'));

    await buildDeepReviewPromptFromSlashCommand(
      '/DeepReview',
      'D:\\workspace\\repo',
    );

    expect(mockGitGetDiff).toHaveBeenCalledWith('D:\\workspace\\repo', {
      source: 'HEAD',
    });
    expect(buildEffectiveReviewTeamManifest).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        changeStats: expect.objectContaining({
          fileCount: 2,
          totalLinesChanged: 5,
          lineCountSource: 'diff_stat',
        }),
      }),
    );
  });

  it('passes cached rate limit status into slash-command launch manifests', async () => {
    mockLoadReviewTeamRateLimitStatus.mockResolvedValueOnce({ remaining: 2 });

    await buildDeepReviewLaunchFromSlashCommand(
      '/DeepReview',
      'D:\\workspace\\repo',
    );

    expect(mockLoadReviewTeamRateLimitStatus).toHaveBeenCalled();
    expect(buildEffectiveReviewTeamManifest).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        workspacePath: 'D:\\workspace\\repo',
        rateLimitStatus: { remaining: 2 },
      }),
    );
  });

  it('does not block slash-command launch manifests when rate limit status is unavailable', async () => {
    mockLoadReviewTeamRateLimitStatus.mockRejectedValueOnce(new Error('rate status unavailable'));

    await buildDeepReviewLaunchFromSlashCommand(
      '/DeepReview',
      'D:\\workspace\\repo',
    );

    const lastCall = vi.mocked(buildEffectiveReviewTeamManifest).mock.calls.at(-1);
    expect(lastCall?.[1]).not.toHaveProperty('rateLimitStatus');
  });

  it('passes project strategy overrides into slash-command launch manifests', async () => {
    mockLoadReviewTeamProjectStrategyOverride.mockResolvedValueOnce('deep');

    await buildDeepReviewLaunchFromSlashCommand(
      '/DeepReview',
      'D:\\workspace\\repo',
    );

    expect(mockLoadReviewTeamProjectStrategyOverride).toHaveBeenCalledWith(
      'D:\\workspace\\repo',
    );
    expect(buildEffectiveReviewTeamManifest).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        workspacePath: 'D:\\workspace\\repo',
        strategyOverride: 'deep',
      }),
    );
  });

  it('does not block slash-command launch manifests when project strategy overrides are unavailable', async () => {
    mockLoadReviewTeamProjectStrategyOverride.mockRejectedValueOnce(new Error('strategy unavailable'));

    await buildDeepReviewLaunchFromSlashCommand(
      '/DeepReview',
      'D:\\workspace\\repo',
    );

    const lastCall = vi.mocked(buildEffectiveReviewTeamManifest).mock.calls.at(-1);
    expect(lastCall?.[1]).not.toHaveProperty('strategyOverride');
  });

  it('classifies commit target files through the git changed-files API', async () => {
    mockGitGetChangedFiles.mockResolvedValueOnce([
      {
        path: 'src/web-ui/src/App.tsx',
        old_path: undefined,
        status: 'modified',
      },
    ]);

    await buildDeepReviewPromptFromSlashCommand(
      '/DeepReview review commit abc123',
      'D:\\workspace\\repo',
    );

    expect(mockGitGetChangedFiles).toHaveBeenCalledWith('D:\\workspace\\repo', {
      source: 'abc123^',
      target: 'abc123',
    });
    expect(buildEffectiveReviewTeamManifest).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        target: expect.objectContaining({
          source: 'slash_command_git_ref',
          resolution: 'resolved',
          tags: expect.arrayContaining(['frontend_ui']),
        }),
      }),
    );
  });

  it('passes git ref diff line stats into the review manifest', async () => {
    mockGitGetChangedFiles.mockResolvedValueOnce([
      {
        path: 'src/web-ui/src/App.tsx',
        old_path: undefined,
        status: 'modified',
      },
    ]);
    mockGitGetDiff.mockResolvedValueOnce([
      'diff --git a/src/web-ui/src/App.tsx b/src/web-ui/src/App.tsx',
      '--- a/src/web-ui/src/App.tsx',
      '+++ b/src/web-ui/src/App.tsx',
      '@@ -10,2 +10,3 @@',
      '-old line',
      '+new line',
      '+new second line',
    ].join('\n'));

    await buildDeepReviewPromptFromSlashCommand(
      '/DeepReview review commit abc123',
      'D:\\workspace\\repo',
    );

    expect(mockGitGetDiff).toHaveBeenCalledWith('D:\\workspace\\repo', {
      source: 'abc123^',
      target: 'abc123',
    });
    expect(buildEffectiveReviewTeamManifest).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        changeStats: expect.objectContaining({
          fileCount: 1,
          totalLinesChanged: 3,
          lineCountSource: 'diff_stat',
        }),
      }),
    );
  });

  it('keeps line stats unknown when git diff stats fail', async () => {
    mockGitGetChangedFiles.mockResolvedValueOnce([
      {
        path: 'src/web-ui/src/App.tsx',
        old_path: undefined,
        status: 'modified',
      },
    ]);
    mockGitGetDiff.mockRejectedValueOnce(new Error('diff unavailable'));

    await buildDeepReviewPromptFromSlashCommand(
      '/DeepReview review commit abc123',
      'D:\\workspace\\repo',
    );

    expect(buildEffectiveReviewTeamManifest).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        changeStats: expect.objectContaining({
          fileCount: 1,
          lineCountSource: 'unknown',
        }),
      }),
    );
  });

  it('classifies explicit ref ranges through the git changed-files API', async () => {
    mockGitGetChangedFiles.mockResolvedValueOnce([
      {
        path: 'src/crates/core/src/service/config/types.rs',
        old_path: undefined,
        status: 'modified',
      },
    ]);

    await buildDeepReviewPromptFromSlashCommand(
      '/DeepReview review main..feature/deep-review',
      'D:\\workspace\\repo',
    );

    expect(mockGitGetChangedFiles).toHaveBeenCalledWith('D:\\workspace\\repo', {
      source: 'main',
      target: 'feature/deep-review',
    });
    expect(buildEffectiveReviewTeamManifest).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        target: expect.objectContaining({
          source: 'slash_command_git_ref',
          resolution: 'resolved',
          tags: expect.arrayContaining(['backend_core']),
        }),
      }),
    );
  });

  it('keeps git targets conservative when no workspace is available', async () => {
    await buildDeepReviewPromptFromSlashCommand(
      '/DeepReview review commit abc123',
    );

    expect(mockGitGetChangedFiles).not.toHaveBeenCalled();
    expect(buildEffectiveReviewTeamManifest).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        target: expect.objectContaining({
          source: 'slash_command_git_ref',
          resolution: 'unknown',
          tags: ['unknown'],
        }),
      }),
    );
  });

  it('returns the run manifest with the slash-command launch prompt', async () => {
    const runManifest = { reviewMode: 'deep', skippedReviewers: [] };
    vi.mocked(buildEffectiveReviewTeamManifest).mockReturnValueOnce(runManifest as any);

    const result = await buildDeepReviewLaunchFromSlashCommand(
      '/DeepReview review commit abc123',
      'D:\\workspace\\repo',
    );

    expect(result.prompt).toContain('Original command:\n/DeepReview review commit abc123');
    expect(result.runManifest).toBe(runManifest);
  });

  it('classifies session files before building the review team manifest', async () => {
    await buildDeepReviewPromptFromSessionFiles(
      ['src/web-ui/src/App.tsx'],
      undefined,
      'D:\\workspace\\repo',
    );

    expect(buildEffectiveReviewTeamManifest).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        workspacePath: 'D:\\workspace\\repo',
        target: expect.objectContaining({
          resolution: 'resolved',
          tags: expect.arrayContaining(['frontend_ui']),
        }),
      }),
    );
  });

  it('builds a read-only session-file preview without preparing launch state', async () => {
    const runManifest = {
      reviewMode: 'deep',
      skippedReviewers: [{ subagentId: 'ReviewFrontend', reason: 'not_applicable' }],
    };
    vi.mocked(buildEffectiveReviewTeamManifest).mockReturnValueOnce(runManifest as any);

    const result = await buildDeepReviewPreviewFromSessionFiles(
      ['src/crates/core/src/service/config/types.rs'],
      'D:\\workspace\\repo',
    );

    expect(result).toBe(runManifest);
    expect(mockLoadDefaultReviewTeam).toHaveBeenCalledWith('D:\\workspace\\repo');
    expect(mockPrepareDefaultReviewTeamForLaunch).not.toHaveBeenCalled();
    expect(buildEffectiveReviewTeamManifest).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        workspacePath: 'D:\\workspace\\repo',
        target: expect.objectContaining({
          source: 'session_files',
          resolution: 'resolved',
          tags: expect.arrayContaining(['backend_core']),
        }),
      }),
    );
  });
});

describe('launchDeepReviewSession', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockSessionsMap.clear();
  });

  it('returns child session ID on successful launch', async () => {
    mockCreateBtwChildSession.mockResolvedValue({
      childSessionId: 'child-123',
      parentDialogTurnId: 'turn-456',
    });
    mockSendMessage.mockResolvedValue(undefined);

    const result = await launchDeepReviewSession({
      parentSessionId: 'parent-123',
      workspacePath: 'D:\\workspace\\repo',
      prompt: 'Review these files',
      displayMessage: 'Deep review started',
    });

    expect(result.childSessionId).toBe('child-123');
    expect(mockCreateBtwChildSession).toHaveBeenCalledWith(
      expect.objectContaining({
        parentSessionId: 'parent-123',
        workspacePath: 'D:\\workspace\\repo',
        sessionKind: 'deep_review',
        agentType: 'DeepReview',
      }),
    );
    expect(mockOpenBtwSessionInAuxPane).toHaveBeenCalledWith(
      expect.objectContaining({ childSessionId: 'child-123' }),
    );
    expect(mockSendMessage).toHaveBeenCalledWith(
      'Review these files',
      'child-123',
      'Deep review started',
    );
    expect(mockInsertReviewSessionSummaryMarker).toHaveBeenCalledWith(
      expect.objectContaining({
        parentSessionId: 'parent-123',
        childSessionId: 'child-123',
        kind: 'deep_review',
      }),
    );
  });

  it('passes the run manifest into child session creation', async () => {
    const runManifest = { reviewMode: 'deep', skippedReviewers: [] };
    mockCreateBtwChildSession.mockResolvedValue({
      childSessionId: 'child-123',
      parentDialogTurnId: 'turn-456',
    });
    mockSendMessage.mockResolvedValue(undefined);

    await launchDeepReviewSession({
      parentSessionId: 'parent-123',
      workspacePath: 'D:\\workspace\\repo',
      prompt: 'Review these files',
      displayMessage: 'Deep review started',
      runManifest: runManifest as any,
    });

    expect(mockCreateBtwChildSession).toHaveBeenCalledWith(
      expect.objectContaining({
        deepReviewRunManifest: runManifest,
      }),
    );
  });

  it('passes the run manifest as first-turn message metadata', async () => {
    const runManifest = { reviewMode: 'deep', skippedReviewers: [] };
    mockCreateBtwChildSession.mockResolvedValue({
      childSessionId: 'child-123',
      parentDialogTurnId: 'turn-456',
    });
    mockSendMessage.mockResolvedValue(undefined);

    await launchDeepReviewSession({
      parentSessionId: 'parent-123',
      workspacePath: 'D:\\workspace\\repo',
      prompt: 'Review these files',
      displayMessage: 'Deep review started',
      runManifest: runManifest as any,
    });

    expect(mockSendMessage).toHaveBeenCalledWith(
      'Review these files',
      'child-123',
      'Deep review started',
      undefined,
      undefined,
      {
        userMessageMetadata: {
          deepReviewRunManifest: runManifest,
        },
      },
    );
  });

  it('throws and does not cleanup when createBtwChildSession fails', async () => {
    mockCreateBtwChildSession.mockRejectedValue(new Error('Session creation failed'));

    let caughtError: unknown;
    try {
      await launchDeepReviewSession({
        parentSessionId: 'parent-123',
        workspacePath: 'D:\\workspace\\repo',
        prompt: 'Review these files',
        displayMessage: 'Deep review started',
      });
    } catch (error) {
      caughtError = error;
    }

    expect(caughtError).toBeInstanceOf(Error);
    expect((caughtError as Error).message).toBe('Deep review failed to start. Please try again.');
    expect((caughtError as { launchErrorMessageKey?: string }).launchErrorMessageKey).toBe(
      'deepReviewActionBar.launchError.unknown',
    );
    expect(
      getDeepReviewLaunchErrorMessage(caughtError, (key: string) => `translated:${key}`),
    ).toBe('translated:deepReviewActionBar.launchError.unknown');

    expect(mockCloseBtwSessionInAuxPane).not.toHaveBeenCalled();
    expect(mockDeleteSession).not.toHaveBeenCalled();
    expect(mockDiscardLocalSession).not.toHaveBeenCalled();
  });

  it('throws and performs full cleanup when openBtwSessionInAuxPane fails', async () => {
    mockCreateBtwChildSession.mockResolvedValue({
      childSessionId: 'child-123',
      parentDialogTurnId: 'turn-456',
    });
    mockOpenBtwSessionInAuxPane.mockImplementation(() => {
      throw new Error('Pane open failed');
    });
    mockDeleteSession.mockResolvedValue(undefined);
    mockSessionsMap.set('child-123', { workspacePath: 'D:\\workspace\\repo' });

    await expect(
      launchDeepReviewSession({
        parentSessionId: 'parent-123',
        workspacePath: 'D:\\workspace\\repo',
        prompt: 'Review these files',
        displayMessage: 'Deep review started',
      }),
    ).rejects.toThrow('Pane open failed');

    expect(mockCloseBtwSessionInAuxPane).toHaveBeenCalledWith('child-123');
    expect(mockDeleteSession).toHaveBeenCalledWith(
      'child-123',
      'D:\\workspace\\repo',
      undefined,
      undefined,
    );
    expect(mockDiscardLocalSession).toHaveBeenCalledWith('child-123');
  });

  it('classifies sendMessage launch failures after cleanup', async () => {
    mockCreateBtwChildSession.mockResolvedValue({
      childSessionId: 'child-123',
      parentDialogTurnId: 'turn-456',
    });
    mockSendMessage.mockRejectedValue(new Error('SSE stream connection timeout'));
    mockDeleteSession.mockResolvedValue(undefined);
    mockSessionsMap.set('child-123', { workspacePath: 'D:\\workspace\\repo' });

    let caughtError: unknown;
    try {
      await launchDeepReviewSession({
        parentSessionId: 'parent-123',
        workspacePath: 'D:\\workspace\\repo',
        prompt: 'Review these files',
        displayMessage: 'Deep review started',
      });
    } catch (error) {
      caughtError = error;
    }

    expect(caughtError).toBeInstanceOf(Error);
    expect((caughtError as Error).message).toBe('Network connection was interrupted before Deep Review could start.');
    expect((caughtError as { launchErrorMessageKey?: string }).launchErrorMessageKey).toBe(
      'deepReviewActionBar.launchError.network',
    );
    expect((caughtError as { launchErrorCategory?: string }).launchErrorCategory).toBe('network');

    expect(mockCloseBtwSessionInAuxPane).toHaveBeenCalledWith('child-123');
    expect(mockDeleteSession).toHaveBeenCalled();
    expect(mockDiscardLocalSession).toHaveBeenCalledWith('child-123');
  });

  it('skips backend cleanup when workspace path is missing', async () => {
    mockCreateBtwChildSession.mockResolvedValue({
      childSessionId: 'child-123',
      parentDialogTurnId: 'turn-456',
    });
    mockOpenBtwSessionInAuxPane.mockImplementation(() => {
      throw new Error('Pane open failed');
    });
    // No workspacePath in session
    mockSessionsMap.set('child-123', {});

    await expect(
      launchDeepReviewSession({
        parentSessionId: 'parent-123',
        workspacePath: 'D:\\workspace\\repo',
        prompt: 'Review these files',
        displayMessage: 'Deep review started',
      }),
    ).rejects.toThrow('Pane open failed');

    expect(mockCloseBtwSessionInAuxPane).toHaveBeenCalledWith('child-123');
    expect(mockDeleteSession).not.toHaveBeenCalled();
    expect(mockDiscardLocalSession).not.toHaveBeenCalled();
  });

  it('treats session missing error as successful cleanup', async () => {
    mockCreateBtwChildSession.mockResolvedValue({
      childSessionId: 'child-123',
      parentDialogTurnId: 'turn-456',
    });
    mockOpenBtwSessionInAuxPane.mockImplementation(() => {
      throw new Error('Pane open failed');
    });
    mockDeleteSession.mockRejectedValue(new Error('Session does not exist'));
    mockSessionsMap.set('child-123', { workspacePath: 'D:\\workspace\\repo' });

    await expect(
      launchDeepReviewSession({
        parentSessionId: 'parent-123',
        workspacePath: 'D:\\workspace\\repo',
        prompt: 'Review these files',
        displayMessage: 'Deep review started',
      }),
    ).rejects.toThrow('Pane open failed');

    expect(mockDeleteSession).toHaveBeenCalled();
    // discardLocalSession should still be called because backend reports session missing
    expect(mockDiscardLocalSession).toHaveBeenCalledWith('child-123');
  });
});
