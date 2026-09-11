// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { FlowChatState, Session } from '../../types/flow-chat';
import { useBtwSessionState } from './useBtwSessionState';

const store = vi.hoisted(() => ({
  state: { sessions: new Map() } as FlowChatState,
  listeners: new Set<() => void>(),
  beforeSubscribe: undefined as (() => void) | undefined,
}));

vi.mock('../../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => store.state,
    subscribeSelector: <T,>(select: (state: FlowChatState) => T, notify: (value: T) => void) => {
      store.beforeSubscribe?.();
      // Match the store's lazy first notification, including unchanged data.
      let hasPrevious = false;
      let previous: T;
      const listener = () => {
        const next = select(store.state);
        if (hasPrevious && Object.is(previous, next)) return;
        hasPrevious = true;
        previous = next;
        notify(next);
      };
      store.listeners.add(listener);
      return () => { store.listeners.delete(listener); };
    },
  },
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('useBtwSessionState', () => {
  let root: Root;
  let container: HTMLDivElement;
  let renders: number;
  let selected: ReturnType<typeof useBtwSessionState>;
  const session = (id: string) => ({ sessionId: id, title: id } as Session);
  function Harness({ child = 'child', parent = 'parent', reviewCheck = false }: { child?: string; parent?: string; reviewCheck?: boolean }) {
    selected = useBtwSessionState(child, parent, reviewCheck);
    renders += 1;
    return null;
  }
  function publish(sessions: Map<string, Session>) {
    act(() => {
      store.state = { ...store.state, sessions };
      store.listeners.forEach(listener => listener());
    });
  }
  beforeEach(() => {
    store.state = { sessions: new Map(['child', 'parent', 'other'].map(id => [id, session(id)])) } as FlowChatState;
    store.listeners.clear();
    store.beforeSubscribe = undefined;
    renders = 0;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });
  afterEach(() => {
    act(() => root.unmount());
    expect(store.listeners.size).toBe(0);
    container.remove();
  });

  it('does not render for unrelated subagent updates, including the first notification', () => {
    act(() => root.render(<Harness />));
    const before = renders;
    const snapshot = selected;
    for (let index = 0; index < 50; index += 1) {
      publish(new Map(store.state.sessions).set('other', session('other')));
    }
    expect(renders).toBe(before);
    expect(selected).toBe(snapshot);
  });

  it('updates when the child streams or parent metadata/review state changes', () => {
    act(() => root.render(<Harness />));
    const before = renders;
    const child = { ...store.state.sessions.get('child')!, title: 'streamed' };
    publish(new Map(store.state.sessions).set('child', child));
    expect(selected.childSession).toBe(child);
    expect(renders).toBe(before + 1);
    const parent = { ...store.state.sessions.get('parent')!, title: 'renamed' };
    publish(new Map(store.state.sessions).set('parent', parent));
    expect(selected.parentMetadata?.title).toBe(parent.title);
    expect(selected.parentMetadata).not.toHaveProperty('dialogTurns');
    expect(renders).toBe(before + 2);
  });

  it('selects new IDs immediately and handles hydration, removal and surface replacement', () => {
    act(() => root.render(<Harness />));
    act(() => root.render(<Harness child="missing" parent="other" />));
    expect(selected.childSession).toBeUndefined();
    expect(selected.parentMetadata?.title).toBe('other');
    expect(store.listeners.size).toBe(1);
    const hydrated = session('missing');
    publish(new Map(store.state.sessions).set('missing', hydrated));
    expect(selected.childSession).toBe(hydrated);
    const replacement = session('missing');
    publish(new Map([['missing', replacement]]));
    expect(selected.childSession).toBe(replacement);
    expect(selected.parentMetadata).toBeUndefined();
    publish(new Map());
    expect(selected.childSession).toBeUndefined();
  });

  it('does not miss an update between render and subscription', () => {
    const child = session('child');
    store.beforeSubscribe = () => {
      store.state = { ...store.state, sessions: new Map(store.state.sessions).set('child', child) };
    };
    act(() => root.render(<Harness />));
    expect(selected.childSession).toBe(child);
  });

  it('ignores parent streaming without scanning its transcript in ordinary panes', () => {
    act(() => root.render(<Harness />));
    const before = renders;
    const snapshot = selected;
    for (let index = 0; index < 50; index += 1) {
      const parent = {
        ...session('parent'), lastActiveAt: index,
        get dialogTurns(): Session['dialogTurns'] {
          throw new Error('Ordinary panes must not read parent turns');
        },
      };
      publish(new Map(store.state.sessions).set('parent', parent));
    }
    expect(renders).toBe(before);
    expect(selected).toBe(snapshot);
  });

  it.each(['title', 'workspacePath', 'remoteConnectionId', 'remoteSshHost'] as const)(
    'updates parent %s and handles its removal', field => {
      act(() => root.render(<Harness />));
      const before = renders;
      publish(new Map(store.state.sessions).set('parent', {
        ...session('parent'), [field]: 'updated',
      }));
      expect(selected.parentMetadata?.[field]).toBe('updated');
      expect(renders).toBe(before + 1);
      const parent = { ...store.state.sessions.get('parent')! };
      Reflect.deleteProperty(parent, field);
      publish(new Map(store.state.sessions).set('parent', parent));
      expect(selected.parentMetadata?.[field]).toBeUndefined();
      expect(renders).toBe(before + 2);
    },
  );

  it('publishes only the linked review outcome and resets it when the view changes', () => {
    const parentWithResult = (status: string, toolId = 'linked-task') => ({
      ...session('parent'),
      dialogTurns: [{
        id: 'turn', modelRounds: [{
          id: 'round', items: [{
            type: 'tool', id: toolId, status: 'completed',
            toolCall: { id: toolId }, toolResult: { result: { status } },
          }],
        }],
      }],
    } as unknown as Session);
    store.state.sessions.set('child', { ...session('child'), parentToolCallId: 'linked-task' });
    store.state.sessions.set('parent', parentWithResult('completed'));
    act(() => root.render(<Harness reviewCheck />));
    const before = renders;
    publish(new Map(store.state.sessions).set('parent', parentWithResult('timed_out', 'unrelated-task')));
    expect(renders).toBe(before);
    publish(new Map(store.state.sessions).set('parent', parentWithResult('partial_timeout')));
    expect(selected.reviewTaskOutcome).toBe('partial-timeout');
    expect(renders).toBe(before + 1);
    publish(new Map(store.state.sessions).set('parent', parentWithResult('partial_timeout')));
    expect(renders).toBe(before + 1);
    publish(new Map(store.state.sessions).set('child', { ...session('child'), parentToolCallId: 'other-task' }));
    expect(selected.reviewTaskOutcome).toBeNull();
    publish(new Map(store.state.sessions).set('parent', parentWithResult('timed_out', 'other-task')));
    expect(selected.reviewTaskOutcome).toBe('timed-out');
    act(() => root.render(<Harness />));
    expect(selected.reviewTaskOutcome).toBeNull();
    act(() => root.render(<Harness reviewCheck />));
    expect(selected.reviewTaskOutcome).toBe('timed-out');
    publish(new Map([['child', store.state.sessions.get('child')!]]));
    expect(selected.reviewTaskOutcome).toBeNull();
  });
});
