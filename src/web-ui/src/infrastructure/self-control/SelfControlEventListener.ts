/**
 * SelfControlEventListener — listens to backend self-control requests and
 * delegates execution to SelfControlService.
 */

import { api } from '@/infrastructure/api/service-api/ApiClient';
import { createLogger } from '@/shared/utils/logger';
import { SelfControlAPI } from '@/infrastructure/api/service-api/SelfControlAPI';
import {
  selfControlService,
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
        logger.error('Self-control action failed', { requestId, error: errorMessage });
        await SelfControlAPI.submitSelfControlResponse({
          requestId,
          success: false,
          error: errorMessage,
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
