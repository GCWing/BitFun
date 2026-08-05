import { afterEach, describe, expect, it, vi } from 'vitest';
import { notificationStore } from '../store/NotificationStore';
import { notificationService } from './NotificationService';

describe('NotificationService error toast', () => {
  afterEach(() => {
    vi.useRealTimers();
    notificationService.dismissAll();
  });

  it('uses the default toast duration and closes automatically', () => {
    vi.useFakeTimers();
    const id = notificationService.error('Failed');

    expect(notificationStore.getState().activeNotifications.some((item) => item.id === id)).toBe(true);

    vi.advanceTimersByTime(notificationStore.getState().config.defaultDuration);

    expect(notificationStore.getState().activeNotifications.some((item) => item.id === id)).toBe(false);
  });

  it('remains open when duration is explicitly zero', () => {
    vi.useFakeTimers();
    const id = notificationService.error('Failed', { duration: 0 });

    vi.advanceTimersByTime(notificationStore.getState().config.defaultDuration * 2);

    expect(notificationStore.getState().activeNotifications.some((item) => item.id === id)).toBe(true);
  });
});
