// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, expect, it } from 'vitest';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { Session } from '@/flow_chat/types/flow-chat';
import type { SceneTab } from '../components/SceneBar/types';
import { useSessionTabLabels } from './useSessionTabLabels';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

afterEach(() => {
  flowChatStore.setState(state => ({ ...state, sessions: new Map(), activeSessionId: null }));
});

it('projects each referenced title independently of focus and updates replaced resources synchronously', () => {
  const a: Session = {
    sessionId: 'a', title: 'First task', workspacePath: '/alpha', config: {},
    dialogTurns: [], status: 'idle', createdAt: 1, lastActiveAt: 1, error: null,
  };
  const b: Session = { ...a, sessionId: 'b', title: 'Second task', workspacePath: '/beta' };
  flowChatStore.setState(state => ({ ...state, sessions: new Map([['a', a], ['b', b]]), activeSessionId: 'b' }));
  let tabs: SceneTab[] = [
    { id: 'session:alpha', lastUsed: 1, session: { surfaceId: 'local', workspaceKey: 'alpha', sessionId: 'a' } },
    { id: 'session:beta', lastUsed: 2, session: { surfaceId: 'local', workspaceKey: 'beta', sessionId: 'b' } },
  ];
  const container = document.createElement('div');
  const root = createRoot(container);
  let labels: Record<string, string> = {};
  function LabelObserver() { labels = useSessionTabLabels(tabs); return null; }
  try {
    act(() => root.render(<LabelObserver />));
    expect(labels).toEqual({ 'session:alpha': 'First task', 'session:beta': 'Second task' });
    act(() => {
      flowChatStore.setState(state => ({
        ...state, sessions: new Map(state.sessions).set('a', { ...a, title: 'Renamed in background' }),
      }));
    });
    expect(labels['session:alpha']).toBe('Renamed in background');
    expect(labels['session:beta']).toBe('Second task');
    tabs = [{ ...tabs[0], session: { ...tabs[0].session!, sessionId: 'b' } }];
    act(() => root.render(<LabelObserver />));
    expect(labels).toEqual({ 'session:alpha': 'Second task' });
  } finally {
    act(() => root.unmount());
  }
});
