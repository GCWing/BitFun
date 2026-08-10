// @vitest-environment jsdom

import { afterEach, describe, expect, it } from 'vitest';
import {
  pendingQueueManager,
  queuedMessageHasUnsupportedSteeringPayload,
} from './PendingQueueModule';

const sessions: string[] = [];

function testSession(): string {
  const sessionId = `pending-queue-test-${sessions.length}`;
  sessions.push(sessionId);
  return sessionId;
}

afterEach(() => {
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

  it('rejects only payloads that the text-only steering contract would flatten', () => {
    const plain = {
      id: 'plain',
      sessionId: 'session-1',
      content: 'plain text',
      timestamp: 1,
      status: 'queued' as const,
      retryCount: 0,
    };

    expect(queuedMessageHasUnsupportedSteeringPayload(plain)).toBe(false);
    expect(
      queuedMessageHasUnsupportedSteeringPayload({
        ...plain,
        imageContexts: [{ id: 'image-1' }],
      }),
    ).toBe(true);
    expect(
      queuedMessageHasUnsupportedSteeringPayload({
        ...plain,
        userMessageMetadata: { deepReviewRunManifest: { requestId: 'review-1' } },
      }),
    ).toBe(true);
    expect(
      queuedMessageHasUnsupportedSteeringPayload({
        ...plain,
        userMessageMetadata: { sessionReferences: [{ sessionId: 'source' }] },
      }),
    ).toBe(true);
    expect(
      queuedMessageHasUnsupportedSteeringPayload({
        ...plain,
        userMessageMetadata: { composerPresentation: { parts: [] } },
      }),
    ).toBe(true);
  });
});
