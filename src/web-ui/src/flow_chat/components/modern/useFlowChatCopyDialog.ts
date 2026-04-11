/**
 * Copy-dialog event handling for FlowChat.
 */

import { useEffect } from 'react';
import { globalEventBus } from '@/infrastructure/event-bus';
import { notificationService } from '@/shared/notification-system';
import { getElementText, copyTextToClipboard } from '@/shared/utils/textSelection';
import { createLogger } from '@/shared/utils/logger';
import { FlowChatStore } from '../../store/FlowChatStore';
import { i18nService } from '@/infrastructure/i18n';

const log = createLogger('useFlowChatCopyDialog');

function extractDialogTurnContent(turnId: string): string {
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
  
  const contentParts: string[] = [];
  
  if (dialogTurn.userMessage?.content) {
    contentParts.push(`${i18nService.t('flow-chat:modelRound.userLabel')}\n${dialogTurn.userMessage.content}`);
  }
  
  dialogTurn.modelRounds.forEach((modelRound: any) => {
    const roundContent: string[] = [];
    
    modelRound.items.forEach((item: any) => {
      if (item.type === 'text' && item.content?.trim()) {
        roundContent.push(item.content.trim());
      } else if (item.type === 'thinking' && item.content?.trim()) {
        roundContent.push(`[Thinking]\n${item.content.trim()}`);
      } else if (item.type === 'tool' && item.toolCall) {
        const toolName = item.toolName || i18nService.t('flow-chat:copyOutput.unknownTool');
        let toolContent = i18nService.t('flow-chat:modelRound.toolCallLabel', { name: toolName }) + '\n';
        
        if (item.toolCall.input) {
          const inputStr = typeof item.toolCall.input === 'string'
            ? item.toolCall.input
            : JSON.stringify(item.toolCall.input, null, 2);
          toolContent += `\n[Input]\n\`\`\`json\n${inputStr}\n\`\`\`\n`;
        }
        
        if (item.toolResult) {
          if (item.toolResult.error) {
            toolContent += `\n[Error]\n${item.toolResult.error}\n`;
          } else if (item.toolResult.result !== undefined) {
            const resultStr = typeof item.toolResult.result === 'string'
              ? item.toolResult.result
              : JSON.stringify(item.toolResult.result, null, 2);
            toolContent += `\n[Result]\n\`\`\`\n${resultStr}\n\`\`\`\n`;
          }
        }
        
        roundContent.push(toolContent.trim());
      }
    });
    
    if (roundContent.length > 0) {
      contentParts.push(roundContent.join('\n\n'));
    }
  });
  
  return contentParts.join('\n\n---\n\n');
}

export function useFlowChatCopyDialog(): void {
  useEffect(() => {
    const unsubscribe = globalEventBus.on('flowchat:copy-dialog', ({ dialogTurn }) => {
      if (!dialogTurn) {
        log.warn('Copy failed: dialog element not provided');
        return;
      }

      const dialogElement = dialogTurn as HTMLElement;
      let fullText = '';
      
      const turnId = dialogElement.getAttribute('data-turn-id');
      if (turnId) {
        fullText = extractDialogTurnContent(turnId);
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
