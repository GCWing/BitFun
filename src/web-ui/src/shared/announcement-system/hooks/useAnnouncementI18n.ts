import { useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';

/**
 * Announcement strings live under the `notifications` JSON bundle.
 * Resolving via `notifications:…` ensures the correct namespace even when
 * `useTranslation` is bound to `common` (nested `announcements.*` keys would
 * otherwise miss and echo the full key).
 */
export function useAnnouncementI18n() {
  const { t: baseT, ...rest } = useI18n();

  const t = useCallback(
    (key: string, options?: Record<string, unknown>) =>
      String(baseT(`notifications:${key}`, options as never)),
    [baseT]
  );

  return { ...rest, t };
}
