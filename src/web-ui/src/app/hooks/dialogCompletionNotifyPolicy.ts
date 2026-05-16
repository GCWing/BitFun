import type { AgenticEvent } from '@/infrastructure/api/service-api/AgentAPI';
import type { Session } from '@/flow_chat/types/flow-chat';

interface DialogCompletionNotificationInput {
  event: AgenticEvent;
  session?: Pick<Session, 'sessionKind' | 'parentSessionId'> | null;
  isBackground: boolean;
  notificationsEnabled?: boolean;
}

interface DialogCompletionNotificationCopyInput {
  sessionTitle?: string | null;
  success?: boolean | null;
  finishReason?: string | null;
  t: (key: string, options?: Record<string, unknown>) => string;
}

export function shouldSendDialogCompletionNotification({
  event,
  session,
  isBackground,
  notificationsEnabled,
}: DialogCompletionNotificationInput): boolean {
  if (!isBackground || notificationsEnabled === false) {
    return false;
  }

  if (event.subagentParentInfo) {
    return false;
  }

  if (!session) {
    return false;
  }

  const sessionKind = session?.sessionKind ?? 'normal';
  if (sessionKind === 'btw' || sessionKind === 'review') {
    return false;
  }

  return true;
}

export function buildDialogCompletionNotificationCopy({
  sessionTitle,
  success,
  finishReason,
  t,
}: DialogCompletionNotificationCopyInput): { title: string; body: string } {
  const trimmedTitle = sessionTitle?.trim();
  const failed = success === false;
  const options = {
    sessionTitle: trimmedTitle,
    finishReason,
  };

  return {
    title: failed
      ? t('notify.dialogFailedTitle')
      : t('notify.dialogCompletedTitle'),
    body: trimmedTitle
      ? t(failed ? 'notify.dialogFailedWithSession' : 'notify.dialogCompletedWithSession', options)
      : t(failed ? 'notify.dialogFailedFallback' : 'notify.dialogCompletedFallback', options),
  };
}
