/**
 * SelfControlEventListener — listens to backend self-control requests and
 * delegates execution to SelfControlService.
 */

import { api } from '@/infrastructure/api/service-api/ApiClient';
import { createLogger } from '@/shared/utils/logger';
import { SelfControlAPI } from '@/infrastructure/api/service-api/SelfControlAPI';
import {
  selfControlService,
  SelfControlError,
  type SelfControlAction,
  type SelfControlIncomingAction,
} from './SelfControlService';

const logger = createLogger('SelfControlEventListener');

export interface SelfControlRequestEvent {
  requestId: string;
  action: SelfControlIncomingAction | SelfControlAction;
}

let unlistenFn: (() => void) | null = null;

export function startSelfControlEventListener(): void {
  if (unlistenFn) {
    logger.warn('SelfControl event listener already running');
    return;
  }

  logger.info('Starting SelfControl event listener');

  unlistenFn = api.listen<SelfControlRequestEvent>('selfcontrol://request', (event) => {
    const { requestId, action } = event;
    logger.info('Received self-control request', {
      requestId,
      actionType: action.type ?? action.action ?? 'unknown',
    });

    void (async () => {
      try {
        const result = await selfControlService.executeAction(action);
        logger.info('Self-control action completed', { requestId, result });
        await SelfControlAPI.submitSelfControlResponse({
          requestId,
          success: true,
          result,
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : String(error);
        const code = error instanceof SelfControlError ? error.code : 'FRONTEND_ERROR';
        const hints = error instanceof SelfControlError ? error.hints : [];
        const composed =
          hints.length > 0 ? `[${code}] ${errorMessage}\nHints: ${hints.join(' | ')}` : `[${code}] ${errorMessage}`;
        logger.error('Self-control action failed', {
          requestId,
          code,
          error: errorMessage,
          hints,
        });
        await SelfControlAPI.submitSelfControlResponse({
          requestId,
          success: false,
          error: composed,
        });
      }
    })();
  });
}

export function stopSelfControlEventListener(): void {
  if (unlistenFn) {
    unlistenFn();
    unlistenFn = null;
    logger.info('Stopped SelfControl event listener');
  }
}
