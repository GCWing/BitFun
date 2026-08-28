/**
 * InputDialog component
 * Replaces the browser's native prompt()
 */

import React, { useState, useEffect, useRef } from 'react';
import { Button, Input, Modal } from '@bitfun/ui';
import { useI18n } from '@/infrastructure/i18n';
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
      overlayClassName="input-dialog-overlay"
    >
      <div className="input-dialog" data-bf-component="input-dialog" data-bf-part="root">
        <div className="input-dialog__body" data-bf-component="input-dialog" data-bf-part="body">
          {description && (
            <p className="input-dialog__description" data-bf-component="input-dialog" data-bf-part="description">{description}</p>
          )}
          <Input
            ref={inputRef}
            type={inputType}
            value={value}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            placeholder={resolvedPlaceholder}
            invalid={!!error}
            aria-describedby={error ? 'input-dialog-error' : undefined}
            autoFocus
          />
          {error && (
            <span
              className="input-dialog__error"
              data-bf-component="input-dialog"
              data-bf-part="error"
              id="input-dialog-error"
              role="alert"
            >
              {error}
            </span>
          )}
        </div>

        <div className="input-dialog__actions" data-bf-component="input-dialog" data-bf-part="actions">
          <Button
            variant="outline"
            size="sm"
            onClick={handleCancel}
          >
            {resolvedCancelText}
          </Button>
          <Button
            variant="fill"
            size="sm"
            onClick={handleConfirm}
          >
            {resolvedConfirmText}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

