import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';
import {
  launchPreparedReviewSession,
  prepareReviewLaunchFromPullRequest,
  prepareReviewLaunchFromSessionFiles,
  prepareReviewLaunchFromSlashCommand,
} from './ReviewService';

const mocks = vi.hoisted(() => ({
  buildDeepReviewLaunchFromSessionFiles: vi.fn(),
  buildDeepReviewLaunchFromSlashCommand: vi.fn(),
  launchDeepReviewSession: vi.fn(),
  resolveSlashCommandReviewTarget: vi.fn(),
  resolveCurrentFileReviewSnapshot: vi.fn(),
  createBtwChildSession: vi.fn(),
  createBtwRequestId: vi.fn(),
  sendMessage: vi.fn(),
  insertReviewSessionSummaryMarker: vi.fn(),
  openBtwSessionInAuxPane: vi.fn(),
  closeBtwSessionInAuxPane: vi.fn(),
  decideReviewQuality: vi.fn(),
  deleteSession: vi.fn(),
  discardLocalSession: vi.fn(),
  sessions: new Map<string, unknown>(),
}));

vi.mock('@/infrastructure/api', () => ({
  agentAPI: {
    decideReviewQuality: (...args: unknown[]) => mocks.decideReviewQuality(...args),
    deleteSession: (...args: unknown[]) => mocks.deleteSession(...args),
  },
}));

vi.mock('./DeepReviewService', () => ({
  buildDeepReviewLaunchFromSessionFiles: (...args: unknown[]) =>
    mocks.buildDeepReviewLaunchFromSessionFiles(...args),
  buildDeepReviewLaunchFromSlashCommand: (...args: unknown[]) =>
    mocks.buildDeepReviewLaunchFromSlashCommand(...args),
  launchDeepReviewSession: (...args: unknown[]) => mocks.launchDeepReviewSession(...args),
}));

vi.mock('../deep-review/launch/targetResolver', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../deep-review/launch/targetResolver')>();
  return {
    ...actual,
    resolveSlashCommandReviewTarget: (...args: unknown[]) =>
      mocks.resolveSlashCommandReviewTarget(...args),
    resolveCurrentFileReviewSnapshot: (...args: unknown[]) =>
      mocks.resolveCurrentFileReviewSnapshot(...args),
  };
});

vi.mock('./BtwThreadService', () => ({
  createBtwChildSession: (...args: unknown[]) => mocks.createBtwChildSession(...args),
  createBtwRequestId: (...args: unknown[]) => mocks.createBtwRequestId(...args),
}));

vi.mock('./FlowChatManager', () => ({
  FlowChatManager: {
    getInstance: () => ({
      sendMessage: mocks.sendMessage,
      discardLocalSession: mocks.discardLocalSession,
    }),
  },
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({ sessions: mocks.sessions }),
  },
}));

vi.mock('./ReviewSessionMarkerService', () => ({
  insertReviewSessionSummaryMarker: (...args: unknown[]) =>
    mocks.insertReviewSessionSummaryMarker(...args),
}));

vi.mock('./btwSessionPane', () => ({
  openBtwSessionInAuxPane: (...args: unknown[]) => mocks.openBtwSessionInAuxPane(...args),
  closeBtwSessionInAuxPane: (...args: unknown[]) => mocks.closeBtwSessionInAuxPane(...args),
}));

function runManifest(strategyLevel: 'normal' | 'deep' = 'normal'): ReviewTeamRunManifest {
  return {
    reviewMode: 'deep',
    policySource: 'default-review-team-config',
    target: {
      source: 'session_files',
      resolution: 'resolved',
      tags: ['backend_core'],
      files: [{
        path: 'src/file.ts',
        normalizedPath: 'src/file.ts',
        status: 'modified',
        source: 'session_files',
        tags: ['backend_core'],
      }],
      warnings: [],
    },
    strategyLevel,
    strategyDecision: {} as ReviewTeamRunManifest['strategyDecision'],
    executionPolicy: {} as ReviewTeamRunManifest['executionPolicy'],
    concurrencyPolicy: {} as ReviewTeamRunManifest['concurrencyPolicy'],
    preReviewSummary: {} as ReviewTeamRunManifest['preReviewSummary'],
    sharedContextCache: {} as ReviewTeamRunManifest['sharedContextCache'],
    incrementalReviewCache: {} as ReviewTeamRunManifest['incrementalReviewCache'],
    tokenBudget: {} as ReviewTeamRunManifest['tokenBudget'],
    coreReviewers: [],
    enabledExtraReviewers: [],
    skippedReviewers: [],
  };
}

