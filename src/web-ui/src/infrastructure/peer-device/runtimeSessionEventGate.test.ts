import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  beginRuntimeSessionAttachment,
  isRuntimeSessionProjectionStale,
  markRuntimeSessionProjectionStale,
  resetRuntimeSessionEventGateForTest,
  routeRuntimeSessionEvent,
  RUNTIME_EVENT_CURSOR_KEY,
  RUNTIME_EVENT_STREAM_ID_KEY,
  subscribeRuntimeSessionEventGaps,
} from './runtimeSessionEventGate';

function payload(sessionId: string, streamId: string, cursor: number, text: string) {
  return {
    sessionId,
    text,
    [RUNTIME_EVENT_STREAM_ID_KEY]: streamId,
    [RUNTIME_EVENT_CURSOR_KEY]: cursor,
  };
}

afterEach(() => {
  resetRuntimeSessionEventGateForTest();
});

describe('runtimeSessionEventGate', () => {
  it('strips cursor metadata on the ordinary live path', () => {
    const delivered = vi.fn();
    routeRuntimeSessionEvent(
      'local',
      'agentic://text-chunk',
      payload('session', 'runtime-a', 1, 'a'),
      delivered,
    );

    expect(delivered).toHaveBeenCalledWith({ sessionId: 'session', text: 'a' });
  });

  it('fences snapshot-covered events and releases only newer cursors in order', () => {
    const delivered: string[] = [];
    const attachment = beginRuntimeSessionAttachment('local', 'session');

    for (const [cursor, text] of [[4, 'covered'], [6, 'newer-1'], [7, 'newer-2']] as const) {
      routeRuntimeSessionEvent(
        'local',
        'agentic://text-chunk',
        payload('session', 'runtime-a', cursor, text),
        event => delivered.push(event.text),
      );
    }
    expect(delivered).toEqual([]);

    attachment.finish({ streamId: 'runtime-a', cursor: 5 });
    expect(delivered).toEqual(['newer-1', 'newer-2']);

    const nextAttachment = beginRuntimeSessionAttachment('local', 'session');
    expect(nextAttachment.requiresReplay({ streamId: 'runtime-a', cursor: 7 })).toBe(false);
    expect(nextAttachment.requiresReplay({ streamId: 'runtime-a', cursor: 8 })).toBe(true);
    nextAttachment.abort({ discard: true });
  });

  it('never compares cursors across Runtime process streams', () => {
    const delivered: string[] = [];
    const attachment = beginRuntimeSessionAttachment('local', 'session');
    routeRuntimeSessionEvent(
      'local',
      'agentic://text-chunk',
      payload('session', 'runtime-b', 1, 'after-restart'),
      event => delivered.push(event.text),
    );

    attachment.finish({ streamId: 'runtime-a', cursor: 99 });
    expect(delivered).toEqual(['after-restart']);
  });

  it('tracks ordinary live progress so healthy periodic refreshes do not replay', () => {
    for (const cursor of [1, 2, 3, 4]) {
      routeRuntimeSessionEvent(
        'local',
        'agentic://text-chunk',
        payload('session', 'runtime-a', cursor, 'live'),
        vi.fn(),
      );
    }

    const attachment = beginRuntimeSessionAttachment('local', 'session');
    expect(attachment.requiresReplay({ streamId: 'runtime-a', cursor: 4 })).toBe(false);
    expect(attachment.requiresReplay({ streamId: 'runtime-a', cursor: 5 })).toBe(true);
    expect(attachment.requiresReplay({ streamId: 'runtime-b', cursor: 1 })).toBe(true);
    attachment.abort({ discard: true });
  });

  it('requires replay after a dropped event even when the live cursor matches', () => {
    for (const cursor of [1, 2, 3]) {
      routeRuntimeSessionEvent(
        'local',
        'agentic://text-chunk',
        payload('session', 'runtime-a', cursor, 'live'),
        vi.fn(),
      );
    }

    markRuntimeSessionProjectionStale('local', 'session');

    const attachment = beginRuntimeSessionAttachment('local', 'session');
    expect(attachment.requiresReplay({ streamId: 'runtime-a', cursor: 3 })).toBe(true);
    attachment.finish({ streamId: 'runtime-a', cursor: 3 });

    const nextAttachment = beginRuntimeSessionAttachment('local', 'session');
    expect(nextAttachment.requiresReplay({ streamId: 'runtime-a', cursor: 3 })).toBe(false);
    nextAttachment.abort({ discard: true });
  });

  it('requires replay after detecting a missing live cursor', () => {
    const gapListener = vi.fn();
    subscribeRuntimeSessionEventGaps(gapListener);
    for (const cursor of [1, 3]) {
      routeRuntimeSessionEvent(
        'local',
        'agentic://text-chunk',
        payload('session', 'runtime-a', cursor, 'live'),
        vi.fn(),
      );
    }

    const attachment = beginRuntimeSessionAttachment('local', 'session');
    expect(gapListener).toHaveBeenCalledWith('local', 'session');
    expect(attachment.requiresReplay({ streamId: 'runtime-a', cursor: 3 })).toBe(true);
    attachment.finish({ streamId: 'runtime-a', cursor: 3 });

    const nextAttachment = beginRuntimeSessionAttachment('local', 'session');
    expect(nextAttachment.requiresReplay({ streamId: 'runtime-a', cursor: 3 })).toBe(false);
    nextAttachment.abort({ discard: true });
  });

  it('releases events that the Runtime snapshot does not materialize', () => {
    const delivered = vi.fn();
    const attachment = beginRuntimeSessionAttachment('local', 'session');
    routeRuntimeSessionEvent(
      'local',
      'session_title_generated',
      { sessionId: 'session', title: 'Keep me live' },
      delivered,
    );

    attachment.finish({ streamId: 'runtime-a', cursor: 100 });
    expect(delivered).toHaveBeenCalledWith({
      sessionId: 'session',
      title: 'Keep me live',
    });
  });

  it('keeps the projection dirty when finish cannot prove the UI caught up', () => {
    for (const cursor of [1, 2, 3]) {
      routeRuntimeSessionEvent(
        'local',
        'agentic://tool-event',
        payload('session', 'runtime-a', cursor, 'tool'),
        vi.fn(),
      );
    }

    const attachment = beginRuntimeSessionAttachment('local', 'session');
    attachment.finish(
      { streamId: 'runtime-a', cursor: 3 },
      { projectionCaughtUp: false },
    );

    expect(isRuntimeSessionProjectionStale('local', 'session')).toBe(true);
    const nextAttachment = beginRuntimeSessionAttachment('local', 'session');
    expect(nextAttachment.requiresReplay({ streamId: 'runtime-a', cursor: 3 })).toBe(true);
    nextAttachment.abort({ discard: true });
  });

  it('transfers an overlapping fence instead of delivering it onto a resetting machine', () => {
    const delivered: string[] = [];
    const first = beginRuntimeSessionAttachment('local', 'session');
    routeRuntimeSessionEvent(
      'local',
      'agentic://tool-event',
      payload('session', 'runtime-a', 4, 'started'),
      event => delivered.push(event.text),
    );
    routeRuntimeSessionEvent(
      'local',
      'agentic://tool-event',
      payload('session', 'runtime-a', 5, 'completed'),
      event => delivered.push(event.text),
    );

    const second = beginRuntimeSessionAttachment('local', 'session');
    expect(first.isCurrent()).toBe(false);
    expect(second.isCurrent()).toBe(true);
    expect(delivered).toEqual([]);

    first.finish({ streamId: 'runtime-a', cursor: 5 });
    expect(delivered).toEqual([]);
    expect(isRuntimeSessionProjectionStale('local', 'session')).toBe(false);

    second.finish(
      { streamId: 'runtime-a', cursor: 5 },
      { projectionCaughtUp: false },
    );
    expect(delivered).toEqual([]);
    expect(isRuntimeSessionProjectionStale('local', 'session')).toBe(true);
  });

  it('isolates identical Session ids on different device Surfaces', () => {
    const localDelivered = vi.fn();
    const peerDelivered = vi.fn();
    const attachment = beginRuntimeSessionAttachment('local', 'same-session');

    routeRuntimeSessionEvent(
      'local',
      'agentic://text-chunk',
      payload('same-session', 'local-runtime', 2, 'local'),
      localDelivered,
    );
    routeRuntimeSessionEvent(
      'peer-a',
      'agentic://text-chunk',
      payload('same-session', 'peer-runtime', 2, 'peer'),
      peerDelivered,
    );

    expect(localDelivered).not.toHaveBeenCalled();
    expect(peerDelivered).toHaveBeenCalledTimes(1);
    attachment.abort({ discard: true });
  });
});
