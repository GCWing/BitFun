import React from 'react';
import { Check, RotateCcw, X } from 'lucide-react';
import { Button } from '@/component-library';
import { InlineMarkdownPreview } from './InlineMarkdownPreview';

type InlineAiPreviewStatus = 'submitting' | 'streaming' | 'ready' | 'error';

interface InlineAiPreviewBlockLabels {
  title: string;
  streaming: string;
  ready: string;
  error: string;
  accept: string;
  reject: string;
  retry: string;
}

interface InlineAiPreviewBlockProps {
  status: InlineAiPreviewStatus;
  response: string;
  error: string | null;
  basePath?: string;
  canAccept: boolean;
  labels: InlineAiPreviewBlockLabels;
  onAccept: () => void;
  onReject: () => void;
  onRetry: () => void;
}

export const InlineAiPreviewBlock: React.FC<InlineAiPreviewBlockProps> = ({
  status,
  response,
  error,
  basePath,
  canAccept,
  labels,
  onAccept,
  onReject,
  onRetry,
}) => {
  const handlePointerDown: React.PointerEventHandler<HTMLElement> = (event) => {
    // Keep ProseMirror from stealing focus and remounting the widget before click fires.
    event.preventDefault();
    event.stopPropagation();
  };

  const handleMouseDown: React.MouseEventHandler<HTMLElement> = (event) => {
    event.preventDefault();
    event.stopPropagation();
  };

  const statusText =
    status === 'submitting' || status === 'streaming'
      ? labels.streaming
      : status === 'error'
        ? labels.error
        : labels.ready;
  const previewStateClass = `m-editor-inline-ai-preview--${status}`;

  return (
    <div
      className={`m-editor-inline-ai-preview m-editor-inline-ai-preview--inline ${previewStateClass}`}
      data-testid="md-inline-ai-preview"
      data-status={status}
      onPointerDownCapture={handlePointerDown}
      onMouseDownCapture={handleMouseDown}
    >
      <div className="m-editor-inline-ai-preview__header">
        <span className="m-editor-inline-ai-preview__title">{labels.title}</span>
        <span className="m-editor-inline-ai-preview__status">{statusText}</span>
      </div>
      <div className="m-editor-inline-ai-preview__body">
        {response ? (
          <InlineMarkdownPreview value={response} basePath={basePath} />
        ) : (
          <div className="m-editor-inline-ai-preview__placeholder" data-testid="md-inline-ai-preview-placeholder">{labels.streaming}</div>
        )}
        {error && <div className="m-editor-inline-ai__error" data-testid="md-inline-ai-preview-error">{error}</div>}
      </div>
      <div className="m-editor-inline-ai-preview__actions">
        {canAccept && (
          <Button
            type="button"
            variant="primary"
            size="small"
            disabled={!canAccept}
            data-testid="md-inline-ai-accept"
            onClick={onAccept}
          >
            <Check size={14} strokeWidth={2} />
            <span>{labels.accept}</span>
          </Button>
        )}
        <Button
          type="button"
          variant="ghost"
          size="small"
          data-testid="md-inline-ai-reject"
          onClick={onReject}
        >
          <X size={14} strokeWidth={2} />
          <span>{labels.reject}</span>
        </Button>
        {(status === 'ready' || status === 'error') && (
          <Button
            type="button"
            variant="ghost"
            size="small"
            data-testid="md-inline-ai-retry"
            onClick={onRetry}
          >
            <RotateCcw size={14} strokeWidth={2} />
            <span>{labels.retry}</span>
          </Button>
        )}
      </div>
    </div>
  );
};
