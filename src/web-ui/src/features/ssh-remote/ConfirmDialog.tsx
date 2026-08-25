/**
 * Confirm Dialog Component
 * Custom modal for confirmation prompts
 */

import { Button } from '@bitfun/ui';
import React from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { AlertTriangle } from 'lucide-react';
import { Modal } from '@/component-library';
import './ConfirmDialog.scss';

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  onConfirm: () => void;
  onCancel: () => void;
  destructive?: boolean;
}

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  open,
  title,
  message,
  confirmText,
  cancelText,
  onConfirm,
  onCancel,
  destructive = false,
}) => {
  const { t } = useI18n('common');

  const handleConfirm = () => {
    onConfirm();
    onCancel();
  };

  return (
    <Modal
      isOpen={open}
      onClose={onCancel}
      title={title}
      size="small"
      showCloseButton
    >
      <div className="ssh-remote-confirm-dialog" data-bf-component="ssh-remote" data-bf-part="confirmDialog">
        {destructive && (
          <div className="ssh-remote-confirm-dialog__warning" data-bf-component="ssh-remote" data-bf-part="confirmWarning">
            <AlertTriangle size={20} />
            <span>{title}</span>
          </div>
        )}
        <p className="ssh-remote-confirm-dialog__message" data-bf-component="ssh-remote" data-bf-part="confirmMessage">{message}</p>
        <div className="ssh-remote-confirm-dialog__actions" data-bf-component="ssh-remote" data-bf-part="confirmActions">
          <Button variant="outline" size="sm" onClick={onCancel}>
            {cancelText || t('actions.cancel')}
          </Button>
          <Button
            variant="fill"
            tone={destructive ? 'danger' : 'neutral'}
            size="sm"
            onClick={handleConfirm}
          >
            {confirmText || t('actions.confirm')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default ConfirmDialog;
