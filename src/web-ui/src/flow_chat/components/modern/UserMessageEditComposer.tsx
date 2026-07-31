import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Check, Loader2, X } from 'lucide-react';
import { Textarea } from '@/component-library';
import type { ContextItem } from '@/shared/types/context';
import { FileMentionPicker } from '../FileMentionPicker';
import {
  RichTextInput,
  type MentionState,
  type RichTextInputElement,
} from '../RichTextInput';
import {
  composerPresentationContexts,
  type ComposerPresentation,
} from '../../utils/composerPresentation';

interface UserMessageEditComposerProps {
  value: string;
  isSubmitting?: boolean;
  submitLabel: string;
  cancelLabel: string;
  placeholder?: string;
  onChange: (value: string) => void;
  onSubmit: (presentation?: ComposerPresentation) => void | Promise<void>;
  onCancel: () => void;
  presentation?: ComposerPresentation | null;
  workspacePath?: string;
  workspaceId?: string;
  excludeSessionId?: string;
}

type RichUserMessageEditComposerProps = Omit<UserMessageEditComposerProps, 'presentation'> & {
  presentation: ComposerPresentation;
};

const RichUserMessageEditComposer: React.FC<RichUserMessageEditComposerProps> = ({
  value,
  isSubmitting = false,
  submitLabel,
  cancelLabel,
  placeholder,
  onChange,
  onSubmit,
  onCancel,
  presentation,
  workspacePath,
  workspaceId,
  excludeSessionId,
}) => {
  const editorRef = useRef<RichTextInputElement>(null);
  const [contexts, setContexts] = useState<ContextItem[]>(() => (
    composerPresentationContexts(presentation)
  ));
  const [mentionState, setMentionState] = useState<MentionState>({
    isActive: false,
    query: '',
    startOffset: 0,
  });
  const canSubmit = value.trim().length > 0 && !isSubmitting;

  useEffect(() => {
    setContexts(composerPresentationContexts(presentation));
    const frame = requestAnimationFrame(() => {
      editorRef.current?.restoreComposerPresentation?.(presentation);
      editorRef.current?.focus();
    });
    return () => cancelAnimationFrame(frame);
  }, [presentation]);

  const capturePresentation = useCallback(() => (
    editorRef.current?.getComposerPresentation?.() ?? presentation
  ), [presentation]);

  const handleSubmit = useCallback(() => {
    if (!canSubmit) return;
    void onSubmit(capturePresentation());
  }, [canSubmit, capturePresentation, onSubmit]);

  const handleKeyDown = useCallback((event: React.KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      if (mentionState.isActive) {
        editorRef.current?.closeMention?.();
      } else {
        onCancel();
      }
      return;
    }

    if (
      event.key === 'Enter' &&
      !mentionState.isActive &&
      !event.shiftKey &&
      !event.altKey &&
      !event.metaKey &&
      !event.ctrlKey
    ) {
      event.preventDefault();
      handleSubmit();
    }
  }, [handleSubmit, mentionState.isActive, onCancel]);

  const handleRemoveContext = useCallback((id: string) => {
    setContexts(current => current.filter(context => context.id !== id));
  }, []);

  const handleSelectContext = useCallback((context: ContextItem) => {
    setContexts(current => (
      current.some(item => item.id === context.id) ? current : [...current, context]
    ));
    requestAnimationFrame(() => {
      editorRef.current?.insertTagReplacingMention?.(context);
      editorRef.current?.focus();
    });
  }, []);

  return (
    <div className="user-message-edit-composer">
      <div className="user-message-edit-composer__rich-input">
        <RichTextInput
          ref={editorRef}
          value={value}
          onChange={(nextValue) => onChange(nextValue)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          disabled={isSubmitting}
          contexts={contexts}
          onRemoveContext={handleRemoveContext}
          onMentionStateChange={setMentionState}
        />
        <FileMentionPicker
          isOpen={mentionState.isActive}
          searchQuery={mentionState.query}
          workspacePath={workspacePath}
          workspaceId={workspaceId}
          excludeSessionId={excludeSessionId}
          onSelect={handleSelectContext}
          onClose={() => editorRef.current?.closeMention?.()}
        />
      </div>
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

export const UserMessageEditComposer: React.FC<UserMessageEditComposerProps> = ({
  value,
  isSubmitting = false,
  submitLabel,
  cancelLabel,
  placeholder,
  onChange,
  onSubmit,
  onCancel,
  presentation,
  workspacePath,
  workspaceId,
  excludeSessionId,
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

  if (presentation) {
    return (
      <RichUserMessageEditComposer
        value={value}
        isSubmitting={isSubmitting}
        submitLabel={submitLabel}
        cancelLabel={cancelLabel}
        placeholder={placeholder}
        onChange={onChange}
        onSubmit={onSubmit}
        onCancel={onCancel}
        presentation={presentation}
        workspacePath={workspacePath}
        workspaceId={workspaceId}
        excludeSessionId={excludeSessionId}
      />
    );
  }

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
