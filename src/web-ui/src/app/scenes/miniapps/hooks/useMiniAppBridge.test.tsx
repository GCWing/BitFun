/**
 * @vitest-environment jsdom
 */

import React, { useRef } from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { MiniApp } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useMiniAppStore } from '../miniAppStore';
import { useMiniAppBridge } from './useMiniAppBridge';

const mocks = vi.hoisted(() => ({
  activeTabId: 'miniapp:market-lens',
  agentEnsureSession: vi.fn(),
  agentRun: vi.fn(),
  apiListen: vi.fn(),
  openMainSession: vi.fn(),
  addExternalSession: vi.fn(),
  loadSessionHistory: vi.fn(),
}));

vi.mock('@/infrastructure/api/service-api/MiniAppAPI', () => ({
  miniAppAPI: {
    agentEnsureSession: mocks.agentEnsureSession,
    agentRun: mocks.agentRun,
  },
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
  message: vi.fn(),
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useCurrentWorkspace: () => ({ workspacePath: '/repo' }),
}));

vi.mock('@/infrastructure/theme/hooks/useTheme', () => ({
  useTheme: () => ({ theme: 'dark' }),
}));

vi.mock('../utils/buildMiniAppThemeVars', () => ({
  buildMiniAppThemeVars: () => ({}),
}));

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: {
    listen: (...args: unknown[]) => mocks.apiListen(...args),
    invoke: vi.fn(),
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ currentLanguage: 'en-US' }),
}));

vi.mock('@/infrastructure/api/service-api/SystemAPI', () => ({
  systemAPI: {},
}));

vi.mock('@/infrastructure/api', () => ({
  workspaceAPI: {},
}));

vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({ sessions: new Map() }),
    addExternalSession: (...args: unknown[]) => mocks.addExternalSession(...args),
    loadSessionHistory: (...args: unknown[]) => mocks.loadSessionHistory(...args),
  },
}));

vi.mock('@/flow_chat/services/sessionActivation', () => ({
  openMainSession: (...args: unknown[]) => mocks.openMainSession(...args),
}));

vi.mock('@/app/stores/sceneStore', () => ({
  useSceneStore: {
    getState: () => ({ activeTabId: mocks.activeTabId }),
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

const app = {
  id: 'market-lens',
  name: 'Market Lens',
  permissions: {
    node: { enabled: false },
    agent: { enabled: true },
    host: { chat_composer: true },
  },
} as unknown as MiniApp;

function BridgeHarness() {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  useMiniAppBridge(
    iframeRef,
    app,
    { kind: 'active', appId: app.id },
    true,
  );
  return <iframe ref={iframeRef} title="Market Lens test" />;
}

async function dispatchRpc(
  iframe: HTMLIFrameElement,
  id: number,
  method: string,
  params: Record<string, unknown> = {},
) {
  await act(async () => {
    window.dispatchEvent(new MessageEvent('message', {
      data: { jsonrpc: '2.0', id, method, params },
      source: iframe.contentWindow,
    }));
    await new Promise((resolve) => window.setTimeout(resolve, 0));
  });
}

describe('useMiniAppBridge floating Agent routing', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mocks.activeTabId = 'miniapp:market-lens';
    mocks.agentEnsureSession.mockResolvedValue({
      sessionId: 'session-1',
      created: true,
      workspacePath: '/repo',
    });
    mocks.agentRun.mockResolvedValue({ sessionId: 'session-1' });
    mocks.openMainSession.mockResolvedValue(undefined);
    mocks.apiListen.mockImplementation(() => vi.fn());
    useMiniAppStore.setState({ composerClaims: {} });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    useMiniAppStore.setState({ composerClaims: {} });
    vi.clearAllMocks();
  });

  it('keeps a claimed and bound strict Agent run in the floating bubble', async () => {
    await act(async () => {
      root.render(<BridgeHarness />);
    });
    const iframe = container.querySelector('iframe') as HTMLIFrameElement;

    await dispatchRpc(iframe, 1, 'chat.claimComposer');
    await dispatchRpc(iframe, 2, 'agent.ensureSession', {
      sessionName: 'Market Lens',
      appDataWorkspace: 'chat',
    });

    expect(mocks.agentEnsureSession).toHaveBeenCalledTimes(1);
    expect(mocks.openMainSession).not.toHaveBeenCalled();

    await dispatchRpc(iframe, 3, 'chat.focusSession', { sessionId: 'session-1' });
    expect(useMiniAppStore.getState().composerClaims[app.id]?.sessionId).toBe('session-1');

    await dispatchRpc(iframe, 4, 'agent.run', {
      sessionId: 'session-1',
      prompt: 'Summarize the market',
      displayText: 'Summarize the market',
    });

    expect(mocks.agentRun).toHaveBeenCalledTimes(1);
    expect(mocks.openMainSession).not.toHaveBeenCalled();
  });

  it('opens an unbound strict Agent run in the main session scene', async () => {
    await act(async () => {
      root.render(<BridgeHarness />);
    });
    const iframe = container.querySelector('iframe') as HTMLIFrameElement;

    await dispatchRpc(iframe, 1, 'chat.claimComposer');
    await dispatchRpc(iframe, 2, 'agent.ensureSession', {
      sessionName: 'Market Lens',
      appDataWorkspace: 'chat',
    });
    await dispatchRpc(iframe, 3, 'agent.run', {
      sessionId: 'session-1',
      prompt: 'Summarize the market',
    });

    expect(mocks.openMainSession).toHaveBeenCalledWith('session-1');
    expect(mocks.agentRun).toHaveBeenCalledTimes(1);
  });

  it('leaves the tool loop of a strict Agent run to the backend allowlist', async () => {
    await act(async () => {
      root.render(<BridgeHarness />);
    });
    const iframe = container.querySelector('iframe') as HTMLIFrameElement;

    await dispatchRpc(iframe, 1, 'agent.ensureSession', {
      sessionName: 'Market Lens',
      appDataWorkspace: 'chat',
    });
    await dispatchRpc(iframe, 2, 'agent.run', {
      sessionId: 'session-1',
      prompt: 'Summarize the market',
    });

    // The host used to force enableTools=false for marketplace MiniApps, which
    // also killed WebSearch/WebFetch. Tool access is now scoped by the backend
    // research allowlist instead, so the bridge must not disable the loop.
    expect(mocks.agentEnsureSession.mock.calls[0][1].enableTools).toBeUndefined();
    expect(mocks.agentRun.mock.calls[0][3].enableTools).toBeUndefined();
  });
});
