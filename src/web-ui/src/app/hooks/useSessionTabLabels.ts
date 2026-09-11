import { useMemo, useSyncExternalStore } from 'react';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { SceneTab } from '../components/SceneBar/types';
import { getActiveSurfaceId } from '@/infrastructure/peer-device/deviceSurface';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { resolveSessionTitle } from '@/flow_chat/utils/sessionTitle';
import { sessionTitleNumbers } from '@/flow_chat/utils/sessionTitlePresentation';

export interface SessionTabLabel {
  title: string;
  number?: string;
}

function readSessionTabLabels(
  tabs: readonly SceneTab[],
  translate: (key: string, options?: Record<string, unknown>) => string,
  formatNumber: (number: number, options?: Intl.NumberFormatOptions) => string,
): Record<string, SessionTabLabel> {
  const source = flowChatStore.getState();
  const labels: Record<string, SessionTabLabel> = {};
  const numbers = sessionTitleNumbers(source.sessions.values());
  for (const tab of tabs) {
    if (!tab.session || tab.session.surfaceId !== getActiveSurfaceId()) continue;
    const session = source.sessions.get(tab.session.sessionId);
    if (!session) continue;
    const number = numbers.get(session.sessionId);
    labels[tab.id] = {
      title: resolveSessionTitle(session, translate),
      number: number === undefined ? undefined : formatNumber(number, { minimumIntegerDigits: 2, useGrouping: false }),
    };
  }
  return labels;
}

function subscribeToLabels(notify: () => void): () => void {
  return flowChatStore.subscribe(notify);
}

/** A synchronous resource projection with stable snapshots during streaming. */
export function useSessionTabLabels(tabs: readonly SceneTab[]): Record<string, SessionTabLabel> {
  const { t, formatNumber } = useI18n('flow-chat');
  const getSnapshot = useMemo(() => {
    let snapshot: Record<string, SessionTabLabel> = {};
    return () => {
      const next = readSessionTabLabels(tabs, t, formatNumber);
      if (Object.keys(snapshot).length !== Object.keys(next).length
        || Object.entries(next).some(([id, label]) => snapshot[id]?.title !== label.title || snapshot[id]?.number !== label.number)) {
        snapshot = next;
      }
      return snapshot;
    };
  }, [tabs, t, formatNumber]);
  return useSyncExternalStore(subscribeToLabels, getSnapshot, getSnapshot);
}
