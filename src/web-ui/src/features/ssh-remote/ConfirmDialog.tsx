/**
 * Confirm Dialog Component
 * Custom modal for confirmation prompts
 */

import React from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Modal } from '@/component-library';
import { Button } from '@/component-library';
import { AlertTriangle } from 'lucide-react';
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
      <div className="confirm-dialog">
        {destructive && (
          <div className="confirm-dialog__warning">
            <AlertTriangle size={20} />
            <span>{title}</span>
          </div>
        )}
        <p className="confirm-dialog__message">{message}</p>
        <div className="confirm-dialog__actions">
          <Button variant="secondary" onClick={onCancel}>
            {cancelText || t('actions.cancel')}
          </Button>
          <Button
            variant={destructive ? 'danger' : 'primary'}
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
