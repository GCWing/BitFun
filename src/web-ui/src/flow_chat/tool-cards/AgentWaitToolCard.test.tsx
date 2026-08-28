import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import type { FlowToolItem, Session, ToolCardConfig } from '../types/flow-chat';
import { AgentWaitToolCard, shouldShowAgentWaitSteeringHint } from './AgentWaitToolCard';

const { getState } = vi.hoisted(() => ({ getState: vi.fn() }));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: { getState },
}));

vi.mock('react-i18next', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return {
    initReactI18next: {
      type: '3rdParty',
      init: vi.fn(),
    },
    useTranslation: () => ({ t: createTestI18nT('flow-chat') }),
  };
});

const config: ToolCardConfig = {
  toolName: 'AgentWait',
  displayName: 'Wait for agents',
  icon: 'WAIT',
  requiresConfirmation: false,
  resultDisplayType: 'summary',
  displayMode: 'compact',
};

function session(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'session-1',
    dialogTurns: [],
    status: 'active',
    config: {},
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    sessionKind: 'normal',
    ...overrides,
  };
}

function item(status: FlowToolItem['status'] = 'running'): FlowToolItem {
  return {
    id: 'agent-wait-1',
    type: 'tool',
    toolName: 'AgentWait',
    status,
    timestamp: 1,
    toolCall: { id: 'agent-wait-1', input: { bg_task_ids: ['a1_bg1'] } },
  };
}

describe('AgentWaitToolCard', () => {
  it('shows the steering hint only for a running top-level BitFun session', () => {
    const topLevel = session();
    expect(shouldShowAgentWaitSteeringHint('running', 'default', topLevel)).toBe(true);
    expect(shouldShowAgentWaitSteeringHint('completed', 'default', topLevel)).toBe(false);
    expect(
      shouldShowAgentWaitSteeringHint('running', 'subagent-projection', topLevel),
    ).toBe(false);
    expect(
      shouldShowAgentWaitSteeringHint('running', 'default', session({ sessionKind: 'subagent' })),
    ).toBe(false);
    expect(
      shouldShowAgentWaitSteeringHint('running', 'default', session({ isHistorical: true })),
    ).toBe(false);
    expect(
      shouldShowAgentWaitSteeringHint('running', 'default', session({ mode: 'acp:codex' })),
    ).toBe(false);
  });

  it('renders the localized send-now steering guidance while waiting', () => {
    getState.mockReturnValue({ sessions: new Map([['session-1', session()]]) });
    const html = renderToStaticMarkup(
      <AgentWaitToolCard toolItem={item()} config={config} sessionId="session-1" />,
    );

    expect(html).toContain('Wait for subagents to finish');
    expect(html).toContain('Send a steering message to end the wait early');
    expect(html).toContain('data-bf-attention="ambient"');
    expect(html).toContain('data-bf-tool-card="agent-wait"');
    expect(html).toContain('data-bf-part="content"');
    expect(html).toContain('data-bf-part="summary"');
    expect(html).not.toContain('data-bf-part="steeringHint"');
  });

  it('renders a normal steered result without the running hint', () => {
    getState.mockReturnValue({ sessions: new Map([['session-1', session()]]) });
    const completed = item('completed');
    completed.toolResult = {
      success: true,
      result: { status: 'steered', results: [], pending_bg_task_ids: ['a1_bg1'] },
    };
    const html = renderToStaticMarkup(
      <AgentWaitToolCard toolItem={completed} config={config} sessionId="session-1" />,
    );

    expect(html).toContain('Wait ended early');
    expect(html).not.toContain('Send a steering message');
  });

  it('formats failures with an optional error detail', () => {
    getState.mockReturnValue({ sessions: new Map([['session-1', session()]]) });
    const failed = item('error');
    failed.toolResult = { success: false, result: {}, error: 'database unavailable' };
    const withDetail = renderToStaticMarkup(
      <AgentWaitToolCard toolItem={failed} config={config} sessionId="session-1" />,
    );
    expect(withDetail).toContain('Execution failed: database unavailable');

    failed.toolResult = { success: false, result: {} };
    const withoutDetail = renderToStaticMarkup(
      <AgentWaitToolCard toolItem={failed} config={config} sessionId="session-1" />,
    );
    expect(withoutDetail).toContain('Execution failed');
  });
});
