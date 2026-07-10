import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';
import {
  launchPreparedReviewSession,
  prepareReviewLaunchFromSessionFiles,
  prepareReviewLaunchFromSlashCommand,
} from './ReviewService';

const mocks = vi.hoisted(() => ({
  buildDeepReviewLaunchFromSessionFiles: vi.fn(),
  buildDeepReviewLaunchFromSlashCommand: vi.fn(),
  launchDeepReviewSession: vi.fn(),
  loadProjectStrategyOverride: vi.fn(),
  resolveSlashCommandReviewTarget: vi.fn(),
  resolveCurrentFileReviewChangeStats: vi.fn(),
  createBtwChildSession: vi.fn(),
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

vi.mock('@/shared/services/reviewTeamService', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/shared/services/reviewTeamService')>();
  return {
    ...actual,
    loadReviewTeamProjectStrategyOverride: (...args: unknown[]) =>
      mocks.loadProjectStrategyOverride(...args),
  };
});

vi.mock('../deep-review/launch/targetResolver', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../deep-review/launch/targetResolver')>();
  return {
    ...actual,
    resolveSlashCommandReviewTarget: (...args: unknown[]) =>
      mocks.resolveSlashCommandReviewTarget(...args),
    resolveCurrentFileReviewChangeStats: (...args: unknown[]) =>
      mocks.resolveCurrentFileReviewChangeStats(...args),
  };
});

vi.mock('./BtwThreadService', () => ({
  createBtwChildSession: (...args: unknown[]) => mocks.createBtwChildSession(...args),
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

describe('ReviewService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.sessions.clear();
    mocks.loadProjectStrategyOverride.mockResolvedValue(undefined);
    mocks.createBtwChildSession.mockResolvedValue({
      childSessionId: 'review-child',
      parentDialogTurnId: 'turn-1',
    });
    mocks.sendMessage.mockResolvedValue(undefined);
    mocks.launchDeepReviewSession.mockResolvedValue({ childSessionId: 'strict-child' });
    mocks.deleteSession.mockResolvedValue(undefined);
    mocks.resolveCurrentFileReviewChangeStats.mockResolvedValue({
      fileCount: 1,
      lineCountSource: 'unknown',
    });
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
    mocks.resolveCurrentFileReviewChangeStats.mockResolvedValueOnce({
      fileCount: 2,
      totalLinesChanged: 3,
      lineCountSource: 'diff_stat',
    });

    await prepareReviewLaunchFromSessionFiles(
      ['src/auth.ts', 'src/helper.ts'],
      {
        workspacePath: 'D:/workspace/project',
        remoteConnectionId: 'remote-connection-1',
      },
    );

    expect(mocks.resolveCurrentFileReviewChangeStats).toHaveBeenCalledWith(
      'D:/workspace/project',
      expect.objectContaining({
        files: expect.arrayContaining([
          expect.objectContaining({ normalizedPath: 'src/auth.ts' }),
          expect.objectContaining({ normalizedPath: 'src/helper.ts' }),
        ]),
      }),
      undefined,
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
        changeStats: expect.objectContaining({ fileCount: 6 }),
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
    }));
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

  it('cleans up a standard review child when its first message fails', async () => {
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
    })).rejects.toThrow('send failed');

    expect(mocks.deleteSession).toHaveBeenCalledWith(
      'review-child',
      'D:/workspace/project',
      undefined,
      undefined,
    );
    expect(mocks.discardLocalSession).toHaveBeenCalledWith('review-child');
  });
});
