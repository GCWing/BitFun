import { describe, expect, it, vi, beforeEach } from 'vitest';
import {
  DEEP_REVIEW_SLASH_COMMAND,
  buildDeepReviewPromptFromSlashCommand,
  getDeepReviewLaunchErrorMessage,
  isDeepReviewSlashCommand,
  launchDeepReviewSession,
} from './DeepReviewService';

const mockDeleteSession = vi.fn();
const mockCreateBtwChildSession = vi.fn();
const mockOpenBtwSessionInAuxPane = vi.fn();
const mockCloseBtwSessionInAuxPane = vi.fn();
const mockSendMessage = vi.fn();
const mockDiscardLocalSession = vi.fn();
const mockInsertReviewSessionSummaryMarker = vi.fn();

vi.mock('@/infrastructure/api', () => ({
  agentAPI: {
    deleteSession: (...args: any[]) => mockDeleteSession(...args),
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
  prepareDefaultReviewTeamForLaunch: vi.fn(async () => ({ members: [] })),
  buildEffectiveReviewTeamManifest: vi.fn(() => ({ reviewers: [] })),
  buildReviewTeamPromptBlock: vi.fn(() => 'Review team manifest.'),
}));

describe('DeepReviewService slash command', () => {
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
