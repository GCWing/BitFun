import { useEffect, useRef } from 'react';
import { agentAPI } from '@/infrastructure/api';
import {
  globalEventBus,
  PERMISSION_REQUEST_NOTIFICATION_EVENT,
  type PermissionRequestNotificationEvent,
} from '@/infrastructure/event-bus';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { useI18n } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import {
  buildPermissionRequestNotificationCopy,
  PermissionRequestNotificationBatcher,
  shouldSendPermissionRequestNotification,
} from './permissionRequestNotifyPolicy';

const log = createLogger('usePermissionRequestNotify');

const PERMISSION_REQUEST_NOTIFY_CONFIG_KEY = 'app.notifications.permission_request_notify';

/**
 * Sends one generic desktop notification for user-actionable permission
 * requests received while OpenBitFun is in the background. Core runtime and ACP
 * requests share the same policy, de-duplication, and round-level batching.
 */
export const usePermissionRequestNotify = () => {
  const { t } = useI18n('common');
  const windowFocusedRef = useRef(true);

  useEffect(() => {
    let disposed = false;
    const handleFocus = () => { windowFocusedRef.current = true; };
    const handleBlur = () => { windowFocusedRef.current = false; };
    windowFocusedRef.current = document.hasFocus();
    const isBackground = () => document.hidden || !windowFocusedRef.current;

    const sendBatchNotification = async (requestCount: number) => {
      let enabled = true;
      try {
        enabled = await configManager.getConfig<boolean>(PERMISSION_REQUEST_NOTIFY_CONFIG_KEY);
      } catch (error) {
        log.warn('Failed to read permission_request_notify config', error);
      }

      if (!shouldSendPermissionRequestNotification({
        isBackground: isBackground(),
        notificationsEnabled: enabled,
      })) {
        return;
      }

      const notificationCopy = buildPermissionRequestNotificationCopy({ requestCount, t });
      await systemAPI.sendSystemNotification(notificationCopy.title, notificationCopy.body);
    };

    const batcher = new PermissionRequestNotificationBatcher(({ requestCount }) => {
      void sendBatchNotification(requestCount);
    });

    const enqueueWhenBackground = (request: PermissionRequestNotificationEvent) => {
      if (isBackground()) {
        batcher.enqueue(request);
      }
    };

    window.addEventListener('focus', handleFocus);
    window.addEventListener('blur', handleBlur);

    const unlistenPermissionRequests = agentAPI.onPermissionRequestEvent((event) => {
      if (event.event !== 'asked') {
        return;
      }
      enqueueWhenBackground({
        requestId: event.request.requestId,
        sessionId: event.request.sessionId,
        roundId: event.request.roundId,
      });
    });
    const unlistenAcpPermissionRequests = globalEventBus.on<PermissionRequestNotificationEvent>(
      PERMISSION_REQUEST_NOTIFICATION_EVENT,
      enqueueWhenBackground,
    );

    void (async () => {
      try {
        await agentAPI.subscribePermissionRequests();
        const pendingRequests = await agentAPI.listPendingPermissionRequests();
        if (disposed) {
          return;
        }
        pendingRequests.forEach((request) => {
          enqueueWhenBackground({
            requestId: request.requestId,
            sessionId: request.sessionId,
            roundId: request.roundId,
          });
        });
      } catch (error) {
        log.warn('Failed to subscribe to permission request events', error);
      }
    })();

    return () => {
      disposed = true;
      window.removeEventListener('focus', handleFocus);
      window.removeEventListener('blur', handleBlur);
      unlistenPermissionRequests();
      unlistenAcpPermissionRequests();
      batcher.dispose();
    };
  }, [t]);
};
