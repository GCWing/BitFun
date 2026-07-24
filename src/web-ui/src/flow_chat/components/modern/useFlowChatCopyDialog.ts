/**
 * Copy-dialog event handling for FlowChat.
 */

import { useEffect } from 'react';
import { globalEventBus } from '@/infrastructure/event-bus';
import { notificationService } from '@/shared/notification-system';
import { getElementText, copyTextToClipboard } from '@/shared/utils/textSelection';
import { createLogger } from '@/shared/utils/logger';
import { FlowChatStore } from '../../store/FlowChatStore';
import { collectAssistantText } from '../../utils/collectAssistantText';

const log = createLogger('useFlowChatCopyDialog');

export function extractAssistantText(turnId: string, mode: 'all' | 'final' = 'all'): string {
  const flowChatStore = FlowChatStore.getInstance();
  const state = flowChatStore.getState();

  let targetSession = null;
  for (const [, session] of state.sessions) {
    if (session.dialogTurns.some((turn: any) => turn.id === turnId)) {
      targetSession = session;
      break;
    }
  }
 
  if (!targetSession) return '';
 
  const dialogTurn = targetSession.dialogTurns.find((turn: any) => turn.id === turnId);
  if (!dialogTurn) return '';
 
  return collectAssistantText(dialogTurn, mode);
}

export function useFlowChatCopyDialog(): void {
  useEffect(() => {
    const unsubscribe = globalEventBus.on('flowchat:copy-dialog', ({ dialogTurn, mode }) => {
      if (!dialogTurn) {
        log.warn('Copy failed: dialog element not provided');
        return;
      }

      const dialogElement = dialogTurn as HTMLElement;
      let fullText = '';
      
      const turnId = dialogElement.getAttribute('data-turn-id');
      if (turnId) {
        fullText = extractAssistantText(turnId, mode === 'final' ? 'final' : 'all');
      }
      
      if (!fullText) {
        fullText = getElementText(dialogElement);
      }

      if (!fullText || fullText.trim().length === 0) {
        notificationService.warning('Dialog is empty, nothing to copy');
        return;
      }

      copyTextToClipboard(fullText).then(success => {
        if (!success) {
          notificationService.error('Copy failed. Please try again.');
        }
      });
    });

    return unsubscribe;
  }, []);
}
