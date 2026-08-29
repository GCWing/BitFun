// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  pendingQueueManager,
  queuedItemDuplicatesLiveTurn,
} from './PendingQueueModule';
import {
  LOCAL_SURFACE_ID,
  activateSurface,
} from '@/infrastructure/peer-device/deviceSurface';

const sessions: string[] = [];

function testSession(): string {
  const sessionId = `pending-queue-test-${sessions.length}`;
  sessions.push(sessionId);
  return sessionId;
}

beforeEach(() => {
  activateSurface(LOCAL_SURFACE_ID);
});

afterEach(() => {
  activateSurface(LOCAL_SURFACE_ID);
  for (const sessionId of sessions.splice(0)) {
    pendingQueueManager.clear(sessionId);
  }
});

describe('PendingQueueModule', () => {
  it('promotes an existing item for explicit drain without rebuilding or losing its payload', () => {
    const sessionId = testSession();
    pendingQueueManager.enqueue({ sessionId, content: 'first' });
    const target = pendingQueueManager.enqueue({
      sessionId,
      content: 'second',
      displayMessage: 'Second display',
      agentType: 'agentic',
      imageContexts: [{ id: 'image-1' }],
      imageDisplayData: [{ id: 'image-1', name: 'clip.png' }],
      composerDraft: {
        value: 'Second original draft',
        contexts: [{
          id: 'file-1',
          type: 'file',
          timestamp: 1,
          filePath: '/workspace/file.ts',
          fileName: 'file.ts',
        }],
        pendingLargePastes: { 'paste-1': 'large paste content' },
      },
      userMessageMetadata: { sessionReferences: [{ sessionId: 'source' }] },
      retryCount: 2,
      initialStatus: 'failed',
    });
    pendingQueueManager.enqueue({ sessionId, content: 'third' });
    const payload = {
      id: target.id,
      content: target.content,
      displayMessage: target.displayMessage,
      agentType: target.agentType,
      imageContexts: structuredClone(target.imageContexts),
      imageDisplayData: structuredClone(target.imageDisplayData),
      composerDraft: structuredClone(target.composerDraft),
      userMessageMetadata: structuredClone(target.userMessageMetadata),
      timestamp: target.timestamp,
    };

    expect(pendingQueueManager.promoteForExplicitDrain(sessionId, target.id)).toBe(true);

    const items = pendingQueueManager.list(sessionId);
    expect(items.map(item => item.content)).toEqual(['second', 'first', 'third']);
    expect(items[0]).toBe(target);
    expect(items[0]).toMatchObject(payload);
    expect(items[0].status).toBe('queued');
    expect(items[0].retryCount).toBe(0);
  });

  it('keeps equal session ids isolated across device surfaces', () => {
    const sessionId = testSession();
    pendingQueueManager.enqueue({ sessionId, content: 'local draft' });

    activateSurface('peer-b');
    expect(pendingQueueManager.list(sessionId)).toEqual([]);
    pendingQueueManager.enqueue({ sessionId, content: 'peer draft' });

    activateSurface(LOCAL_SURFACE_ID);
    expect(pendingQueueManager.list(sessionId).map(item => item.content)).toEqual([
      'local draft',
    ]);
    activateSurface('peer-b');
    expect(pendingQueueManager.list(sessionId).map(item => item.content)).toEqual([
      'peer draft',
    ]);

    pendingQueueManager.clearSurface('peer-b');
    activateSurface(LOCAL_SURFACE_ID);
  });

  it('drops a queued duplicate of a live turn after a surface switch', () => {
    const sessionId = testSession();
    pendingQueueManager.enqueue({
      sessionId,
      content: '详细分析项目，然后调用 askuserquestion 随便问我几个当前项目相关的问题吧',
      initialStatus: 'failed',
    });
    pendingQueueManager.enqueue({
      sessionId,
      content: 'a later follow-up that should stay',
    });

    const removed = pendingQueueManager.reconcileAgainstLiveTurns(sessionId, [
      {
        id: 'dialog_live',
        status: 'processing',
        userMessage: {
          id: 'user-1',
          content: '详细分析项目，然后调用 askuserquestion 随便问我几个当前项目相关的问题吧',
          timestamp: Date.now(),
        },
      },
    ]);

    expect(removed).toBe(1);
    expect(pendingQueueManager.list(sessionId).map(item => item.content)).toEqual([
      'a later follow-up that should stay',
    ]);
    expect(queuedItemDuplicatesLiveTurn(
      { content: '详细分析项目，然后调用 askuserquestion 随便问我几个当前项目相关的问题吧' },
      [{
        id: 'dialog_live',
        status: 'processing',
        userMessage: {
          id: 'user-1',
          content: '详细分析项目，然后调用 askuserquestion 随便问我几个当前项目相关的问题吧',
          timestamp: Date.now(),
        },
      }],
    )).toBe(true);
  });
});
