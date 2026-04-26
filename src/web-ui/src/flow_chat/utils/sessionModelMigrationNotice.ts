import type { SessionModelAutoMigratedEvent } from '@/infrastructure/api/service-api/AgentAPI';

type SessionModelMigrationReasonKey =
  | 'modelUnavailableOnRestore'
  | 'modelReconciled'
  | 'fallback';

const REASON_COPY_KEY_BY_CODE: Record<string, SessionModelMigrationReasonKey> = {
  model_unavailable_on_restore: 'modelUnavailableOnRestore',
  model_reconciled: 'modelReconciled',
};

export const SESSION_MODEL_MIGRATION_NOTICE_WINDOW_MS = 2000;

export interface SessionModelMigrationNotice {
  title: string;
  message: string;
  dedupeKey: string;
}

type TranslateFn = (key: string, options?: Record<string, unknown>) => string;

export function normalizeSessionModelMigrationReason(
  reason?: string
): SessionModelMigrationReasonKey {
  if (!reason) {
    return 'fallback';
  }

  return REASON_COPY_KEY_BY_CODE[reason] ?? 'fallback';
}

export function buildSessionModelMigrationNotice(
  event: Pick<SessionModelAutoMigratedEvent, 'sessionId' | 'newModelId' | 'reason'>,
  t: TranslateFn
): SessionModelMigrationNotice {
  const reasonKey = normalizeSessionModelMigrationReason(event.reason);
  const description = t('flow-chat:model.autoMigrated.description');
  const reason = t(`flow-chat:model.autoMigrated.reasons.${reasonKey}`);

  return {
    title: t('flow-chat:model.autoMigrated.title'),
    message: `${description} ${reason}`.trim(),
    dedupeKey: [event.sessionId, event.newModelId || 'unknown', reasonKey].join(':'),
  };
}

export function shouldSuppressSessionModelMigrationNotice(
  recentNoticeTimestamps: Map<string, number>,
  dedupeKey: string,
  now: number = Date.now(),
  quietWindowMs: number = SESSION_MODEL_MIGRATION_NOTICE_WINDOW_MS
): boolean {
  for (const [key, timestamp] of recentNoticeTimestamps) {
    if (now - timestamp >= quietWindowMs) {
      recentNoticeTimestamps.delete(key);
    }
  }

  const previousTimestamp = recentNoticeTimestamps.get(dedupeKey);
  if (previousTimestamp !== undefined && now - previousTimestamp < quietWindowMs) {
    return true;
  }

  recentNoticeTimestamps.set(dedupeKey, now);
  return false;
}
