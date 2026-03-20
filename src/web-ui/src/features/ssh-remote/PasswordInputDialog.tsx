/**
 * Password Input Dialog Component
 * Custom modal for secure password/key passphrase input
 */

import React, { useState, useRef, useEffect } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Modal } from '@/component-library';
import { Button } from '@/component-library';
import { Input } from '@/component-library';
import { Lock, Key, Loader2 } from 'lucide-react';
import './PasswordInputDialog.scss';

interface PasswordInputDialogProps {
  open: boolean;
  title: string;
  description?: string;
  placeholder?: string;
  isKeyPath?: boolean;
  isConnecting?: boolean;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}

export const PasswordInputDialog: React.FC<PasswordInputDialogProps> = ({
  open,
  title,
  description,
  placeholder = '',
  isKeyPath = false,
  isConnecting = false,
  onSubmit,
  onCancel,
}) => {
  const { t } = useI18n('common');
  const [value, setValue] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  // Focus input when dialog opens
  useEffect(() => {
    if (open) {
      setValue('');
      setTimeout(() => {
        inputRef.current?.focus();
      }, 100);
    }
  }, [open]);

  const handleSubmit = () => {
    if (value.trim()) {
      onSubmit(value.trim());
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !isConnecting) {
      e.preventDefault();
      handleSubmit();
    }
    if (e.key === 'Escape') {
      onCancel();
    }
  };

  return (
    <Modal
      isOpen={open}
      onClose={onCancel}
      title={title}
      size="small"
      showCloseButton
    >
      <div className="password-input-dialog">
        {description && (
          <div className="password-input-dialog__description">
            <div className="password-input-dialog__description-icon">
              {isKeyPath ? <Key size={16} /> : <Lock size={16} />}
            </div>
            <span className="password-input-dialog__description-text">{description}</span>
          </div>
        )}
        <div className="password-input-dialog__input">
          <Input
            ref={inputRef}
            type={isKeyPath ? 'text' : 'password'}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={placeholder}
            prefix={isKeyPath ? <Key size={16} /> : <Lock size={16} />}
            size="medium"
            disabled={isConnecting}
          />
        </div>
        <div className="password-input-dialog__actions">
          <Button variant="secondary" onClick={onCancel} disabled={isConnecting}>
            {t('actions.cancel')}
          </Button>
          <Button
            variant="primary"
            onClick={handleSubmit}
            disabled={!value.trim() || isConnecting}
          >
            {isConnecting ? (
              <>
                <Loader2 size={14} className="password-input-dialog__spinner" />
                {t('ssh.remote.connecting')}
              </>
            ) : (
              t('actions.confirm')
            )}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default PasswordInputDialog;
