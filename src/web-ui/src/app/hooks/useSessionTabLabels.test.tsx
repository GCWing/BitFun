// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, expect, it } from 'vitest';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { Session } from '@/flow_chat/types/flow-chat';
import type { SceneTab } from '../components/SceneBar/types';
import { useSessionTabLabels, type SessionTabLabel } from './useSessionTabLabels';
import { createDefaultSessionTitleDescriptor, deriveSessionTitleState } from '@/flow_chat/utils/sessionTitle';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

afterEach(() => {
  flowChatStore.setState(state => ({ ...state, sessions: new Map(), activeSessionId: null }));
});

it('updates a visible tab number when an unopened default is added or removed in its workspace', () => {
  const a: Session = {
    sessionId: 'a', workspacePath: '/alpha', config: {},
    ...deriveSessionTitleState(createDefaultSessionTitleDescriptor(() => 'New Session')),
    workspaceSessionNumber: 7,
    dialogTurns: [], status: 'idle', createdAt: 1, lastActiveAt: 1, error: null,
  };
  const b: Session = { ...a, sessionId: 'b', workspaceSessionNumber: 8 };
  flowChatStore.setState(state => ({ ...state, sessions: new Map([['a', a]]), activeSessionId: 'a' }));
  const tabs: SceneTab[] = [
    { id: 'session:alpha', lastUsed: 1, session: { surfaceId: 'local', workspaceKey: 'alpha', sessionId: 'a' } },
  ];
  const root = createRoot(document.createElement('div'));
  let labels: Record<string, SessionTabLabel> = {};
  function LabelObserver() { labels = useSessionTabLabels(tabs); return null; }
  try {
    act(() => root.render(<LabelObserver />));
    expect(labels['session:alpha'].number).toBeUndefined();
    act(() => flowChatStore.setState(state => ({ ...state, sessions: new Map(state.sessions).set('b', b) })));
    expect(labels['session:alpha'].number).toBe('07');
    act(() => flowChatStore.setState(state => ({ ...state, sessions: new Map([['a', a]]) })));
    expect(labels['session:alpha'].number).toBeUndefined();
    act(() => flowChatStore.setState(state => ({ ...state, sessions: new Map([['b', b], ['a', a]]) })));
    expect(labels['session:alpha'].number).toBe('07');
  } finally {
    act(() => root.unmount());
  }
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
  let labels: Record<string, SessionTabLabel> = {};
  function LabelObserver() { labels = useSessionTabLabels(tabs); return null; }
  try {
    act(() => root.render(<LabelObserver />));
    expect(labels).toEqual({ 'session:alpha': { title: 'First task' }, 'session:beta': { title: 'Second task' } });
    act(() => {
      flowChatStore.setState(state => ({
        ...state, sessions: new Map(state.sessions).set('a', { ...a, title: 'Renamed in background' }),
      }));
    });
    expect(labels['session:alpha'].title).toBe('Renamed in background');
    expect(labels['session:beta'].title).toBe('Second task');
    tabs = [{ ...tabs[0], session: { ...tabs[0].session!, sessionId: 'b' } }];
    act(() => root.render(<LabelObserver />));
    expect(labels).toEqual({ 'session:alpha': { title: 'Second task' } });
  } finally {
    act(() => root.unmount());
  }
});
