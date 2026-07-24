// @vitest-environment jsdom

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import SessionListPage, {
  captureSessionListOwnerEpoch,
} from '../../../../../mobile-web/src/pages/SessionListPage';
import type { RemoteSessionManager } from '../../../../../mobile-web/src/services/RemoteSessionManager';
import { useMobileStore } from '../../../../../mobile-web/src/services/store';

vi.mock('../../../../../mobile-web/src/i18n', () => ({
  useI18n: () => ({
    language: 'en-US',
    toggleLanguage: vi.fn(),
    t: (key: string) => key,
    formatDate: () => '',
  }),
}));

vi.mock('../../../../../mobile-web/src/theme', () => ({
  useTheme: () => ({ isDark: false, toggleTheme: vi.fn() }),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => { resolve = res; });
  return { promise, resolve };
}

async function flushPromises(): Promise<void> {
  await act(async () => {
    for (let index = 0; index < 8; index += 1) await Promise.resolve();
  });
}

describe('SessionList target initialization ownership', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useMobileStore.getState().resetConnectionState();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('does not let an old timer closure borrow the mutable new owner epoch', () => {
    const manager = {
      controlTargetEpoch: 2,
    } as unknown as RemoteSessionManager;
    const mutableOwnerAfterRender = {
      sessionMgr: manager,
      epoch: 2,
      active: true,
    };

    expect(captureSessionListOwnerEpoch(
      mutableOwnerAfterRender,
      manager,
      1,
    )).toBeNull();
    expect(captureSessionListOwnerEpoch(
      mutableOwnerAfterRender,
      manager,
      2,
    )).toBe(2);
  });

  it('blocks A actions while B initializes and restores them only after B is ready', async () => {
    let epoch = 0;
    const targetListeners = new Set<() => void>();
    const deviceBInfo = deferred<any>();
    const getWorkspaceInfo = vi.fn()
      .mockResolvedValueOnce({
        resp: 'workspace_info',
        has_workspace: true,
        workspace_kind: 'assistant',
        path: '/assistant-a',
        project_name: 'Assistant A',
      })
      .mockImplementationOnce(() => deviceBInfo.promise);
    const listSessions = vi.fn().mockResolvedValue({
      resp: 'sessions',
      sessions: [],
      has_more: false,
    });
    const createSession = vi.fn().mockResolvedValue('session-b');
    const manager = {
      get controlTargetEpoch() { return epoch; },
      onControlTargetChange: (listener: () => void) => {
        targetListeners.add(listener);
        return () => targetListeners.delete(listener);
      },
      getWorkspaceInfo,
      listSessions,
      listRecentWorkspaces: vi.fn().mockResolvedValue([]),
      listAssistants: vi.fn().mockResolvedValue([]),
      createSession,
    } as unknown as RemoteSessionManager;
    const onSelectSession = vi.fn();

    await act(async () => {
      root.render(
        <SessionListPage
          sessionMgr={manager}
          onSelectSession={onSelectSession}
          onOpenWorkspace={vi.fn()}
          onDisconnect={vi.fn()}
        />,
      );
    });
    await flushPromises();

    const initialSearch = container.querySelector<HTMLInputElement>('.session-list__search-input');
    expect(initialSearch?.disabled).toBe(false);
    expect(container.textContent).toContain('sessions.clawSession');

    await act(async () => {
      epoch = 1;
      for (const listener of targetListeners) listener();
    });

    const pendingSearch = container.querySelector<HTMLInputElement>('.session-list__search-input');
    expect(pendingSearch?.disabled).toBe(true);
    expect(createSession).not.toHaveBeenCalled();
    expect(
      [...container.querySelectorAll<HTMLButtonElement>('.session-list__mode-toggle-btn')]
        .every((button) => button.disabled),
    ).toBe(true);

    deviceBInfo.resolve({
      resp: 'workspace_info',
      has_workspace: true,
      workspace_kind: 'assistant',
      path: '/assistant-b',
      project_name: 'Assistant B',
    });
    await flushPromises();

    const readySearch = container.querySelector<HTMLInputElement>('.session-list__search-input');
    expect(readySearch?.disabled).toBe(false);
    const createClaw = [...container.querySelectorAll<HTMLButtonElement>('button')]
      .find((button) => button.textContent?.includes('sessions.clawSession'));
    expect(createClaw?.disabled).toBe(false);
    await act(async () => { createClaw?.click(); });
    await flushPromises();

    expect(createSession).toHaveBeenCalledWith(
      'claw',
      undefined,
      '/assistant-b',
      undefined,
    );
    expect(onSelectSession).toHaveBeenCalledWith(
      'session-b',
      'sessions.remoteClawSession',
      true,
    );
  });
});
