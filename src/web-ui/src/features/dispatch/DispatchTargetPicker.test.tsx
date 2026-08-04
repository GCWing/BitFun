// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DispatchTargetPicker } from './DispatchTargetPicker';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/infrastructure/account/useAccountLoginState', () => ({
  useAccountLoginState: () => ({ loggedIn: false }),
}));

vi.mock('./useDispatchTargets', () => ({
  useDispatchTargets: () => ({
    targets: [],
    loading: false,
    error: false,
    refresh: vi.fn(async () => undefined),
  }),
}));

vi.mock('@/features/ssh-remote/SSHConnectionDialog', () => ({
  SSHConnectionDialog: () => null,
}));

vi.mock('./DispatchInstallDialog', () => ({
  DispatchInstallDialog: () => null,
}));

const rect = (
  top: number,
  left: number,
  width: number,
  height: number,
): DOMRect => ({
  top,
  bottom: top + height,
  left,
  right: left + width,
  width,
  height,
  x: left,
  y: top,
  toJSON() { return this; },
} as DOMRect);

describe('DispatchTargetPicker overlay', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 800 });
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 800 });
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function () {
      if (this.dataset.testid === 'chat-input-dispatch-trigger') {
        return rect(500, 420, 120, 40);
      }
      if (this.classList.contains('dispatch-target-picker__menu')) {
        return rect(0, 0, 300, 200);
      }
      return rect(0, 0, 0, 0);
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    document.querySelector('[data-bf-overlay-host="true"]')?.remove();
    container.remove();
    vi.restoreAllMocks();
  });

  it('anchors the portalled menu to the actual dispatch trigger', async () => {
    await act(async () => {
      root.render(
        <DispatchTargetPicker
          target={{ kind: 'local' }}
          locked={false}
          onSelectTarget={vi.fn()}
        />,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-input-dispatch-trigger"]',
    );
    await act(async () => trigger?.click());

    const menu = document.querySelector<HTMLElement>('[data-testid="dispatch-target-menu"]');
    expect(menu?.parentElement?.getAttribute('data-bf-overlay-host')).toBe('true');
    expect(menu?.style.visibility).toBe('visible');
    expect(menu?.style.left).toBe('240px');
    expect(menu?.style.top).toBe('293px');
  });
});
