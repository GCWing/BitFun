import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockCreateSession = vi.fn();
const mockAskStream = vi.fn();
const mockAddExternalSession = vi.fn();
const mockUpdateSessionRelationship = vi.fn();
const mockUpdateSessionBtwOrigin = vi.fn();
const mockAddBtwThreadMarker = vi.fn();
const mockUpdateSessionModelName = vi.fn();
const mockAddDialogTurn = vi.fn();
const mockDeleteDialogTurn = vi.fn();
const mockCancelSessionTask = vi.fn();
const mockBtwCancel = vi.fn();
const mockTransition = vi.fn();

const sessions = new Map<string, any>();

vi.mock('@/infrastructure/api', () => ({
  agentAPI: {
    createSession: (...args: any[]) => mockCreateSession(...args),
  },
  btwAPI: {
    askStream: (...args: any[]) => mockAskStream(...args),
    cancel: (...args: any[]) => mockBtwCancel(...args),
  },
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({ sessions }),
    addExternalSession: (...args: any[]) => mockAddExternalSession(...args),
    updateSessionRelationship: (...args: any[]) => mockUpdateSessionRelationship(...args),
    updateSessionBtwOrigin: (...args: any[]) => mockUpdateSessionBtwOrigin(...args),
    addBtwThreadMarker: (...args: any[]) => mockAddBtwThreadMarker(...args),
    updateSessionModelName: (...args: any[]) => mockUpdateSessionModelName(...args),
    addDialogTurn: (...args: any[]) => mockAddDialogTurn(...args),
    deleteDialogTurn: (...args: any[]) => mockDeleteDialogTurn(...args),
    cancelSessionTask: (...args: any[]) => mockCancelSessionTask(...args),
  },
}));

vi.mock('../state-machine', () => ({
  SessionExecutionEvent: {
    START: 'start',
    FINISHING_SETTLED: 'finishing_settled',
  },
  stateMachineManager: {
    get: () => ({
      getContext: () => ({
        currentDialogTurnId: 'turn-parent-1',
      }),
    }),
    transition: (...args: any[]) => mockTransition(...args),
  },
}));

vi.mock('./FlowChatManager', () => ({
  flowChatManager: {
    discardLocalSession: vi.fn(),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    warning: vi.fn(),
  },
}));

import {
  cancelTransientBtwSession,
  createBtwChildSession,
  sendMessageToTransientBtwSession,
} from './BtwThreadService';

