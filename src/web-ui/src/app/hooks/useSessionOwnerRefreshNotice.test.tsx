// @vitest-environment jsdom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  listen: vi.fn(),
  persistent: vi.fn(),
  unlisten: vi.fn(),
}));

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: { listen: mocks.listen },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: { persistent: mocks.persistent },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => `translated:${key}`,
  }),
}));

import { SESSION_OWNER_REFRESH_EVENT } from '@/infrastructure/api/adapters/websocket-adapter';
import { useSessionOwnerRefreshNotice } from './useSessionOwnerRefreshNotice';

function Harness({ reload }: { reload: () => void }) {
  useSessionOwnerRefreshNotice(reload);
  return null;
}

describe('useSessionOwnerRefreshNotice', () => {
  let container: HTMLDivElement;
  let root: Root;
  let listener: (() => void) | undefined;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.unlisten.mockReset();
    mocks.persistent.mockReset().mockReturnValue('notice-1');
    mocks.listen.mockReset().mockImplementation((_event, callback) => {
      listener = callback;
      return mocks.unlisten;
    });
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  it('exposes an actionable page reload when Session projections cannot be recovered', async () => {
    const reload = vi.fn();
    await act(async () => root.render(<Harness reload={reload} />));

    expect(mocks.listen).toHaveBeenCalledWith(
      SESSION_OWNER_REFRESH_EVENT,
      expect.any(Function),
    );
    act(() => listener?.());

    expect(mocks.persistent).toHaveBeenCalledWith({
      type: 'warning',
      title: 'translated:sessionOwnerRefresh.title',
      message: 'translated:sessionOwnerRefresh.message',
      closable: false,
      actions: [{
        label: 'translated:sessionOwnerRefresh.reload',
        variant: 'primary',
        onClick: expect.any(Function),
      }],
      metadata: { source: 'app-server-session-owner-refresh' },
    });

    const notification = mocks.persistent.mock.calls[0][0];
    notification.actions[0].onClick();
    expect(reload).toHaveBeenCalledOnce();

    act(() => listener?.());
    expect(mocks.persistent).toHaveBeenCalledOnce();
  });

  it('removes the transport listener when the application shell unmounts', async () => {
    await act(async () => root.render(<Harness reload={() => {}} />));
    await act(async () => root.unmount());
    root = createRoot(container);
    expect(mocks.unlisten).toHaveBeenCalledOnce();
  });
});
