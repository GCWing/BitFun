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
  t,
}: DialogCompletionNotificationCopyInput): { title: string; body: string } {
  const trimmedTitle = sessionTitle?.trim();

  return {
    title: t('notify.dialogCompletedTitle'),
    body: trimmedTitle
      ? t('notify.dialogCompletedWithSession', { sessionTitle: trimmedTitle })
      : t('notify.dialogCompletedFallback'),
  };
}
