import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockCreateSession = vi.fn();
const mockAskStream = vi.fn();
const mockAddExternalSession = vi.fn();
const mockUpdateSessionRelationship = vi.fn();
const mockUpdateSessionBtwOrigin = vi.fn();
const mockAddBtwThreadMarker = vi.fn();
const mockUpdateSessionModelName = vi.fn();

const sessions = new Map<string, any>();

vi.mock('@/infrastructure/api', () => ({
  agentAPI: {
    createSession: (...args: any[]) => mockCreateSession(...args),
  },
  btwAPI: {
    askStream: (...args: any[]) => mockAskStream(...args),
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
  },
}));

vi.mock('../state-machine', () => ({
  stateMachineManager: {
    get: () => ({
      getContext: () => ({
        currentDialogTurnId: 'turn-parent-1',
      }),
    }),
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

import { createBtwChildSession, sendMessageToTransientBtwSession } from './BtwThreadService';

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
  });
});
