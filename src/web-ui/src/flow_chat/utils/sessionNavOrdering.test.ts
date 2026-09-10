import { describe, expect, it } from 'vitest';
import type { Session } from '../types/flow-chat';
import type { SessionNavStatusKind } from './sessionNavStatus';
import { SessionNavOrdering } from './sessionNavOrdering';
import { compareWorkspaceNavSessions } from '@/app/components/NavPanel/workspaceSessionView';

const session = (sessionId: string, createdAt: number, overrides: Partial<Session> = {}): Session => ({
  sessionId, title: sessionId, createdAt, lastActiveAt: createdAt, status: 'idle',
  config: {}, dialogTurns: [], error: null, ...overrides,
} as Session);
const status = (kind: SessionNavStatusKind) => ({ kind, pendingCount: 0 });
const sortedIds = (store: SessionNavOrdering, sessions: Session[], surface = 'local') => [...sessions]
  .sort((left, right) => compareWorkspaceNavSessions(left, right, 'updated', value => value.title ?? '',
    value => store.get(surface, value.sessionId)?.status.kind === 'running',
    value => store.get(surface, value.sessionId)?.sortTimestamp ?? value.createdAt))
  .map(value => value.sessionId);

describe('navigation execution recency', () => {
  it('captures historical order once and ignores opening, hydration, rename and completion timestamps', () => {
    const store = new SessionNavOrdering();
    let older = session('older', 10, { lastFinishedAt: 30, historyState: 'metadata-only' });
    const newer = session('newer', 20, { lastFinishedAt: 40 });
    store.observe('local', older, status('idle'), 100);
    store.observe('local', newer, status('idle'), 100);
    expect(sortedIds(store, [older, newer])).toEqual(['newer', 'older']);
    for (const change of [
      { status: 'active' as const, lastActiveAt: 200 },
      { historyState: 'ready' as const, updatedAt: 300 },
      { title: 'Renamed', updatedAt: 400 },
      { lastFinishedAt: 500 },
    ]) {
      older = { ...older, ...change };
      expect(store.observe('local', older, status('idle'), 600)).toBe(false);
      expect(sortedIds(store, [older, newer])).toEqual(['newer', 'older']);
    }
  });

  it('promotes each new run once and keeps the order during output, completion and failure', () => {
    const store = new SessionNavOrdering();
    const older = session('older', 10);
    const newer = session('newer', 20);
    const sessions = [older, newer];
    sessions.forEach(value => store.observe('local', value, status('idle'), 30));
    store.observe('local', older, status('running'), 100);
    expect(sortedIds(store, sessions)).toEqual(['older', 'newer']);
    store.observe('local', newer, status('running'), 200);
    expect(sortedIds(store, sessions)).toEqual(['newer', 'older']);
    for (const kind of ['running', 'unread', 'idle', 'error'] as const) {
      store.observe('local', { ...older, lastActiveAt: 300, lastFinishedAt: 300 }, status(kind), 300);
      expect(sortedIds(store, sessions)).toEqual(['newer', 'older']);
    }
    store.observe('local', older, status('running'), 400);
    expect(sortedIds(store, sessions)).toEqual(['older', 'newer']);
  });

  it.each(['idle', 'approval', 'input', 'error', 'unread', 'paused', 'stopped', 'queued'] as const)(
    'promotes on %s to running, including resuming an existing turn', kind => {
      const store = new SessionNavOrdering();
      const value = session('session', 10);
      store.observe('local', value, status(kind), 20);
      store.observe('local', value, status('running'), 100);
      expect(store.get('local', value.sessionId)?.sortTimestamp).toBe(100);
      expect(store.observe('local', value, status('running'), 200)).toBe(false);
    },
  );

  it('does not treat initial running snapshots or a reconnect as another start', () => {
    const store = new SessionNavOrdering();
    const value = session('session', 10, { lastFinishedAt: 20 });
    store.observe('local', value, status('syncing'), 100);
    store.observe('local', value, status('running'), 200);
    expect(store.get('local', value.sessionId)?.sortTimestamp).toBe(20);
    store.observe('local', value, status('syncing'), 300);
    store.observe('local', value, status('running'), 400);
    expect(store.get('local', value.sessionId)?.sortTimestamp).toBe(20);
    store.observe('local', value, status('idle'), 500);
    store.observe('local', value, status('syncing'), 600);
    store.observe('local', value, status('running'), 700);
    expect(store.get('local', value.sessionId)?.sortTimestamp).toBe(700);
  });

  it('orders rapid starts deterministically even if the clock moves backwards', () => {
    const store = new SessionNavOrdering();
    const first = session('first', 10);
    const second = session('second', 20);
    const third = session('third', 30);
    const sessions = [first, second, third];
    sessions.forEach(value => store.observe('local', value, status('idle'), 40));
    store.observe('local', first, status('running'), 100);
    store.observe('local', second, status('running'), 100);
    store.observe('local', third, status('running'), 90);
    expect(sortedIds(store, sessions)).toEqual(['third', 'second', 'first']);
  });

  it('isolates identical session ids across devices and retains background ordering', () => {
    const store = new SessionNavOrdering();
    const sessions = [session('older', 10), session('newer', 20)];
    for (const surface of ['local', 'peer']) {
      sessions.forEach(value => store.observe(surface, value, status('idle'), 30));
    }
    store.observe('peer', sessions[0], status('running'), 100);
    store.observe('peer', sessions[0], status('unread'), 200);
    expect(sortedIds(store, sessions, 'local')).toEqual(['newer', 'older']);
    expect(sortedIds(store, sessions, 'peer')).toEqual(['older', 'newer']);
    store.delete('peer', 'older');
    expect(store.get('peer', 'older')).toBeUndefined();
    expect(store.get('local', 'older')).toBeDefined();
    store.clearSurface('peer');
    expect([...store.sessionIds('peer')]).toEqual([]);
    expect([...store.sessionIds('local')]).toEqual(['older', 'newer']);
  });
});
