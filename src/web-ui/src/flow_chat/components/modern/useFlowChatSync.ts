/**
 * FlowChat store synchronization effects.
 */

import { useEffect } from 'react';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { flowChatStore } from '../../store/FlowChatStore';
import { startAutoSync } from '../../services/storeSync';

export function useFlowChatSync(): void {
  useEffect(() => {
    const unsubscribe = startAutoSync();
    return () => {
      unsubscribe();
    };
  }, []);

  useEffect(() => {
    const unlisten = agentAPI.onSessionTitleGenerated((event) => {
      flowChatStore.updateSessionTitle(
        event.sessionId,
        event.title,
        'generated',
      );
    });

    return () => {
      unlisten();
    };
  }, []);
}