describe('BtwThreadService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    sessions.clear();
    sessions.set('parent-1', {
      sessionId: 'parent-1',
      mode: 'agentic',
      workspacePath: '/workspace',
      remoteConnectionId: 'remote-1',
      remoteSshHost: 'host-1',
      config: {
        modelName: 'primary',
      },
      dialogTurns: [
        {
          id: 'turn-parent-1',
        },
      ],
    });
    mockAskStream.mockResolvedValue({ ok: true });
    mockBtwCancel.mockResolvedValue(undefined);
    mockTransition.mockResolvedValue(true);
    mockCreateSession.mockResolvedValue({
      sessionId: 'child-1',
    });
  });

  it('passes structured relationship metadata to backend-created review sessions', async () => {
    const deepReviewRunManifest = {
      reviewers: [],
    };

    await createBtwChildSession({
      parentSessionId: 'parent-1',
      workspacePath: '/workspace',
      childSessionName: 'Deep review',
      sessionKind: 'deep_review',
      agentType: 'DeepReview',
      deepReviewRunManifest,
    });

    expect(mockCreateSession).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionName: 'Deep review',
        agentType: 'DeepReview',
        workspacePath: '/workspace',
        remoteConnectionId: 'remote-1',
        remoteSshHost: 'host-1',
        relationship: {
          kind: 'deep_review',
          parentSessionId: 'parent-1',
          parentRequestId: expect.any(String),
          parentDialogTurnId: 'turn-parent-1',
          parentTurnIndex: 1,
        },
        deepReviewRunManifest,
      }),
    );
  });

  it('passes image contexts through to the desktop /btw API', async () => {
    sessions.set('btw-child', {
      sessionId: 'btw-child',
      title: 'Side question',
      isTransient: true,
      sessionKind: 'btw',
      agentBackedTransient: false,
      config: { modelName: 'fast' },
    });

    await sendMessageToTransientBtwSession({
      parentSessionId: 'parent-1',
      childSessionId: 'btw-child',
      question: 'What is in this image?',
      imagePayload: {
        imageContexts: [
          {
            id: 'img-1',
            image_path: 'C:/tmp/clip.png',
            mime_type: 'image/png',
            metadata: { name: 'clip.png' },
          },
        ],
        imageDisplayData: [
          {
            id: 'img-1',
            name: 'clip.png',
            imagePath: 'C:/tmp/clip.png',
            mimeType: 'image/png',
          },
        ],
      },
    });

    expect(mockAskStream).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: 'parent-1',
        childSessionId: 'btw-child',
        question: 'What is in this image?',
        imageContexts: [
          expect.objectContaining({
            id: 'img-1',
            image_path: 'C:/tmp/clip.png',
            mime_type: 'image/png',
          }),
        ],
      }),
    );
    expect(mockAskStream.mock.calls[0][0]).not.toHaveProperty('modelId');
    expect(mockAddDialogTurn).toHaveBeenCalledWith(
      'btw-child',
      expect.objectContaining({
        id: expect.stringMatching(/^btw-turn-/),
        sessionId: 'btw-child',
        userMessage: expect.objectContaining({
          content: 'What is in this image?',
          hasImages: true,
          images: [
            expect.objectContaining({
              id: 'img-1',
              name: 'clip.png',
              imagePath: 'C:/tmp/clip.png',
            }),
          ],
        }),
        status: 'pending',
      }),
    );
    expect(mockUpdateSessionBtwOrigin).toHaveBeenCalledWith(
      'btw-child',
      expect.objectContaining({
        requestId: expect.any(String),
        parentSessionId: 'parent-1',
      }),
      'btw',
    );
  });

  it('cancels transient /btw sessions through the desktop /btw API', async () => {
    sessions.set('btw-child', {
      sessionId: 'btw-child',
      title: 'Side question',
      isTransient: true,
      sessionKind: 'btw',
      agentBackedTransient: false,
      config: { modelName: 'fast' },
      btwOrigin: {
        parentSessionId: 'parent-1',
        requestId: 'req-1',
      },
    });

    await expect(cancelTransientBtwSession('btw-child')).resolves.toBe(true);

    expect(mockBtwCancel).toHaveBeenCalledWith({ requestId: 'req-1' });
    expect(mockCancelSessionTask).not.toHaveBeenCalled();
  });

  it('removes the pending local turn and settles the state machine when /btw send fails', async () => {
    const error = new Error('backend refused');
    mockAskStream.mockRejectedValueOnce(error);
    sessions.set('btw-child', {
      sessionId: 'btw-child',
      title: 'Side question',
      isTransient: true,
      sessionKind: 'btw',
      agentBackedTransient: false,
      config: { modelName: 'fast' },
      dialogTurns: [],
    });

    await expect(sendMessageToTransientBtwSession({
      parentSessionId: 'parent-1',
      childSessionId: 'btw-child',
      question: 'Will this send?',
    })).rejects.toThrow('backend refused');

    expect(mockAddDialogTurn).toHaveBeenCalledWith(
      'btw-child',
      expect.objectContaining({
        id: expect.stringMatching(/^btw-turn-/),
      }),
    );
    const [, localTurn] = mockAddDialogTurn.mock.calls.at(-1)!;
    expect(mockDeleteDialogTurn).toHaveBeenCalledWith('btw-child', localTurn.id);
    expect(mockTransition).toHaveBeenCalledWith('btw-child', 'finishing_settled');
  });

  it('uses an explicit model override for /btw sends when provided', async () => {
    sessions.set('btw-child', {
      sessionId: 'btw-child',
      title: 'Side question',
      isTransient: true,
      sessionKind: 'btw',
      agentBackedTransient: false,
      config: { modelName: 'parent-multimodal-model' },
      dialogTurns: [],
    });

    await sendMessageToTransientBtwSession({
      parentSessionId: 'parent-1',
      childSessionId: 'btw-child',
      question: 'What is in this image?',
      modelId: 'parent-multimodal-model',
      imagePayload: {
        imageContexts: [
          {
            id: 'img-1',
            image_path: '/tmp/clip.png',
            mime_type: 'image/png',
          },
        ],
        imageDisplayData: [
          {
            id: 'img-1',
            name: 'clip.png',
            imagePath: '/tmp/clip.png',
            mimeType: 'image/png',
          },
        ],
      },
    });

    expect(mockAskStream).toHaveBeenCalledWith(
      expect.objectContaining({
        modelId: 'parent-multimodal-model',
      }),
    );
    expect(mockUpdateSessionModelName).toHaveBeenCalledWith(
      'btw-child',
      'parent-multimodal-model',
    );
  });
});
