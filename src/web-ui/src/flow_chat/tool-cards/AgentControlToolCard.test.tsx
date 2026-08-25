import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';
import { useSubagentIdentityStore } from '../subagent-identity';
import { AgentControlToolCard } from './AgentControlToolCard';

const mocks = vi.hoisted(() => ({
  openBtwSessionInAuxPane: vi.fn(),
  listeners: new Set<() => void>(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (key === 'flowChatHeader.agentTreeStatus.running') return 'Running';
      if (key === 'flowChatHeader.agentTreeStatus.completed') return 'Completed';
      if (key === 'toolCards.taskTool.defaultAgentKind') return 'Agent';
      if (typeof options?.defaultValue === 'string') return options.defaultValue;
      return key;
    },
  }),
}));

vi.mock('@/component-library/components/Markdown/Markdown', () => ({
  Markdown: ({ content }: { content: string }) => <div>{content}</div>,
}));

vi.mock('../services/btwSessionPane', () => ({
  openBtwSessionInAuxPane: (...args: unknown[]) => mocks.openBtwSessionInAuxPane(...args),
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    subscribe: (listener: () => void) => {
      mocks.listeners.add(listener);
      return () => mocks.listeners.delete(listener);
    },
    getState: () => ({
      sessions: new Map([
        ['parent-session', {
          sessionId: 'parent-session',
          workspacePath: 'D:\\workspace\\repo',
          remoteConnectionId: 'remote-1',
          remoteSshHost: 'host-1',
          config: { agentType: 'Ultra' },
          dialogTurns: [],
        }],
        ['child-session', {
          sessionId: 'child-session',
          sessionKind: 'subagent',
          parentSessionId: 'parent-session',
          parentToolCallId: 'agent-call',
          subagentType: 'SwarmWorker',
          mode: 'SwarmWorker',
          title: 'SwarmWorker: inspect parser',
          createdAt: 1000,
          status: 'active',
          config: { agentType: 'SwarmWorker' },
          dialogTurns: [{
            id: 'child-turn',
            status: 'processing',
            modelRounds: [],
          }],
        }],
      ]),
    }),
  },
}));

let JSDOMCtor: (new (
  html?: string,
  options?: { pretendToBeVisual?: boolean; url?: string }
) => { window: Window & typeof globalThis }) | null = null;

try {
  const jsdom = await import('jsdom');
  JSDOMCtor = jsdom.JSDOM as typeof JSDOMCtor;
} catch {
  JSDOMCtor = null;
}

const describeWithJsdom = JSDOMCtor ? describe : describe.skip;

const config: ToolCardConfig = {
  toolName: 'AgentSpawn',
  displayName: 'Launch Agent',
  icon: '',
  requiresConfirmation: false,
  resultDisplayType: 'detailed',
};

function agentToolItem(
  toolName: 'AgentSpawn' | 'AgentSendInput',
  overrides: Partial<FlowToolItem> = {},
): FlowToolItem {
  return {
    id: 'agent-tool',
    type: 'tool',
    toolName,
    timestamp: Date.now(),
    status: 'completed',
    subagentSessionId: 'child-session',
    subagentDialogTurnId: 'child-turn',
    toolCall: {
      id: 'agent-call',
      input: toolName === 'AgentSpawn'
        ? {
            agent_type: 'SwarmWorker',
            description: 'Inspect parser',
            prompt: 'Inspect the parser flow and report findings.',
          }
        : {
            agent_id: 'agent-1',
            description: 'Continue parser review',
            prompt: 'Continue with the error recovery paths.',
          },
    },
    ...overrides,
  };
}

describeWithJsdom('AgentControlToolCard', () => {
  let dom: { window: Window & typeof globalThis };
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOMCtor!('<!doctype html><html><body></body></html>', {
      pretendToBeVisual: true,
      url: 'http://localhost',
    });
    const { window } = dom;
    vi.stubGlobal('window', window);
    vi.stubGlobal('document', window.document);
    vi.stubGlobal('navigator', window.navigator);
    vi.stubGlobal('HTMLElement', window.HTMLElement);
    vi.stubGlobal('CustomEvent', window.CustomEvent);
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);

    useSubagentIdentityStore.getState().clear();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    dom.window.close();
    mocks.listeners.clear();
    useSubagentIdentityStore.getState().clear();
    vi.clearAllMocks();
    vi.unstubAllGlobals();
  });

  it.each(['AgentSpawn', 'AgentSendInput'] as const)(
    'shares the agent pill, status, and expandable prompt for %s',
    async (toolName) => {
      await act(async () => {
        root.render(
          <AgentControlToolCard
            toolItem={agentToolItem(toolName)}
            config={{ ...config, toolName }}
            sessionId="parent-session"
          />,
        );
      });

      const pill = container.querySelector<HTMLButtonElement>('.agent-control-tool-card__agent-pill');
      const toggle = container.querySelector<HTMLElement>('[data-testid="agent-control-tool-card-toggle"]');
      expect(pill?.textContent?.trim()).toBeTruthy();
      expect(pill?.querySelector('[data-bf-component="subagent-avatar"]')).not.toBeNull();
      expect(container.textContent).toContain('Running');
      expect(container.querySelector('[data-bf-part="expandIndicator"]')).not.toBeNull();
      expect(container.textContent).not.toContain(
        toolName === 'AgentSpawn'
          ? 'Inspect the parser flow and report findings.'
          : 'Continue with the error recovery paths.',
      );

      await act(async () => {
        toggle!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      });
      expect(container.textContent).toContain(
        toolName === 'AgentSpawn'
          ? 'Inspect the parser flow and report findings.'
          : 'Continue with the error recovery paths.',
      );

      await act(async () => {
        pill!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      });
      expect(mocks.openBtwSessionInAuxPane).toHaveBeenCalledWith(expect.objectContaining({
        childSessionId: 'child-session',
        parentSessionId: 'parent-session',
        parentToolCallId: 'agent-call',
        sessionKind: 'subagent',
        includeInternal: true,
      }));
      expect(container.textContent).toContain(
        toolName === 'AgentSpawn'
          ? 'Inspect the parser flow and report findings.'
          : 'Continue with the error recovery paths.',
      );
    },
  );

  it('stays collapsed and disables expansion while parameters are streaming', async () => {
    await act(async () => {
      root.render(
        <AgentControlToolCard
          toolItem={agentToolItem('AgentSpawn')}
          config={config}
          sessionId="parent-session"
        />,
      );
    });
    await act(async () => {
      container
        .querySelector<HTMLElement>('[data-testid="agent-control-tool-card-toggle"]')!
        .dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });
    expect(container.textContent).toContain('Inspect the parser flow and report findings.');

    const streamingItem = agentToolItem('AgentSpawn', {
      status: 'streaming',
      isParamsStreaming: true,
      partialParams: {
        agent_type: 'SwarmWorker',
        description: 'Inspect parser',
        prompt: 'Partial prompt that must stay collapsed.',
      },
    });

    await act(async () => {
      root.render(
        <AgentControlToolCard
          toolItem={streamingItem}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    expect(container.querySelector('[data-testid="agent-control-tool-card-toggle"]')).toBeNull();
    expect(container.textContent).not.toContain('Inspect the parser flow and report findings.');
    expect(
      container.querySelector('.base-tool-card-expanded-collapse')?.getAttribute('aria-hidden'),
    ).toBe('true');
    expect(container.querySelector('[data-bf-state~="streaming"]')).not.toBeNull();
  });
});
