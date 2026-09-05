import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Loader2 } from 'lucide-react';
import { Composer, ComposerToolbar, IconButton, Icon } from '@openbitfun/ui';
import { useImeOwnedKeyGuard } from '@/flow_chat/hooks/useImeOwnedKeyGuard';
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
  remoteConnectionId?: string;
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
  remoteConnectionId,
  excludeSessionId,
}) => {
  const editorRef = useRef<RichTextInputElement>(null);
  const mentionAnchorRef = useRef<HTMLDivElement>(null);
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
    <Composer
      className="user-message-edit-composer"
      data-openbitfun-product-component="user-message-edit-composer"
      data-openbitfun-product-part="root"
      data-openbitfun-mode="rich"
      data-openbitfun-state={isSubmitting ? 'submitting' : undefined}
      disabled={isSubmitting}
      toolbar={(
        <ComposerToolbar
          className="user-message-edit-composer__actions"
          data-openbitfun-product-component="user-message-edit-composer"
          data-openbitfun-product-part="actions"
          trailing={(
            <>
              <IconButton
                aria-label={cancelLabel}
                data-openbitfun-action="cancel"
                data-openbitfun-product-component="user-message-edit-composer"
                data-openbitfun-product-part="action"
                icon={<Icon name="xmark" size="sm" />}
                onClick={onCancel}
                size="xs"
                title={cancelLabel}
                variant="quiet"
              />
              <IconButton
                aria-busy={isSubmitting || undefined}
                aria-label={submitLabel}
                data-openbitfun-action="submit"
                data-openbitfun-product-component="user-message-edit-composer"
                data-openbitfun-product-part="action"
                disabled={!canSubmit}
                icon={isSubmitting ? (
                  <Loader2
                    className="user-message-edit-composer__spinner"
                    data-openbitfun-product-component="user-message-edit-composer"
                    data-openbitfun-product-part="spinner"
                    size={14}
                  />
                ) : <Icon name="check-line" size="sm" />}
                onClick={handleSubmit}
                size="xs"
                title={submitLabel}
                variant="primary"
              />
            </>
          )}
        />
      )}
    >
      <div
        ref={mentionAnchorRef}
        className="user-message-edit-composer__rich-input"
        data-openbitfun-product-component="user-message-edit-composer"
        data-openbitfun-product-part="input"
      >
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
          remoteConnectionId={remoteConnectionId}
          excludeSessionId={excludeSessionId}
          anchorRef={mentionAnchorRef}
          onSelect={handleSelectContext}
          onClose={() => editorRef.current?.closeMention?.()}
        />
      </div>
    </Composer>
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
  remoteConnectionId,
  excludeSessionId,
}) => {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { isImeOwnedKey, handleCompositionStart, handleCompositionEnd } = useImeOwnedKeyGuard();
  const trimmedValue = value.trim();
  const canSubmit = trimmedValue.length > 0 && !isSubmitting;

  const resizeTextarea = useCallback((textarea: HTMLTextAreaElement | null) => {
    if (!textarea) return;
    textarea.style.height = 'auto';
    textarea.style.height = `${textarea.scrollHeight}px`;
  }, []);

  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;

    textarea.focus();
    textarea.setSelectionRange(textarea.value.length, textarea.value.length);
  }, []);

  useLayoutEffect(() => {
    resizeTextarea(textareaRef.current);
  }, [resizeTextarea, value]);

  const handleSubmit = useCallback(() => {
    if (!canSubmit) return;
    void onSubmit();
  }, [canSubmit, onSubmit]);

  const handleKeyDown = useCallback((event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if ((event.key === 'Enter' || event.key === 'Escape') && isImeOwnedKey(event)) {
      return;
    }

    if (event.key === 'Escape') {
      event.preventDefault();
      onCancel();
      return;
    }

    if (event.key === 'Enter' && !event.shiftKey && !event.altKey && !event.metaKey && !event.ctrlKey) {
      event.preventDefault();
      handleSubmit();
    }
  }, [handleSubmit, isImeOwnedKey, onCancel]);

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
        remoteConnectionId={remoteConnectionId}
        excludeSessionId={excludeSessionId}
      />
    );
  }

  return (
    <Composer
      className="user-message-edit-composer"
      data-openbitfun-product-component="user-message-edit-composer"
      data-openbitfun-product-part="root"
      data-openbitfun-mode="plain"
      data-openbitfun-state={isSubmitting ? 'submitting' : undefined}
      disabled={isSubmitting}
      toolbar={(
        <ComposerToolbar
          className="user-message-edit-composer__actions"
          data-openbitfun-product-component="user-message-edit-composer"
          data-openbitfun-product-part="actions"
          trailing={(
            <>
              <IconButton
                aria-label={cancelLabel}
                data-openbitfun-action="cancel"
                data-openbitfun-product-component="user-message-edit-composer"
                data-openbitfun-product-part="action"
                icon={<Icon name="xmark" size="sm" />}
                onClick={onCancel}
                size="xs"
                title={cancelLabel}
                variant="quiet"
              />
              <IconButton
                aria-busy={isSubmitting || undefined}
                aria-label={submitLabel}
                data-openbitfun-action="submit"
                data-openbitfun-product-component="user-message-edit-composer"
                data-openbitfun-product-part="action"
                disabled={!canSubmit}
                icon={isSubmitting ? (
                  <Loader2
                    className="user-message-edit-composer__spinner"
                    data-openbitfun-product-component="user-message-edit-composer"
                    data-openbitfun-product-part="spinner"
                    size={14}
                  />
                ) : <Icon name="check-line" size="sm" />}
                onClick={handleSubmit}
                size="xs"
                title={submitLabel}
                variant="primary"
              />
            </>
          )}
        />
      )}
    >
      <textarea
        data-openbitfun-product-component="user-message-edit-composer"
        data-openbitfun-product-part="input"
        ref={textareaRef}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={handleKeyDown}
        onCompositionStart={handleCompositionStart}
        onCompositionEnd={handleCompositionEnd}
        placeholder={placeholder}
        disabled={isSubmitting}
        className="user-message-edit-composer__textarea"
      />
    </Composer>
  );
};

UserMessageEditComposer.displayName = 'UserMessageEditComposer';
