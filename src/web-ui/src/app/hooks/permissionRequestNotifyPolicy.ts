import type { PermissionRequestNotificationEvent } from '@/infrastructure/event-bus';

export const PERMISSION_REQUEST_NOTIFY_COALESCE_MS = 400;

const MAX_DEDUPLICATED_REQUESTS = 1_000;

interface PermissionRequestNotificationInput {
  isBackground: boolean;
  notificationsEnabled?: boolean;
}

interface PermissionRequestNotificationCopyInput {
  requestCount: number;
  t: (key: string, options?: Record<string, unknown>) => string;
}

export interface PermissionRequestNotificationBatch {
  requestCount: number;
}

interface PendingBatch {
  requestIds: Set<string>;
  timer: ReturnType<typeof setTimeout>;
}

/**
 * Keeps duplicate runtime events from creating repeated OS notifications and
 * groups approval prompts raised by the same model round into one notification.
 */
export class PermissionRequestNotificationBatcher {
  private readonly seenRequestIds = new Set<string>();
  private readonly pendingBatches = new Map<string, PendingBatch>();

  constructor(
    private readonly onBatchReady: (batch: PermissionRequestNotificationBatch) => void,
    private readonly coalesceMs = PERMISSION_REQUEST_NOTIFY_COALESCE_MS,
  ) {}

  enqueue(request: PermissionRequestNotificationEvent): void {
    if (!request.requestId || this.seenRequestIds.has(request.requestId)) {
      return;
    }

    this.rememberRequestId(request.requestId);

    const batchKey = request.roundId
      ? JSON.stringify([request.sessionId, request.roundId])
      : `request:${request.requestId}`;
    const existingBatch = this.pendingBatches.get(batchKey);
    if (existingBatch) {
      existingBatch.requestIds.add(request.requestId);
      return;
    }

    const requestIds = new Set([request.requestId]);
    const timer = setTimeout(() => {
      const batch = this.pendingBatches.get(batchKey);
      if (!batch) {
        return;
      }
      this.pendingBatches.delete(batchKey);
      this.onBatchReady({ requestCount: batch.requestIds.size });
    }, this.coalesceMs);

    this.pendingBatches.set(batchKey, { requestIds, timer });
  }

  dispose(): void {
    for (const batch of this.pendingBatches.values()) {
      clearTimeout(batch.timer);
    }
    this.pendingBatches.clear();
  }

  private rememberRequestId(requestId: string): void {
    this.seenRequestIds.add(requestId);
    while (this.seenRequestIds.size > MAX_DEDUPLICATED_REQUESTS) {
      const oldestRequestId = this.seenRequestIds.values().next().value;
      if (!oldestRequestId) {
        return;
      }
      this.seenRequestIds.delete(oldestRequestId);
    }
  }
}

export function shouldSendPermissionRequestNotification({
  isBackground,
  notificationsEnabled,
}: PermissionRequestNotificationInput): boolean {
  return isBackground && notificationsEnabled !== false;
}

export function buildPermissionRequestNotificationCopy({
  requestCount,
  t,
}: PermissionRequestNotificationCopyInput): { title: string; body: string } {
  return {
    title: t('notify.permissionRequestTitle'),
    body: requestCount === 1
      ? t('notify.permissionRequestBody')
      : t('notify.permissionRequestBatchBody', { count: requestCount }),
  };
}
