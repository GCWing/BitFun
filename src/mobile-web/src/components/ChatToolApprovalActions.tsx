import React, { useEffect, useState } from 'react';
import { MobileButton } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';
import type { RemoteToolStatus } from '../services/RemoteSessionManager';

interface ChatToolApprovalActionsProps {
  tool: RemoteToolStatus;
  onApprove?: (toolId: string) => Promise<void>;
  onReject?: (toolId: string) => Promise<void>;
}

export function isToolAwaitingApproval(tool: RemoteToolStatus): boolean {
  const status = tool.status.toLowerCase();
  return (status === 'pending_confirmation' || status === 'needs_confirmation') && Boolean(tool.id);
}

export default function ChatToolApprovalActions({
  tool,
  onApprove,
  onReject,
}: ChatToolApprovalActionsProps) {
  const { t } = useI18n();
  const [pendingAction, setPendingAction] = useState<'approve' | 'reject' | null>(null);

  const [failed, setFailed] = useState(false);
  const [submitted, setSubmitted] = useState(false);

  useEffect(() => {
    setPendingAction(null);
    setFailed(false);
    setSubmitted(false);
  }, [tool.id, tool.status]);

  if (!isToolAwaitingApproval(tool) || !onApprove || !onReject) return null;

  const request = tool.tool_input ?? tool.input_preview;
  const requestText = typeof request === 'string' ? request : request == null ? '' : JSON.stringify(request, null, 2);

  const runAction = async (action: 'approve' | 'reject') => {
    if (pendingAction || submitted) return;
    setPendingAction(action);
    setFailed(false);
    try {
      await (action === 'approve' ? onApprove(tool.id) : onReject(tool.id));
      setSubmitted(true);
      setPendingAction(null);
    } catch {
      setFailed(true);
      setPendingAction(null);
    }
  };

  return (
    <div className="chat-tool-approval" role="group" aria-label={t('chat.approvalRequired')}>
      <p className="chat-tool-approval__label">{t('chat.approvalRequired')}</p>
      {requestText && <pre className="chat-tool-approval__request">{requestText.length > 240 ? `${requestText.slice(0, 240)}…` : requestText}</pre>}
      {failed && <p className="chat-tool-approval__error" role="alert">{t('chat.approvalFailed')}</p>}
      {submitted && <p role="status">{t('chat.approvalSubmitted')}</p>}
      <div className="chat-tool-approval__actions" aria-busy={pendingAction !== null}>
        <MobileButton
          appearance="primary"
          block
          className="chat-tool-approval__button chat-tool-approval__button--approve"
          disabled={pendingAction !== null || submitted}
          onClick={() => void runAction('approve')}
          size="sm"
        >
          {pendingAction === 'approve' ? t('chat.approving') : t('chat.approve')}
        </MobileButton>
        <MobileButton
          appearance="secondary"
          block
          className="chat-tool-approval__button chat-tool-approval__button--reject"
          disabled={pendingAction !== null || submitted}
          onClick={() => void runAction('reject')}
          size="sm"
        >
          {pendingAction === 'reject' ? t('chat.rejecting') : t('chat.reject')}
        </MobileButton>
      </div>
    </div>
  );
}
