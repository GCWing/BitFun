/**
 * Copy-dialog hook.
 * Centralizes the copy-dialog event handling to avoid duplicate listeners.
 */

import { useEffect } from 'react';
import { globalEventBus } from '../../infrastructure/event-bus';
import { getElementText, copyTextToClipboard } from '../../shared/utils/textSelection';
import { notificationService } from '../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useCopyDialog');

/**
 * Listen for flowchat:copy-dialog and copy text from the DOM element.
 */
export const useCopyDialog = () => {
  useEffect(() => {
    const unsubscribe = globalEventBus.on('flowchat:copy-dialog', ({ dialogTurn }) => {
      if (!dialogTurn) {
        log.warn('Dialog turn not provided');
        return;
      }

      const dialogElement = dialogTurn as HTMLElement;
      const fullText = getElementText(dialogElement);
      
      if (!fullText || fullText.trim().length === 0) {
        notificationService.warning('Dialog content is empty; nothing to copy.');
        return;
      }

      copyTextToClipboard(fullText).then(success => {
        if (!success) {
          notificationService.error('Copy failed. Please try again.');
        }
        // Keep the UI quiet on success.
        // Optionally:
        // else {
        //   notificationService.success('Dialog copied to clipboard.');
        // }
      }).catch(error => {
        log.error('Copy failed', error);
        notificationService.error('Copy failed. Please try again.');
      });
    });

    return () => {
      unsubscribe();
    };
  }, []);
};

