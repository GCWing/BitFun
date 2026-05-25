import { notificationService } from '@/shared/notification-system';
import { flowChatStore } from '../store/FlowChatStore';

export type GoalVerificationOutcome = 'achieved' | 'continuing' | 'failed' | 'limit_reached';

export interface GoalVerificationEventPayload {
  sessionId: string;
  sourceTurnId?: string;
  outcome?: GoalVerificationOutcome;
}

export function handleGoalVerificationStarted(
  payload: GoalVerificationEventPayload,
  loadingMessage: string,
): void {
  if (!payload.sessionId) return;

  const verifyingId = `verify-${payload.sessionId}-${payload.sourceTurnId ?? Date.now()}`;
  flowChatStore.addLocalGoalVerifyingTurn({
    sessionId: payload.sessionId,
    message: loadingMessage,
    verifyingId,
  });
}

export function handleGoalVerificationFinished(
  payload: GoalVerificationEventPayload,
  messages: {
    achievedTitle: string;
    achievedMessage?: string;
    failedMessage: string;
  },
): void {
  if (!payload.sessionId) return;

  flowChatStore.removeLocalGoalVerifyingTurn(payload.sessionId);

  if (payload.outcome === 'achieved' || payload.outcome === 'limit_reached') {
    flowChatStore.setGoalModeActive(payload.sessionId, false);
  }

  switch (payload.outcome) {
    case 'achieved':
      if (messages.achievedMessage) {
        notificationService.success(messages.achievedMessage, {
          title: messages.achievedTitle,
          duration: 6000,
        });
      } else {
        notificationService.success(messages.achievedTitle, {
          duration: 6000,
        });
      }
      break;
    case 'failed':
      notificationService.error(messages.failedMessage, {
        duration: 5000,
      });
      break;
    default:
      break;
  }
}
