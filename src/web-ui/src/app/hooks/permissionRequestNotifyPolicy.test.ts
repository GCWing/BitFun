import { describe, expect, it, vi } from 'vitest';
import {
  buildPermissionRequestNotificationCopy,
  PermissionRequestNotificationBatcher,
  PERMISSION_REQUEST_NOTIFY_COALESCE_MS,
  shouldSendPermissionRequestNotification,
} from './permissionRequestNotifyPolicy';

describe('PermissionRequestNotificationBatcher', () => {
  it('de-duplicates requests and aggregates requests from the same session round', () => {
    vi.useFakeTimers();
    const onBatchReady = vi.fn();
    const batcher = new PermissionRequestNotificationBatcher(onBatchReady);

    batcher.enqueue({ requestId: 'request-1', sessionId: 'session-1', roundId: 'round-1' });
    batcher.enqueue({ requestId: 'request-2', sessionId: 'session-1', roundId: 'round-1' });
    batcher.enqueue({ requestId: 'request-1', sessionId: 'session-1', roundId: 'round-1' });

    vi.advanceTimersByTime(PERMISSION_REQUEST_NOTIFY_COALESCE_MS);

    expect(onBatchReady).toHaveBeenCalledTimes(1);
    expect(onBatchReady).toHaveBeenCalledWith({ requestCount: 2 });
    batcher.dispose();
    vi.useRealTimers();
  });

  it('keeps requests from different rounds in separate notifications', () => {
    vi.useFakeTimers();
    const onBatchReady = vi.fn();
    const batcher = new PermissionRequestNotificationBatcher(onBatchReady);

    batcher.enqueue({ requestId: 'request-1', sessionId: 'session-1', roundId: 'round-1' });
    batcher.enqueue({ requestId: 'request-2', sessionId: 'session-1', roundId: 'round-2' });

    vi.advanceTimersByTime(PERMISSION_REQUEST_NOTIFY_COALESCE_MS);

    expect(onBatchReady).toHaveBeenCalledTimes(2);
    expect(onBatchReady).toHaveBeenCalledWith({ requestCount: 1 });
    batcher.dispose();
    vi.useRealTimers();
  });
});

describe('permission request notification policy', () => {
  it('requires both a background window and an enabled preference', () => {
    expect(shouldSendPermissionRequestNotification({
      isBackground: false,
      notificationsEnabled: true,
    })).toBe(false);
    expect(shouldSendPermissionRequestNotification({
      isBackground: true,
      notificationsEnabled: false,
    })).toBe(false);
    expect(shouldSendPermissionRequestNotification({
      isBackground: true,
      notificationsEnabled: true,
    })).toBe(true);
  });

  it('uses generic copy without accepting request details', () => {
    const t = (key: string, options?: Record<string, unknown>) => {
      if (key === 'notify.permissionRequestTitle') return 'OpenBitFun requires approval';
      if (key === 'notify.permissionRequestBody') return 'An action is waiting for your approval.';
      return `${options?.count} actions are waiting for your approval.`;
    };

    expect(buildPermissionRequestNotificationCopy({ requestCount: 2, t })).toEqual({
      title: 'OpenBitFun requires approval',
      body: '2 actions are waiting for your approval.',
    });
  });
});
