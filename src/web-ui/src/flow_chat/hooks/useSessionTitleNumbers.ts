import { useMemo } from 'react';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import type { Session } from '../types/flow-chat';
import { sessionTitleNumbers } from '../utils/sessionTitlePresentation';

export function useSessionTitleNumbers(sessions: ReadonlyMap<string, Session>): Map<string, string> {
  const { formatNumber } = useI18n('flow-chat');
  return useMemo(() => new Map(
    Array.from(sessionTitleNumbers(sessions.values()), ([id, number]) => [
      id, formatNumber(number, { minimumIntegerDigits: 2, useGrouping: false }),
    ]),
  ), [sessions, formatNumber]);
}
