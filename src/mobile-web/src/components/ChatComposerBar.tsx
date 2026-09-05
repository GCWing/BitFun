import React from 'react';
import { MobileComposer, MobileIconButton } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';

interface PendingImage {
  dataUrl: string;
  name: string;
}

interface ChatComposerBarProps {
  cancelling: boolean;
  containerRef: React.Ref<HTMLDivElement>;
  expanded: boolean;
  imageAnalyzing: boolean;
  input: string;
  inputRef: React.Ref<HTMLTextAreaElement>;
  modelControls: React.ReactNode;
  onActivate: () => void;
  onAttach: () => void;
  onCancel: () => void;
  onChange: (value: string) => void;
  onCompositionEnd: () => void;
  onCompositionStart: () => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLTextAreaElement>) => void;
  onRemoveImage: (index: number) => void;
  onSend: () => void;
  pendingImages: PendingImage[];
  streaming: boolean;
}

export default function ChatComposerBar({
  cancelling,
  containerRef,
  expanded,
  imageAnalyzing,
  input,
  inputRef,
  modelControls,
  onActivate,
  onAttach,
  onCancel,
  onChange,
  onCompositionEnd,
  onCompositionStart,
  onKeyDown,
  onRemoveImage,
  onSend,
  pendingImages,
  streaming,
}: ChatComposerBarProps) {
  const { t } = useI18n();
  const attachDisabled = imageAnalyzing || pendingImages.length >= 5;

  return (
    <div className={`chat-page__input-wrap ${expanded ? 'is-expanded' : ''}`} ref={containerRef}>
      <MobileComposer
        aria-label={t('chat.collapsedInputPlaceholder')}
        className="chat-page__composer"
        endActions={(
          <>
            {imageAnalyzing ? (
              <MobileIconButton
                appearance="plain"
                aria-label={t('chat.imageAnalyzingPlaceholder')}
                className="chat-page__send-btn is-processing"
                disabled
                icon={(
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                    <circle cx="12" cy="12" r="3" />
                    <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2" />
                  </svg>
                )}
                size="sm"
              />
            ) : streaming ? (
              <MobileIconButton
                appearance="plain"
                aria-label={t('common.stop')}
                className={`chat-page__send-btn is-stop${cancelling ? ' is-cancelling' : ''}`}
                disabled={cancelling}
                icon={cancelling
                  ? <span className="chat-page__stop-spinner" aria-hidden="true" />
                  : <span className="chat-page__stop-glyph" aria-hidden="true" />}
                onClick={onCancel}
                size="sm"
              />
            ) : expanded ? (
              <MobileIconButton
                appearance="plain"
                aria-label={t('common.submit')}
                className="chat-page__send-btn"
                disabled={!input.trim() && pendingImages.length === 0}
                icon={(
                  <svg width="12" height="12" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                    <path d="M10 3L10 17M10 3L5 8M10 3L15 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                )}
                onClick={onSend}
                size="sm"
              />
            ) : null}
          </>
        )}
        expanded={expanded}
        leading={(
          <MobileIconButton
            appearance="plain"
            aria-label={t('common.attachImage')}
            className="chat-page__composer-leading"
            disabled={attachDisabled}
            icon={<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true"><path d="M12 5V19M5 12H19" /></svg>}
            onClick={() => { onActivate(); onAttach(); }}
          />
        )}
        onActivate={!expanded ? onActivate : undefined}
        startActions={(
          <>
            <MobileIconButton
              appearance="plain"
              aria-label={t('common.attachImage')}
              className="chat-page__action-btn"
              disabled={attachDisabled}
              icon={<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true"><path d="M12 4v16M4 12h16" /></svg>}
              onClick={onAttach}
            />
            {modelControls}
            {pendingImages.length > 0 && (
              <div className="chat-page__image-preview-row">
                {pendingImages.map((image, index) => (
                  <div key={`${image.name}-${index}`} className="chat-page__image-thumb">
                    <img src={image.dataUrl} alt={image.name} />
                    <MobileIconButton appearance="plain" size="sm" aria-label={t('common.close')} className="chat-page__image-remove" icon={<span aria-hidden="true">×</span>} onClick={() => onRemoveImage(index)} />
                  </div>
                ))}
              </div>
            )}
          </>
        )}
      >
        <div className="chat-page__input-area">
          {expanded ? (
            <textarea
              className="chat-page__input"
              disabled={imageAnalyzing}
              onChange={(event) => onChange(event.target.value)}
              onCompositionEnd={onCompositionEnd}
              onCompositionStart={onCompositionStart}
              onKeyDown={onKeyDown}
              placeholder={t('chat.inputPlaceholder')}
              ref={inputRef}
              rows={1}
              value={input}
            />
          ) : (
            <span className="chat-page__input-placeholder">
              {imageAnalyzing
                ? t('chat.imageAnalyzingPlaceholder')
                : streaming
                  ? t('chat.collapsedStreamingPlaceholder')
                  : t('chat.collapsedInputPlaceholder')}
            </span>
          )}
        </div>
      </MobileComposer>
    </div>
  );
}