function targetEvidence() {
  return {
    version: 1 as const,
    source: 'workspace' as const,
    fingerprint: 'abc12345',
    baseRevision: '1'.repeat(40),
    headRevision: 'worktree:abc12345',
    completeness: 'complete' as const,
    workspaceBinding: 'matching_dirty' as const,
    files: [{
      path: 'src/file.ts',
      status: 'modified' as const,
      completeness: 'complete' as const,
    }],
    limitations: ['mutable_workspace_evidence'],
  };
}

describe('ReviewService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.sessions.clear();
    mocks.createBtwChildSession.mockResolvedValue({
      childSessionId: 'review-child',
      parentDialogTurnId: 'turn-1',
    });
    mocks.createBtwRequestId.mockReturnValue('generated-review-id');
    mocks.sendMessage.mockResolvedValue(undefined);
    mocks.launchDeepReviewSession.mockResolvedValue({ childSessionId: 'strict-child' });
    mocks.deleteSession.mockResolvedValue(undefined);
    mocks.resolveCurrentFileReviewSnapshot.mockImplementation(
      async (_workspacePath, target) => ({
        target,
        changeStats: {
          fileCount: 1,
          lineCountSource: 'unknown',
        },
        targetEvidence: targetEvidence(),
      }),
    );
    mocks.decideReviewQuality.mockResolvedValue({
      level: 'l1',
      executionMode: 'standard',
      strategyLevel: 'quick',
      reason: 'risk_score',
      score: 1,
      requiresConsent: false,
    });
  });

  it('prepares a small session review without constructing a review team', async () => {
    const prepared = await prepareReviewLaunchFromSessionFiles(
      ['src/small.ts'],
      {
        workspacePath: 'D:/workspace/project',
        changeStats: {
          fileCount: 1,
          totalLinesChanged: 4,
          lineCountSource: 'diff_stat',
        },
      },
    );

    expect(prepared.mode).toBe('standard');
    expect(prepared.level).toBe('l1');
    expect(prepared.prompt).toContain('independent adversarial review');
    expect(prepared.prompt).toContain('src/small.ts');
    expect(mocks.buildDeepReviewLaunchFromSessionFiles).not.toHaveBeenCalled();
  });

  it('measures the current diff for a file-scoped follow-up decision', async () => {
    mocks.resolveCurrentFileReviewSnapshot.mockImplementationOnce(
      async (_workspacePath, target) => ({
        target,
        changeStats: {
          fileCount: 2,
          totalLinesChanged: 3,
          lineCountSource: 'diff_stat',
        },
        targetEvidence: targetEvidence(),
      }),
    );

    await prepareReviewLaunchFromSessionFiles(
      ['src/auth.ts', 'src/helper.ts'],
      {
        workspacePath: 'D:/workspace/project',
        remoteConnectionId: 'remote-connection-1',
      },
    );

    expect(mocks.resolveCurrentFileReviewSnapshot).toHaveBeenCalledWith(
      'D:/workspace/project',
      expect.objectContaining({
        files: expect.arrayContaining([
          expect.objectContaining({ normalizedPath: 'src/auth.ts' }),
          expect.objectContaining({ normalizedPath: 'src/helper.ts' }),
        ]),
      }),
      'remote-connection-1',
    );
    expect(mocks.decideReviewQuality).toHaveBeenCalledWith(expect.objectContaining({
      target: expect.objectContaining({
        fileCount: 2,
        totalLinesChanged: 3,
      }),
    }));
  });

  it('prepares one immutable L2 launch for a medium target', async () => {
    mocks.decideReviewQuality.mockResolvedValueOnce({
      level: 'l2',
      executionMode: 'strict',
      strategyLevel: 'normal',
      reason: 'risk_score',
      score: 6,
      requiresConsent: true,
    });
    const manifest = runManifest('normal');
    mocks.buildDeepReviewLaunchFromSessionFiles.mockResolvedValue({
      prompt: 'team prompt',
      runManifest: manifest,
    });

    const files = Array.from({ length: 6 }, (_, index) => `src/file-${index}.ts`);
    const prepared = await prepareReviewLaunchFromSessionFiles(files, {
      workspacePath: 'D:/workspace/project',
      changeStats: {
        fileCount: files.length,
        totalLinesChanged: 20,
        lineCountSource: 'diff_stat',
      },
    });

    expect(prepared).toMatchObject({
      mode: 'strict',
      level: 'l2',
      strategyLevel: 'normal',
      prompt: 'team prompt',
      runManifest: manifest,
    });
    expect(mocks.buildDeepReviewLaunchFromSessionFiles).toHaveBeenCalledWith(
      files,
      undefined,
      'D:/workspace/project',
      expect.objectContaining({
        qualityDecision: expect.objectContaining({
          level: 'l2',
          strategyLevel: 'normal',
          reason: 'risk_score',
        }),
        resolvedTarget: expect.objectContaining({
          changeStats: expect.objectContaining({ fileCount: 6 }),
        }),
        maxCoreReviewers: 3,
        maxExtraReviewers: 0,
        includeQualityGate: false,
      }),
    );
  });

  it('maps legacy DeepReview commands to the explicit L3 path', async () => {
    mocks.decideReviewQuality.mockResolvedValueOnce({
      level: 'l3',
      executionMode: 'strict',
      strategyLevel: 'deep',
      reason: 'explicit_strict',
      score: 1,
      requiresConsent: true,
    });
    const manifest = runManifest('deep');
    mocks.resolveSlashCommandReviewTarget.mockResolvedValue({
      target: manifest.target,
      changeStats: {
        fileCount: 1,
        totalLinesChanged: 4,
        lineCountSource: 'diff_stat',
      },
      targetEvidence: targetEvidence(),
    });
    mocks.buildDeepReviewLaunchFromSlashCommand.mockResolvedValue({
      prompt: 'strict prompt',
      runManifest: manifest,
    });

    const prepared = await prepareReviewLaunchFromSlashCommand(
      '/DeepReview focus on auth',
      'D:/workspace/project',
    );

    expect(prepared).toMatchObject({
      mode: 'strict',
      level: 'l3',
      strategyLevel: 'deep',
      runManifest: manifest,
    });
  });

  it('rejects targets that exceed the evidence file boundary before quality selection', async () => {
    mocks.resolveCurrentFileReviewSnapshot.mockImplementationOnce(
      async (_workspacePath, target) => ({
        target,
        changeStats: { fileCount: 501, lineCountSource: 'unknown' },
        targetEvidence: {
          ...targetEvidence(),
          omittedFileCount: 1,
          completeness: 'partial' as const,
          limitations: ['target_file_limit_exceeded'],
        },
      }),
    );

    await expect(prepareReviewLaunchFromSessionFiles(
      ['src/file.ts'],
      { workspacePath: 'D:/workspace/project' },
    )).rejects.toThrow('exceeds the bounded evidence file limit');
    expect(mocks.decideReviewQuality).not.toHaveBeenCalled();
  });

  it('blocks remote Git ranges before spending reviewer capacity', async () => {
    const manifest = runManifest('normal');
    mocks.resolveSlashCommandReviewTarget.mockResolvedValue({
      target: {
        ...manifest.target,
        source: 'slash_command_git_ref',
      },
      changeStats: {
        fileCount: 1,
        lineCountSource: 'diff_stat',
        totalLinesChanged: 4,
      },
      targetEvidence: {
        ...targetEvidence(),
        source: 'git_range',
        headRevision: '2'.repeat(40),
        completeness: 'partial',
        workspaceBinding: 'unavailable',
        limitations: ['remote_exact_diff_unavailable'],
      },
    });

    await expect(prepareReviewLaunchFromSlashCommand(
      '/review main..feature',
      '/remote/workspace',
      'remote-1',
    )).rejects.toThrow('Remote Git range Review is not supported yet');
    expect(mocks.decideReviewQuality).not.toHaveBeenCalled();
    expect(mocks.buildDeepReviewLaunchFromSlashCommand).not.toHaveBeenCalled();
  });

  it('prepares a provider-bound pull request review without embedding the diff', async () => {
    const prepared = await prepareReviewLaunchFromPullRequest({
      workspacePath: 'D:/workspace/project',
      remote: {
        id: 'origin|https://github.com/example/repo.git',
        name: 'origin',
        url: 'https://github.com/example/repo.git',
        platform: 'github',
        host: 'github.com',
        owner: 'example',
        repositoryName: 'repo',
        projectPath: 'example/repo',
        webUrl: 'https://github.com/example/repo',
        supported: true,
        authState: 'connected',
        authSource: 'stored',
      },
      repository: {
        providerId: 'origin|https://github.com/example/repo.git',
        platform: 'github',
        host: 'github.com',
        owner: 'example',
        name: 'repo',
        projectPath: 'example/repo',
        defaultBranch: 'main',
        workspacePath: 'D:/workspace/project',
        webUrl: 'https://github.com/example/repo',
      },
      reviewTarget: {
        pullRequest: {
          id: '42',
          number: 42,
          title: 'Fix review target',
          state: 'open',
          author: 'alice',
          sourceBranch: 'feature',
          targetBranch: 'main',
          baseRevision: '1'.repeat(40),
          headRevision: '2'.repeat(40),
          updatedAt: '2026-07-11T00:00:00Z',
          webUrl: 'https://github.com/example/repo/pull/42',
          additions: 3,
          deletions: 1,
          changedFiles: 1,
          comments: 0,
          reviewDecision: 'pending',
          checks: { total: 0, passed: 0, failed: 0, pending: 0 },
        },
        files: [{
          path: 'src/lib.rs',
          status: 'modified',
          additions: 3,
          deletions: 1,
          diffAvailable: true,
        }],
        omittedFileCount: 0,
        limitations: [],
      },
    });

    expect(prepared.mode).toBe('standard');
    expect(prepared.targetEvidence.source).toBe('pull_request');
    expect(prepared.targetEvidence.pullRequest?.pullRequestId).toBe('42');
    expect(prepared.prompt).not.toContain('@@');
  });

  it('blocks remote workspace Review before spending reviewer capacity', async () => {
    const manifest = runManifest('normal');
    mocks.resolveSlashCommandReviewTarget.mockResolvedValue({
      target: { ...manifest.target, source: 'workspace_diff', resolution: 'unknown' },
      changeStats: { fileCount: 0, lineCountSource: 'unknown' },
      targetEvidence: {
        ...targetEvidence(),
        completeness: 'unknown',
        workspaceBinding: 'unavailable',
        files: [],
        limitations: ['remote_workspace_review_unavailable'],
      },
    });

    await expect(prepareReviewLaunchFromSlashCommand(
      '/review',
      '/remote/workspace',
      'remote-1',
    )).rejects.toThrow('Remote workspace Review is not supported');
    expect(mocks.decideReviewQuality).not.toHaveBeenCalled();
  });

  it('blocks an empty confirmed workspace snapshot before spending reviewer capacity', async () => {
    const manifest = runManifest('normal');
    mocks.resolveSlashCommandReviewTarget.mockResolvedValue({
      target: {
        ...manifest.target,
        source: 'workspace_diff',
        files: [],
      },
      changeStats: {
        fileCount: 0,
        lineCountSource: 'diff_stat',
        totalLinesChanged: 0,
      },
      targetEvidence: {
        ...targetEvidence(),
        files: [],
      },
    });

    await expect(prepareReviewLaunchFromSlashCommand(
      '/review',
      'D:/workspace/project',
    )).rejects.toThrow('There are no workspace changes to review.');
    expect(mocks.decideReviewQuality).not.toHaveBeenCalled();
  });

  it('blocks unresolved workspace evidence before spending reviewer capacity', async () => {
    const manifest = runManifest('normal');
    mocks.resolveSlashCommandReviewTarget.mockResolvedValue({
      target: {
        ...manifest.target,
        source: 'workspace_diff',
        resolution: 'unknown',
        files: [],
      },
      changeStats: { fileCount: 0, lineCountSource: 'unknown' },
      targetEvidence: {
        ...targetEvidence(),
        completeness: 'unknown',
        workspaceBinding: 'unavailable',
        files: [],
        limitations: ['review_target_unresolved'],
      },
    });

    await expect(prepareReviewLaunchFromSlashCommand('/review focus on auth'))
      .rejects.toThrow('could not be prepared as bounded evidence');
    expect(mocks.decideReviewQuality).not.toHaveBeenCalled();
  });

  it('launches standard review as a read-only CodeReview child in the shared pane', async () => {
    const prepared = await prepareReviewLaunchFromSessionFiles(['src/small.ts'], {
      workspacePath: 'D:/workspace/project',
      changeStats: {
        fileCount: 1,
        totalLinesChanged: 4,
        lineCountSource: 'diff_stat',
      },
    });

    await launchPreparedReviewSession({
      parentSessionId: 'parent',
      workspacePath: 'D:/workspace/project',
      displayMessage: 'Review current changes',
      requestId: 'review-follow-up-1',
      prepared,
    });

    expect(mocks.createBtwChildSession).toHaveBeenCalledWith(expect.objectContaining({
      parentSessionId: 'parent',
      sessionKind: 'review',
      agentType: 'CodeReview',
      childSessionName: 'Review',
      requestId: 'review-follow-up-1',
      reviewTargetEvidence: prepared.targetEvidence,
    }));
    expect(mocks.sendMessage).toHaveBeenCalledWith(
      expect.any(String),
      'review-child',
      'Review current changes',
      undefined,
      undefined,
      {
        turnId: 'review_turn_review-follow-up-1',
        preserveTurnOnStartError: true,
      },
    );
    expect(mocks.insertReviewSessionSummaryMarker).toHaveBeenCalledWith(expect.objectContaining({
      childSessionId: 'review-child',
      kind: 'review',
    }));
    expect(mocks.openBtwSessionInAuxPane).toHaveBeenCalledWith(expect.objectContaining({
      childSessionId: 'review-child',
      expand: true,
    }));
  });

  it('launches the exact prepared team manifest instead of rebuilding it after consent', async () => {
    const manifest = runManifest('normal');
    const prepared = {
      mode: 'strict' as const,
      level: 'l2' as const,
      strategyLevel: 'normal' as const,
      target: manifest.target,
      requestedFiles: ['src/file.ts'],
      prompt: 'prepared prompt',
      runManifest: manifest,
    };

    await launchPreparedReviewSession({
      parentSessionId: 'parent',
      workspacePath: 'D:/workspace/project',
      displayMessage: '/review',
      requestId: 'review-follow-up-2',
      prepared,
    });

    expect(mocks.launchDeepReviewSession).toHaveBeenCalledWith(expect.objectContaining({
      prompt: 'prepared prompt',
      runManifest: manifest,
      childSessionName: 'Review',
      requestId: 'review-follow-up-2',
    }));
    expect(mocks.buildDeepReviewLaunchFromSessionFiles).not.toHaveBeenCalled();
    expect(mocks.buildDeepReviewLaunchFromSlashCommand).not.toHaveBeenCalled();
  });

  it('preserves a standard review child when first-message acceptance is uncertain', async () => {
    const prepared = await prepareReviewLaunchFromSessionFiles(['src/small.ts'], {
      workspacePath: 'D:/workspace/project',
      changeStats: {
        fileCount: 1,
        totalLinesChanged: 4,
        lineCountSource: 'diff_stat',
      },
    });
    mocks.sessions.set('review-child', { workspacePath: 'D:/workspace/project' });
    mocks.sendMessage.mockRejectedValueOnce(new Error('send failed'));

    await expect(launchPreparedReviewSession({
      parentSessionId: 'parent',
      workspacePath: 'D:/workspace/project',
      displayMessage: '/review',
      prepared,
    })).resolves.toEqual({ childSessionId: 'review-child', launchStatus: 'uncertain' });

    expect(mocks.deleteSession).not.toHaveBeenCalled();
    expect(mocks.discardLocalSession).not.toHaveBeenCalled();
    expect(mocks.insertReviewSessionSummaryMarker).toHaveBeenCalledWith(
      expect.objectContaining({ childSessionId: 'review-child', kind: 'review' }),
    );
    expect(mocks.openBtwSessionInAuxPane).toHaveBeenCalledWith(
      expect.objectContaining({ childSessionId: 'review-child', sessionKind: 'review' }),
    );
  });

  it('retries uncertain standard child creation with the same request id', async () => {
    const prepared = await prepareReviewLaunchFromSessionFiles(['src/small.ts'], {
      workspacePath: 'D:/workspace/project',
      changeStats: {
        fileCount: 1,
        totalLinesChanged: 4,
        lineCountSource: 'diff_stat',
      },
    });
    mocks.createBtwChildSession
      .mockRejectedValueOnce(new Error('Create acknowledgement lost'))
      .mockResolvedValueOnce({
        childSessionId: 'review-child',
        parentDialogTurnId: 'turn-1',
      });

    await expect(launchPreparedReviewSession({
      parentSessionId: 'parent',
      workspacePath: 'D:/workspace/project',
      displayMessage: '/review',
      prepared,
    })).resolves.toEqual({ childSessionId: 'review-child', launchStatus: 'started' });

    expect(mocks.createBtwChildSession).toHaveBeenCalledTimes(2);
    expect(mocks.createBtwChildSession.mock.calls[0][0].requestId).toBe(
      'generated-review-id',
    );
    expect(mocks.createBtwChildSession.mock.calls[1][0].requestId).toBe(
      'generated-review-id',
    );
  });
});
