/**
 * ConfirmDialog component
 * Supports both controlled usage and imperative calls
 */

import React, { useEffect, useRef } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Modal } from '../Modal/Modal';
import { Button } from '../Button/Button';
import { AlertTriangle, Info, AlertCircle, CheckCircle } from 'lucide-react';
import './ConfirmDialog.scss';

export type ConfirmDialogType = 'info' | 'warning' | 'error' | 'success';

export interface ConfirmDialogProps {
  /** Whether the dialog is visible */
  isOpen: boolean;
  /** Close callback */
  onClose: () => void;
  /** Confirm callback */
  onConfirm: () => void;
  /** Cancel callback */
  onCancel?: () => void;
  /** Title */
  title: string;
  /** Message content */
  message: React.ReactNode;
  /** Dialog type */
  type?: ConfirmDialogType;
  /** Confirm button text */
  confirmText?: string;
  /** Cancel button text */
  cancelText?: string;
  /** Whether the confirm button uses danger styling */
  confirmDanger?: boolean;
  /** Whether to show the cancel button */
  showCancel?: boolean;
  /** Preview content (e.g. multi-line text) */
  preview?: string;
  /** Max preview height */
  previewMaxHeight?: number;
}

const iconMap: Record<ConfirmDialogType, React.ReactNode> = {
  info: <Info size={24} />,
  warning: <AlertTriangle size={24} />,
  error: <AlertCircle size={24} />,
  success: <CheckCircle size={24} />,
};

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  isOpen,
  onClose,
  onConfirm,
  onCancel,
  title,
  message,
  type = 'warning',
  confirmText,
  cancelText,
  confirmDanger = false,
  showCancel = true,
  preview,
  previewMaxHeight = 200,
}) => {
  const { t } = useI18n('components');
  const hasMessage = message !== null && message !== undefined && message !== '';
  
  // Resolve i18n default values
  const resolvedConfirmText = confirmText ?? t('dialog.confirm.ok');
  const resolvedCancelText = cancelText ?? t('dialog.confirm.cancel');
  
  const confirmButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (isOpen) {
      setTimeout(() => {
        confirmButtonRef.current?.focus();
      }, 100);
    }
  }, [isOpen]);

  const handleConfirm = () => {
    onConfirm();
    onClose();
  };

  const handleCancel = () => {
    onCancel?.();
    onClose();
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={handleCancel}
      size="small"
      showCloseButton={false}
    >
      <div className={`confirm-dialog confirm-dialog--${type}`}>
        <div className="confirm-dialog__icon">
          {iconMap[type]}
        </div>
        
        <div className="confirm-dialog__content">
          <h3 className={`confirm-dialog__title${hasMessage ? '' : ' confirm-dialog__title--compact'}`}>{title}</h3>
          {hasMessage ? (
            <div className="confirm-dialog__message">{message}</div>
          ) : null}
          
          {preview && (
            <div 
              className="confirm-dialog__preview"
              style={{ maxHeight: previewMaxHeight }}
            >
              <pre>{preview}</pre>
            </div>
          )}
        </div>

        <div className="confirm-dialog__actions">
          {showCancel && (
            <Button
              variant="secondary"
              size="medium"
              onClick={handleCancel}
            >
              {resolvedCancelText}
            </Button>
          )}
          <Button
            ref={confirmButtonRef}
            variant={confirmDanger ? 'danger' : 'primary'}
            size="medium"
            onClick={handleConfirm}
          >
            {resolvedConfirmText}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default ConfirmDialog;
