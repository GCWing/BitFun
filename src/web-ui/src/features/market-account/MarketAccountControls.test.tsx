// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MarketAccountControls } from './MarketAccountControls';

const mocks = vi.hoisted(() => ({
  account: {
    resolved: true,
    status: 'signed-out',
    me: null as null | {
      user: { githubId: number; login: string; avatarUrl: string };
      isAdmin: boolean;
    },
    lastError: undefined,
  },
  signIn: vi.fn(),
  cancelSignIn: vi.fn(),
  logout: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
}));

vi.mock('@/infrastructure/market-account', () => ({
  MarketAccountError: class MarketAccountError extends Error {
    constructor(public readonly code: string, message: string) {
      super(message);
    }
  },
  marketAccountService: {
    signIn: mocks.signIn,
    cancelSignIn: mocks.cancelSignIn,
    logout: mocks.logout,
  },
  useMarketAccount: () => mocks.account,
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock('@/shared/notification-system', () => ({
  useNotification: () => ({ success: mocks.success, error: mocks.error }),
}));

vi.mock('@/component-library', () => ({
  Avatar: ({ src, alt }: any) => <img src={src} alt={alt} />,
  Button: ({ children, ...props }: any) => <button {...props}>{children}</button>,
  Modal: ({ isOpen, title, children }: any) => isOpen ? (
    <section role="dialog" aria-label={title}>{children}</section>
  ) : null,
}));

describe('MarketAccountControls', () => {
  let container: HTMLDivElement;
  let root: ReturnType<typeof createRoot>;

  beforeEach(() => {
    mocks.account.resolved = true;
    mocks.account.status = 'signed-out';
    mocks.account.me = null;
    mocks.account.lastError = undefined;
    mocks.signIn.mockReset().mockResolvedValue({
      user: { githubId: 42, login: 'octocat', avatarUrl: '' },
      isAdmin: false,
    });
    mocks.cancelSignIn.mockReset();
    mocks.logout.mockReset().mockResolvedValue(undefined);
    mocks.success.mockReset();
    mocks.error.mockReset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('opens the shared GitHub login dialog and starts the vault-backed flow', async () => {
    await act(async () => root.render(<MarketAccountControls />));
    const signIn = [...container.querySelectorAll('button')]
      .find(button => button.textContent?.includes('market.signIn'));
    await act(async () => signIn?.click());

    expect(container.querySelector('[role="dialog"]')).not.toBeNull();
    const continueButton = [...container.querySelectorAll('button')]
      .find(button => button.textContent?.includes('market.account.continue'));
    await act(async () => continueButton?.click());
    expect(mocks.signIn).toHaveBeenCalledOnce();
  });

  it('shows the shared avatar menu and logs out through the same account service', async () => {
    mocks.account.status = 'signed-in';
    mocks.account.me = {
      user: { githubId: 42, login: 'octocat', avatarUrl: 'https://example.com/avatar.png' },
      isAdmin: false,
    };
    await act(async () => root.render(<MarketAccountControls />));

    const trigger = container.querySelector<HTMLButtonElement>('[aria-haspopup="menu"]');
    await act(async () => trigger?.click());
    const logout = container.querySelector<HTMLButtonElement>('[role="menuitem"]');
    expect(logout?.textContent).toContain('market.signOut');
    await act(async () => logout?.click());
    expect(mocks.logout).toHaveBeenCalledOnce();
  });
});
