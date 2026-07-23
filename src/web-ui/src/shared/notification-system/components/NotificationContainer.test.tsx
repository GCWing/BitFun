import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Notification } from '../types';
import { useActiveNotifications } from '../hooks/useNotificationState';
import { NotificationContainer } from './NotificationContainer';

vi.mock('../hooks/useNotificationState', () => ({
  useActiveNotifications: vi.fn(),
}));

vi.mock('./NotificationItem', () => ({
  NotificationItem: ({ notification }: { notification: Notification }) => (
    <div data-variant="toast">{notification.message}</div>
  ),
}));

vi.mock('./ProgressNotification', () => ({
  ProgressNotification: ({ notification }: { notification: Notification }) => (
    <div data-variant="progress">{notification.message}</div>
  ),
}));

vi.mock('./LoadingNotification', () => ({
  LoadingNotification: ({ notification }: { notification: Notification }) => (
    <div data-variant="loading">{notification.message}</div>
  ),
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const notification = (variant: Notification['variant'], message: string): Notification => ({
  id: `${variant}-${message}`,
  type: 'info',
  variant,
  title: 'Test',
  message,
  timestamp: 1,
  status: 'active',
});

describe('NotificationContainer', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>');
    globalThis.window = dom.window as unknown as Window & typeof globalThis;
    globalThis.document = dom.window.document;
    container = document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    vi.clearAllMocks();
    dom.window.close();
  });

  it('keeps task notifications in the notification center instead of the toast stack', () => {
    vi.mocked(useActiveNotifications).mockReturnValue([
      notification('toast', 'Saved'),
      notification('progress', 'Indexing'),
      notification('loading', 'Connecting'),
    ]);

    act(() => root.render(<NotificationContainer />));

    expect(container.querySelector('[data-variant="toast"]')?.textContent).toBe('Saved');
    expect(container.querySelector('[data-variant="progress"]')).toBeNull();
    expect(container.querySelector('[data-variant="loading"]')).toBeNull();
  });

  it('keeps silent notifications out of the toast stack', () => {
    vi.mocked(useActiveNotifications).mockReturnValue([notification('silent', 'Background')]);

    act(() => root.render(<NotificationContainer />));

    expect(container.querySelector('.notification-container')).toBeNull();
  });
});
