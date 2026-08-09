/**
 * Copy-dialog event handling for FlowChat.
 */

import { useEffect } from 'react';
import { globalEventBus } from '@/infrastructure/event-bus';
import { notificationService } from '@/shared/notification-system';
import { getElementText, copyTextToClipboard } from '@/shared/utils/textSelection';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';
import { buildDialogTurnCopyText } from '../../utils/dialogTurnCopy';
import type { TranscriptExportScope } from '../../utils/dialogTranscriptExport';
import { buildTranscriptExportLabels } from '../../utils/transcriptExportLabels';

const log = createLogger('useFlowChatCopyDialog');

export function useFlowChatCopyDialog(): void {
  useEffect(() => {
    const unsubscribe = globalEventBus.on('flowchat:copy-dialog', ({ dialogTurn, scope }) => {
      if (!dialogTurn) {
        log.warn('Copy failed: dialog element not provided');
        return;
      }

      const exportScope: TranscriptExportScope = scope === 'result' ? 'result' : 'full';
      const dialogElement = dialogTurn as HTMLElement;
      let fullText = '';

      const turnId = dialogElement.getAttribute('data-turn-id');
      if (turnId) {
        fullText = buildDialogTurnCopyText(turnId, exportScope, buildTranscriptExportLabels());
      }

      // The DOM fallback cannot distinguish thinking / tool output, so it only
      // stands in for a full-process copy.
      if (!fullText && exportScope === 'full') {
        fullText = getElementText(dialogElement);
      }

      if (!fullText || fullText.trim().length === 0) {
        notificationService.warning(i18nService.t('flow-chat:transcriptExport.copyEmpty'));
        return;
      }

      copyTextToClipboard(fullText).then(success => {
        if (!success) {
          notificationService.error(i18nService.t('errors:general.copyFailed'));
        }
      });
    });

    return unsubscribe;
  }, []);
}
