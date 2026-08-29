import { common, wantAgent } from '@kit.AbilityKit';
import { backgroundTaskManager } from '@kit.BackgroundTasksKit';
import { notificationManager } from '@kit.NotificationKit';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { RemoteLogger } from './RemoteLogger';
import {
  TaskCompletionNotificationPort,
  TaskCompletionObservation
} from './TaskCompletionNotificationController';

const BACKGROUND_REASON: string = 'Waiting for an active BitFun task to finish';

/** HarmonyOS platform leaf for task-completion notifications. */
export class HarmonyTaskCompletionNotificationPort implements TaskCompletionNotificationPort {
  private readonly contextProvider: () => Context;
  private permissionPrepared: boolean = false;
  private suspendRequestId: number = 0;

  constructor(contextProvider: () => Context) {
    this.contextProvider = contextProvider;
  }

  async prepare(): Promise<void> {
    if (this.permissionPrepared) {
      return;
    }
    this.permissionPrepared = true;
    try {
      if (!notificationManager.isNotificationEnabledSync()) {
        await notificationManager.requestEnableNotification(this.uiAbilityContext());
      }
    } catch (err) {
      RemoteLogger.warn(`task completion notification permission unavailable: ${HarmonyTaskCompletionNotificationPort.errorText(err)}`);
    }
  }

  keepAlive(onExpired: () => void): boolean {
    this.releaseKeepAlive();
    try {
      const info = backgroundTaskManager.requestSuspendDelay(BACKGROUND_REASON, (): void => {
        this.suspendRequestId = 0;
        RemoteLogger.info('task completion background observation expired');
        onExpired();
      });
      this.suspendRequestId = info.requestId;
      RemoteLogger.info(`task completion background observation started delayMs=${info.actualDelayTime}`);
      return true;
    } catch (err) {
      RemoteLogger.warn(`task completion background observation unavailable: ${HarmonyTaskCompletionNotificationPort.errorText(err)}`);
      return false;
    }
  }

  releaseKeepAlive(): void {
    if (this.suspendRequestId === 0) {
      return;
    }
    const requestId = this.suspendRequestId;
    this.suspendRequestId = 0;
    try {
      backgroundTaskManager.cancelSuspendDelay(requestId);
    } catch (err) {
      RemoteLogger.warn(`task completion background observation release failed: ${HarmonyTaskCompletionNotificationPort.errorText(err)}`);
    }
  }

  async publish(observation: TaskCompletionObservation): Promise<void> {
    try {
      if (!notificationManager.isNotificationEnabledSync()) {
        RemoteLogger.info('task completion notification skipped: notifications disabled');
        return;
      }
      const context = this.uiAbilityContext();
      const notificationId = HarmonyTaskCompletionNotificationPort.notificationId(observation);
      const launchAgent = await wantAgent.getWantAgent({
        wants: [{
          bundleName: context.abilityInfo.bundleName,
          abilityName: context.abilityInfo.name
        }],
        actionType: wantAgent.OperationType.START_ABILITY,
        requestCode: notificationId,
        actionFlags: [wantAgent.WantAgentFlags.UPDATE_PRESENT_FLAG]
      });
      await notificationManager.publish({
        id: notificationId,
        appMessageId: `bitfun-task-completed:${observation.source}:${observation.sessionId}:${observation.turnId}`,
        notificationSlotType: notificationManager.SlotType.SERVICE_INFORMATION,
        tapDismissed: true,
        wantAgent: launchAgent,
        content: {
          notificationContentType: notificationManager.ContentType.NOTIFICATION_CONTENT_BASIC_TEXT,
          normal: {
            title: RemoteI18n.t('notification.taskCompletedTitle'),
            text: RemoteI18n.t('notification.taskCompletedBody')
          }
        }
      });
      RemoteLogger.info(`task completion notification published source=${observation.source}`);
    } catch (err) {
      RemoteLogger.warn(`task completion notification publish failed: ${HarmonyTaskCompletionNotificationPort.errorText(err)}`);
    }
  }

  private uiAbilityContext(): common.UIAbilityContext {
    return this.contextProvider() as common.UIAbilityContext;
  }

  private static notificationId(observation: TaskCompletionObservation): number {
    const value = `${observation.source}:${observation.sessionId}:${observation.turnId}`;
    let hash = 17;
    for (let index = 0; index < value.length; index += 1) {
      hash = ((hash * 31) + value.charCodeAt(index)) & 0x7FFFFFFF;
    }
    return hash === 0 ? 1 : hash;
  }

  private static errorText(err: Object): string {
    return err instanceof Error ? err.message : JSON.stringify(err);
  }
}
