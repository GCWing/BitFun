import React from 'react';
import { MobileActionSheet, type MobileActionSheetItem } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';
import type { ChatMessage } from '../services/RemoteSessionManager';

interface ChatMessageActionsProps {
  deleting: boolean;
  message: ChatMessage | null;
  onClose: () => void;
  onCopy: () => void;
  onDelete: () => void;
  onResend: () => void;
}

const CopyIcon = () => <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" /></svg>;
const ResendIcon = () => <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><polyline points="23 4 23 10 17 10" /><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" /></svg>;
const DeleteIcon = () => <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>;

export default function ChatMessageActions({ deleting, message, onClose, onCopy, onDelete, onResend }: ChatMessageActionsProps) {
  const { t } = useI18n();
  const actions: MobileActionSheetItem[] = message ? [
    { id: 'copy', label: t('chat.copyMessage'), leading: <CopyIcon /> },
    ...(message.role === 'user' ? [{ id: 'resend', label: t('chat.resendMessage'), leading: <ResendIcon /> }] : []),
    { disabled: deleting, id: 'delete', label: deleting ? '...' : t('chat.deleteMessage'), leading: <DeleteIcon />, tone: 'danger' },
  ] : [];

  return (
    <MobileActionSheet
      actions={actions}
      cancelLabel={t('common.cancel')}
      closeOnAction={false}
      onAction={(id) => {
        if (id === 'copy') onCopy();
        if (id === 'resend') onResend();
        if (id === 'delete') onDelete();
      }}
      onOpenChange={onClose}
      open={message !== null}
      title={t('common.more')}
    />
  );
}
