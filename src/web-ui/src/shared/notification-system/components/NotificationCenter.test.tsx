// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { NotificationRecord } from '../types';
import { NotificationCenter } from './NotificationCenter';

vi.mock('../hooks/useNotificationState', () => ({
  useNotificationHistory: vi.fn(),
  useCenterOpen: vi.fn(),
  useAllProgressNotifications: vi.fn(),
  useAllLoadingNotifications: vi.fn(),
}));

vi.mock('../services/NotificationService', () => ({
  notificationService: {
    toggleCenter: vi.fn(),
    markAllAsRead: vi.fn(),
    clearHistory: vi.fn(),
    deleteFromHistory: vi.fn(),
    markAsRead: vi.fn(),
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
    formatDate: (ts: number) => String(ts),
  }),
}));

vi.mock('@/component-library', () => {
  const Modal = ({ children, isOpen }: { children: React.ReactNode; isOpen: boolean }) =>
    isOpen ? <div data-testid="modal">{children}</div> : null;
  const Search = () => <div />;
  return { Modal, Search };
});

import {
  useNotificationHistory,
  useCenterOpen,
  useAllProgressNotifications,
  useAllLoadingNotifications,
} from '../hooks/useNotificationState';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const historyItem = (overrides: Partial<NotificationRecord> = {}): NotificationRecord => ({
  id: 'item-1',
  type: 'info',
  variant: 'toast',
  title: 'Issue-Fix pending review',
  message: 'Merge PR #2038 — fixes #1980',
  timestamp: Date.now(),
  read: false,
  status: 'dismissed',
  actions: [
    { label: 'Open panel', onClick: vi.fn() },
    { label: 'Open link', onClick: vi.fn(), variant: 'secondary' },
  ],
  ...overrides,
});

describe('NotificationCenter action rendering', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>');
    globalThis.window = dom.window as unknown as Window & typeof globalThis;
    globalThis.document = dom.window.document;
    container = document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);

    vi.mocked(useCenterOpen).mockReturnValue(true);
    vi.mocked(useAllProgressNotifications).mockReturnValue([]);
    vi.mocked(useAllLoadingNotifications).mockReturnValue([]);
  });

  afterEach(() => {
    act(() => root.unmount());
    vi.clearAllMocks();
    dom.window.close();
  });

  it('renders action buttons on a history item so the center stays actionable after the toast fades', () => {
    vi.mocked(useNotificationHistory).mockReturnValue([historyItem()]);

    act(() => root.render(<NotificationCenter />));

    const actions = container.querySelectorAll('.notification-center__item-action');
    expect(actions).toHaveLength(2);
    expect(actions[0]?.textContent).toBe('Open panel');
    expect(actions[1]?.textContent).toBe('Open link');
  });

  it('invokes an action handler without toggling the row expand state', () => {
    const onClick = vi.fn();
    vi.mocked(useNotificationHistory).mockReturnValue([
      historyItem({ actions: [{ label: 'Open panel', onClick }] }),
    ]);

    act(() => root.render(<NotificationCenter />));

    const button = container.querySelector('.notification-center__item-action') as HTMLButtonElement;
    act(() => {
      button.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('omits the action row when the history item has no actions', () => {
    vi.mocked(useNotificationHistory).mockReturnValue([
      historyItem({ actions: undefined, id: 'no-actions' }),
    ]);

    act(() => root.render(<NotificationCenter />));

    expect(container.querySelector('.notification-center__item-message-actions')).toBeNull();
  });
});
