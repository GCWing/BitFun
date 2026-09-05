import React, { useState } from 'react';
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

  if (!isToolAwaitingApproval(tool) || !onApprove || !onReject) return null;

  const runAction = async (action: 'approve' | 'reject') => {
    if (pendingAction) return;
    setPendingAction(action);
    try {
      await (action === 'approve' ? onApprove(tool.id) : onReject(tool.id));
    } catch {
      setPendingAction(null);
    }
  };

  return (
    <div className="chat-tool-approval" role="group" aria-label={t('chat.approvalRequired')}>
      <div className="chat-tool-approval__actions">
        <MobileButton
          appearance="primary"
          block
          className="chat-tool-approval__button chat-tool-approval__button--approve"
          disabled={pendingAction !== null}
          onClick={() => void runAction('approve')}
          size="sm"
        >
          {pendingAction === 'approve' ? t('chat.approving') : t('chat.approve')}
        </MobileButton>
        <MobileButton
          appearance="secondary"
          block
          className="chat-tool-approval__button chat-tool-approval__button--reject"
          disabled={pendingAction !== null}
          onClick={() => void runAction('reject')}
          size="sm"
        >
          {pendingAction === 'reject' ? t('chat.rejecting') : t('chat.reject')}
        </MobileButton>
      </div>
    </div>
  );
}
