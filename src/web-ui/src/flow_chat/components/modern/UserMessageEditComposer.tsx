import React, { useCallback, useEffect, useRef } from 'react';
import { Check, Loader2, X } from 'lucide-react';
import { Textarea } from '@/component-library';

interface UserMessageEditComposerProps {
  value: string;
  isSubmitting?: boolean;
  submitLabel: string;
  cancelLabel: string;
  placeholder?: string;
  onChange: (value: string) => void;
  onSubmit: () => void | Promise<void>;
  onCancel: () => void;
}

export const UserMessageEditComposer: React.FC<UserMessageEditComposerProps> = ({
  value,
  isSubmitting = false,
  submitLabel,
  cancelLabel,
  placeholder,
  onChange,
  onSubmit,
  onCancel,
}) => {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const trimmedValue = value.trim();
  const canSubmit = trimmedValue.length > 0 && !isSubmitting;

  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;

    textarea.focus();
    textarea.setSelectionRange(textarea.value.length, textarea.value.length);
  }, []);

  const handleSubmit = useCallback(() => {
    if (!canSubmit) return;
    void onSubmit();
  }, [canSubmit, onSubmit]);

  const handleKeyDown = useCallback((event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onCancel();
      return;
    }

    if (event.key === 'Enter' && !event.shiftKey && !event.altKey && !event.metaKey && !event.ctrlKey) {
      event.preventDefault();
      handleSubmit();
    }
  }, [handleSubmit, onCancel]);

  return (
    <div className="user-message-edit-composer">
      <Textarea
        ref={textareaRef}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        autoResize
        disabled={isSubmitting}
        className="user-message-edit-composer__textarea"
      />
      <div className="user-message-edit-composer__actions">
        <button
          type="button"
          onClick={onCancel}
          disabled={isSubmitting}
          className="user-message-edit-composer__icon-button"
          title={cancelLabel}
          aria-label={cancelLabel}
        >
          <X size={14} />
        </button>
        <button
          type="button"
          onClick={handleSubmit}
          disabled={!canSubmit}
          className="user-message-edit-composer__icon-button user-message-edit-composer__icon-button--confirm"
          title={submitLabel}
          aria-label={submitLabel}
        >
          {isSubmitting ? <Loader2 size={14} className="user-message-edit-composer__spinner" /> : <Check size={14} />}
        </button>
      </div>
    </div>
  );
};

UserMessageEditComposer.displayName = 'UserMessageEditComposer';