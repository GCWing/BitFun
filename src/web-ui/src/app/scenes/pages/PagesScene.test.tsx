// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import PagesScene from './PagesScene';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  accountStatus: vi.fn(),
  accountGetCredentialHint: vi.fn(),
  listPages: vi.fn(),
  createOpenLink: vi.fn(),
  update: vi.fn(),
}));

vi.mock('@/infrastructure/api/service-api/PageAPI', () => ({
  pageAPI: {
    listPages: mocks.listPages,
    listVersions: vi.fn(),
    createOpenLink: mocks.createOpenLink,
    update: mocks.update,
    deploy: vi.fn(),
    unpublish: vi.fn(),
    deleteVersion: vi.fn(),
    deletePage: vi.fn(),
  },
}));

vi.mock('@/infrastructure/api/service-api/RemoteConnectAPI', () => ({
  remoteConnectAPI: {
    accountStatus: mocks.accountStatus,
    accountGetCredentialHint: mocks.accountGetCredentialHint,
  },
}));

vi.mock('@/infrastructure/api/service-api/SystemAPI', () => ({
  systemAPI: { openExternal: vi.fn(), setClipboard: vi.fn() },
}));

vi.mock('@/infrastructure/i18n', () => {
  const t = (key: string) => key;
  return {
    useI18n: () => ({
      t,
      formatDate: () => 'date',
      formatNumber: (value: number) => String(value),
    }),
  };
});

vi.mock('@/shared/notification-system', () => ({
  useNotification: () => ({
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
  }),
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({ error: vi.fn() }),
}));

vi.mock('@/component-library', () => ({
  Button: ({ children, isLoading: _isLoading, ...props }: React.ButtonHTMLAttributes<HTMLButtonElement> & { isLoading?: boolean }) => (
    <button {...props}>{children}</button>
  ),
  Input: (props: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props} />,
  Select: () => <div />,
  confirmDanger: vi.fn(),
  confirmWarning: vi.fn(),
}));

vi.mock('@/app/components', () => ({
  GalleryLayout: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  GalleryPageHeader: ({ title }: { title: React.ReactNode }) => <header>{title}</header>,
  GalleryEmpty: ({ message, action, testId }: { message: React.ReactNode; action?: React.ReactNode; testId?: string }) => (
    <div data-testid={testId}>{message}{action}</div>
  ),
}));

describe('PagesScene initial loading', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.accountStatus.mockReset().mockResolvedValue({ logged_in: true, user_id: 'u1' });
    mocks.accountGetCredentialHint.mockReset().mockResolvedValue({ relay_url: 'https://relay.test' });
    mocks.listPages.mockReset().mockRejectedValue(new Error('relay unavailable'));
    mocks.createOpenLink.mockReset();
    mocks.update.mockReset();
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('attempts a failed initial load only once until the user retries', async () => {
    await act(async () => {
      root.render(<PagesScene isActive />);
      await Promise.resolve();
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    await act(async () => {
      await Promise.resolve();
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    // The failed relay call triggers one bounded auth re-check so an expired
    // session can switch to the sign-in state; it must not retry listPages.
    expect(mocks.accountStatus).toHaveBeenCalledTimes(2);
    expect(mocks.listPages).toHaveBeenCalledTimes(1);
    expect(container.querySelector('[data-testid="pages-error"]')).not.toBeNull();
  });

  it('locks every action on one Page while an operation is pending and exposes title editing', async () => {
    mocks.listPages.mockResolvedValue([{
      slug: 'demo',
      visibility: 'public',
      title: 'Demo',
      file_count: 1,
      total_bytes: 20,
      created_at: 1,
      updated_at: 1,
      url_path: '/p/alice/demo',
      preview_url_path: '/p/alice/demo/@v/v1',
      deployed_version_id: 'v1',
    }]);
    let resolveOpenLink: ((value: { open_url: string; expires_in_seconds: number }) => void) | undefined;
    mocks.createOpenLink.mockImplementation(() => new Promise((resolve) => {
      resolveOpenLink = resolve;
    }));

    await act(async () => {
      root.render(<PagesScene isActive />);
      await Promise.resolve();
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(container.querySelector('input[aria-label="titleField.inputAria"]')).not.toBeNull();
    const buttons = [...container.querySelectorAll('button')];
    const open = buttons.find((button) => button.textContent?.includes('actions.openProduction'));
    const remove = buttons.find((button) => button.textContent?.includes('actions.deletePage'));
    expect(open).toBeDefined();
    expect(remove).toBeDefined();

    await act(async () => {
      open?.click();
      await Promise.resolve();
    });
    expect(remove?.disabled).toBe(true);

    await act(async () => {
      resolveOpenLink?.({ open_url: 'https://relay.test/open', expires_in_seconds: 60 });
      await Promise.resolve();
    });
    expect(remove?.disabled).toBe(false);
  });
});
