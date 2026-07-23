/**
 * InputDialog component
 * Replaces the browser's native prompt()
 */

import React, { useState, useEffect, useRef } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Modal } from '../Modal/Modal';
import { Input } from '../Input/Input';
import { Button } from '../Button/Button';
import './InputDialog.scss';

export interface InputDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (value: string) => void;
  title: string;
  description?: string;
  placeholder?: string;
  defaultValue?: string;
  confirmText?: string;
  cancelText?: string;
  validator?: (value: string) => string | null;
  required?: boolean;
  inputType?: 'text' | 'password' | 'email' | 'number';
}

export const InputDialog: React.FC<InputDialogProps> = ({
  isOpen,
  onClose,
  onConfirm,
  title,
  description,
  placeholder,
  defaultValue = '',
  confirmText,
  cancelText,
  validator,
  required = true,
  inputType = 'text',
}) => {
  const { t } = useI18n('components');
  
  // Resolve i18n default values
  const resolvedPlaceholder = placeholder ?? t('dialog.prompt.placeholder');
  const resolvedConfirmText = confirmText ?? t('dialog.confirm.ok');
  const resolvedCancelText = cancelText ?? t('dialog.confirm.cancel');
  
  const [value, setValue] = useState(defaultValue);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setValue(defaultValue);
      setError(null);
      setTimeout(() => {
        inputRef.current?.focus();
        inputRef.current?.select();
      }, 100);
    }
  }, [isOpen, defaultValue]);

  const validateInput = (val: string): boolean => {
    if (required && !val.trim()) {
      setError(t('inputDialog.emptyError'));
      return false;
    }

    if (validator) {
      const errorMsg = validator(val);
      if (errorMsg) {
        setError(errorMsg);
        return false;
      }
    }

    setError(null);
    return true;
  };

  const handleConfirm = () => {
    if (validateInput(value)) {
      onConfirm(value.trim());
      onClose();
    }
  };

  const handleCancel = () => {
    onClose();
  };

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = e.target.value;
    setValue(newValue);
    if (error) {
      setError(null);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleConfirm();
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={handleCancel}
      title={title}
      size="small"
      showCloseButton={true}
    >
      <div className="input-dialog">
        <div className="input-dialog__body">
          {description && (
            <p className="input-dialog__description">{description}</p>
          )}
          <Input
            ref={inputRef}
            type={inputType}
            value={value}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            placeholder={resolvedPlaceholder}
            error={!!error}
            errorMessage={error || undefined}
            inputSize="medium"
            autoFocus
          />
        </div>

        <div className="input-dialog__actions">
          <Button
            variant="secondary"
            size="small"
            onClick={handleCancel}
          >
            {resolvedCancelText}
          </Button>
          <Button
            variant="primary"
            size="small"
            onClick={handleConfirm}
          >
            {resolvedConfirmText}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

