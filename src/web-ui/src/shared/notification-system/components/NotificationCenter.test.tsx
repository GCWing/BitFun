// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NotificationCenter } from './NotificationCenter';

const service = vi.hoisted(() => ({
  toggleCenter: vi.fn(),
  markAllAsRead: vi.fn(),
  clearHistory: vi.fn(),
  deleteFromHistory: vi.fn(),
  markAsRead: vi.fn(),
}));

const notificationState = vi.hoisted(() => ({
  centerOpen: true,
  notificationHistory: [{
    id: 'history-1',
    type: 'info' as const,
    variant: 'toast' as const,
    title: 'Notice',
    message: 'Notification message',
    timestamp: Date.now(),
    read: false,
  }],
  activeNotifications: [],
  unreadCount: 1,
  config: {},
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
    formatDate: () => '12:00',
    formatNumber: (value: number) => String(value),
  }),
}));

vi.mock('../hooks/useNotificationState', () => ({
  useNotificationState: () => notificationState,
}));

vi.mock('../services/NotificationService', () => ({ notificationService: service }));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('NotificationCenter header', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    act(() => root.render(<NotificationCenter />));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.clearAllMocks();
  });

  it('provides a named dialog without an empty title bar above the notification header', () => {
    const dialog = document.querySelector('[role="dialog"]');
    expect(dialog?.getAttribute('aria-label')).toBe('components:notificationCenter.title');
    expect(dialog?.querySelector('header:empty')).toBeNull();
    expect(dialog?.querySelectorAll('h2')).toHaveLength(1);
  });

  it('keeps each header action independently operable', () => {
    const actions = [
      ['components:notificationCenter.actions.markAllRead', service.markAllAsRead],
      ['components:notificationCenter.actions.clearAll', service.clearHistory],
      ['common:actions.close', service.toggleCenter],
    ] as const;

    for (const [label, handler] of actions) {
      vi.clearAllMocks();
      const button = document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`);
      expect(button).not.toBeNull();
      act(() => button!.click());
      expect(handler).toHaveBeenCalledTimes(1);
      for (const [, otherHandler] of actions) {
        if (otherHandler !== handler) expect(otherHandler).not.toHaveBeenCalled();
      }
    }
    expect(service.toggleCenter).toHaveBeenCalledWith(false);
  });
});
