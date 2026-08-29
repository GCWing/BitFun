// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { AcpClientInfo } from '@/infrastructure/api/service-api/ACPClientAPI';

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string, options?: { agentName?: string }) => {
      if (key === 'nav.sessions.acpSessions') return 'ACP sessions';
      if (key === 'nav.sessions.newExternalAgentSessionShort') {
        return `${options?.agentName ?? ''} Session`;
      }
      if (key === 'app.loading') return 'Loading';
      return key;
    },
  }),
}));

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

import WorkspaceAcpSessionSubmenu from './WorkspaceAcpSessionSubmenu';

const clients: AcpClientInfo[] = [
  {
    id: 'codex',
    name: 'Codex',
    command: 'codex-acp',
    args: [],
    enabled: true,
    readonly: false,
    permissionMode: 'ask',
    status: 'configured',
    toolName: 'codex',
    sessionCount: 0,
  },
  {
    id: 'claude-code',
    name: 'Claude Code',
    command: 'claude-code-acp',
    args: [],
    enabled: true,
    readonly: false,
    permissionMode: 'ask',
    status: 'configured',
    toolName: 'claude-code',
    sessionCount: 0,
  },
];

describe('WorkspaceAcpSessionSubmenu', () => {
  let container: HTMLDivElement;
  let root: Root;
  let onSelect: ReturnType<typeof vi.fn>;

  const render = (nextClients: readonly AcpClientInfo[] = clients, loading = false) => {
    act(() => {
      root.render(
        <WorkspaceAcpSessionSubmenu
          clients={nextClients}
          loading={loading}
          onSelect={onSelect}
        />,
      );
    });
  };

  beforeEach(() => {
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    onSelect = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  });

  it('keeps ACP clients out of the project menu until the ACP group is opened', () => {
    render();

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="nav-workspace-menu-acp-sessions"]',
    );
    expect(trigger?.textContent).toContain('ACP sessions');
    expect(trigger?.getAttribute('aria-haspopup')).toBe('menu');
    expect(document.querySelector('[data-testid="nav-workspace-menu-create-acp-session"]')).toBeNull();

    act(() => trigger!.click());

    const submenu = document.querySelector('[data-testid="nav-workspace-menu-acp-submenu"]');
    const options = document.querySelectorAll<HTMLButtonElement>(
      '[data-testid="nav-workspace-menu-create-acp-session"]',
    );
    expect(submenu?.getAttribute('role')).toBe('menu');
    expect(options).toHaveLength(2);
    expect(submenu?.textContent).toContain('Codex Session');
    expect(submenu?.textContent).toContain('Claude Code Session');

    act(() => options[1]!.click());
    expect(onSelect).toHaveBeenCalledWith(clients[1]);
  });

  it('opens with ArrowRight and returns focus to the ACP group with ArrowLeft', () => {
    render();
    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="nav-workspace-menu-acp-sessions"]',
    );
    trigger?.focus();

    act(() => {
      trigger!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    });

    const submenu = document.querySelector<HTMLDivElement>(
      '[data-testid="nav-workspace-menu-acp-submenu"]',
    );
    expect(submenu).not.toBeNull();
    expect(document.activeElement?.getAttribute('data-acp-client-id')).toBe('codex');

    act(() => {
      submenu!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    });

    expect(document.querySelector('[data-testid="nav-workspace-menu-acp-submenu"]')).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it('keeps loading and empty states inside the ACP group boundary', () => {
    render([], true);
    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="nav-workspace-menu-acp-sessions"]',
    );
    expect(trigger).not.toBeNull();

    act(() => trigger!.click());
    expect(document.querySelector('[data-testid="nav-workspace-menu-acp-submenu"]')?.textContent)
      .toContain('Loading');

    render([], false);
    expect(container.querySelector('[data-testid="nav-workspace-menu-acp-sessions"]')).toBeNull();
    expect(document.querySelector('[data-testid="nav-workspace-menu-acp-submenu"]')).toBeNull();
  });
});
