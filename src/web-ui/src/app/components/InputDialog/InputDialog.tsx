import { useEffect, useRef, useState, type ChangeEvent, type KeyboardEvent } from 'react';
import {
  Button,
  Dialog,
  DialogBody,
  DialogClose,
  DialogFooter,
  DialogHeader,
  DialogHeading,
  DialogTitle,
  Input,
} from '@bitfun/ui';
import { useI18n } from '@/infrastructure/i18n';
import './InputDialog.scss';

export interface InputDialogProps {
  cancelText?: string;
  confirmText?: string;
  defaultValue?: string;
  description?: string;
  inputType?: 'text' | 'password' | 'email' | 'number';
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (value: string) => void;
  placeholder?: string;
  required?: boolean;
  title: string;
  validator?: (value: string) => string | null;
}

export function InputDialog({
  cancelText,
  confirmText,
  defaultValue = '',
  description,
  inputType = 'text',
  isOpen,
  onClose,
  onConfirm,
  placeholder,
  required = true,
  title,
  validator,
}: InputDialogProps) {
  const { t } = useI18n('components');
  const inputRef = useRef<HTMLInputElement>(null);
  const [value, setValue] = useState(defaultValue);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setValue(defaultValue);
    setError(null);
  }, [defaultValue, isOpen]);

  const validateInput = (nextValue: string): boolean => {
    if (required && !nextValue.trim()) {
      setError(t('inputDialog.emptyError'));
      return false;
    }
    const validationError = validator?.(nextValue);
    if (validationError) {
      setError(validationError);
      return false;
    }
    setError(null);
    return true;
  };

  const handleConfirm = () => {
    if (!validateInput(value)) return;
    onConfirm(value.trim());
    onClose();
  };

  const handleChange = (event: ChangeEvent<HTMLInputElement>) => {
    setValue(event.target.value);
    if (error) setError(null);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    handleConfirm();
  };

  const resolvedDescriptionId = error ? 'input-dialog-error' : undefined;

  return (
    <Dialog
      initialFocusRef={inputRef}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) onClose();
      }}
      open={isOpen}
      size="sm"
    >
      <DialogHeader>
        <DialogHeading>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeading>
        <DialogClose />
      </DialogHeader>
      <DialogBody data-bf-component="input-dialog" data-bf-part="body">
        {description ? (
          <p
            className="input-dialog__description"
            data-bf-component="input-dialog"
            data-bf-part="description"
          >
            {description}
          </p>
        ) : null}
        <Input
          aria-describedby={resolvedDescriptionId}
          autoFocus
          className="input-dialog__input"
          invalid={Boolean(error)}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          placeholder={placeholder ?? t('dialog.prompt.placeholder')}
          ref={inputRef}
          type={inputType}
          value={value}
        />
        {error ? (
          <span
            className="input-dialog__error"
            data-bf-component="input-dialog"
            data-bf-part="error"
            id="input-dialog-error"
            role="alert"
          >
            {error}
          </span>
        ) : null}
      </DialogBody>
      <DialogFooter data-bf-component="input-dialog" data-bf-part="actions">
        <Button onClick={onClose} size="sm" variant="outline">
          {cancelText ?? t('dialog.confirm.cancel')}
        </Button>
        <Button onClick={handleConfirm} size="sm" variant="fill">
          {confirmText ?? t('dialog.confirm.ok')}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
