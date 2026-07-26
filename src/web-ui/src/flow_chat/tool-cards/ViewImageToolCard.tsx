import React, { useEffect, useMemo, useRef, useState } from 'react';
import { ChevronDown, ChevronRight, Image as ImageIcon } from 'lucide-react';

import { Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { SmoothHeightCollapse } from '../components/modern/SmoothHeightCollapse';
import './ViewImageToolCard.scss';

const SUPPORTED_IMAGE_MIME_TYPES = new Set([
  'image/png',
  'image/jpeg',
  'image/gif',
  'image/webp',
  'image/bmp',
]);

interface ViewImageResult {
  path: string | null;
  width: number | null;
  height: number | null;
}

function parseResult(result: unknown): ViewImageResult {
  if (!result || typeof result !== 'object' || Array.isArray(result)) {
    return { path: null, width: null, height: null };
  }

  const value = result as Record<string, unknown>;
  return {
    path: typeof value.path === 'string' && value.path.trim() ? value.path : null,
    width: typeof value.width === 'number' && value.width > 0 ? value.width : null,
    height: typeof value.height === 'number' && value.height > 0 ? value.height : null,
  };
}

function imageSource(toolItem: ToolCardProps['toolItem']): string | null {
  const attachment = toolItem.toolResult?.imageAttachments?.[0];
  if (!attachment) return null;

  const mimeType = attachment.mime_type?.toLowerCase();
  if (!SUPPORTED_IMAGE_MIME_TYPES.has(mimeType) || !attachment.data_base64) return null;

  return `data:${mimeType};base64,${attachment.data_base64}`;
}

function fileName(path: string | null): string {
  if (!path) return 'view_image';
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

export const ViewImageToolCard: React.FC<ToolCardProps> = ({ toolItem, onExpand }) => {
  const { t } = useI18n('flow-chat');
  const result = useMemo(() => parseResult(toolItem.toolResult?.result), [toolItem.toolResult?.result]);
  const source = useMemo(() => imageSource(toolItem), [toolItem]);
  const [isExpanded, setIsExpanded] = useState(Boolean(source));
  const [isLightboxOpen, setIsLightboxOpen] = useState(false);
  const [imageFailed, setImageFailed] = useState(false);
  const didAutoExpand = useRef(Boolean(source));
  const toolId = toolItem.id ?? toolItem.toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  useEffect(() => {
    if (!source || didAutoExpand.current) return;
    didAutoExpand.current = true;
    applyExpandedState(isExpanded, true, setIsExpanded, { reason: 'auto' });
  }, [applyExpandedState, isExpanded, source]);

  useEffect(() => {
    setImageFailed(false);
  }, [source]);

  const handleToggle = () => {
    if (!source) return;
    applyExpandedState(isExpanded, !isExpanded, setIsExpanded, { onExpand });
  };

  const path = result.path
    ?? (typeof toolItem.toolCall?.input?.path === 'string' ? toolItem.toolCall.input.path : null);
  const title = fileName(path);
  const imageCount = toolItem.toolResult?.imageAttachments?.length ?? 1;
  const viewedImagesText = t('toolCards.viewImage.viewedImages', { count: imageCount });
  const viewingText = t('toolCards.viewImage.viewing');
  const statusText = toolItem.status === 'error'
    ? toolItem.toolResult?.error ?? t('toolCards.default.failed')
    : toolItem.status === 'completed'
      ? viewedImagesText === 'toolCards.viewImage.viewedImages'
        ? t('toolCards.default.completed')
        : viewedImagesText
      : viewingText === 'toolCards.viewImage.viewing'
        ? t('toolCards.default.executing')
        : viewingText;

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <CompactToolCard
        status={toolItem.status}
        isExpanded={false}
        onClick={handleToggle}
        clickable={Boolean(source)}
        className="view-image-tool-card"
        header={(
          <CompactToolCardHeader
            icon={(
              <ToolCardStatusSlot
                status={toolItem.status}
                toolIcon={<ImageIcon size={16} />}
              />
            )}
            action={statusText}
            rightStatusIcon={source ? (isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />) : undefined}
          />
        )}
      />

      <SmoothHeightCollapse
        isOpen={Boolean(source && isExpanded)}
        className="view-image-tool-card__collapse"
      >
        {source ? (
          <div className="view-image-tool-card__content">
            {imageFailed ? (
              <div className="view-image-tool-card__error" role="alert">
                {t('toolCards.default.failed')}
              </div>
            ) : (
              <button
                type="button"
                className="view-image-tool-card__preview-button"
                aria-label={t('toolCards.common.viewDetails')}
                onClick={(event) => {
                  event.stopPropagation();
                  setIsLightboxOpen(true);
                }}
              >
                <img
                  src={source}
                  alt={title}
                  width={result.width ?? undefined}
                  height={result.height ?? undefined}
                  title={path ?? undefined}
                  onError={() => setImageFailed(true)}
                />
              </button>
            )}
          </div>
        ) : null}
      </SmoothHeightCollapse>

      <Modal
        isOpen={isLightboxOpen && Boolean(source) && !imageFailed}
        onClose={() => setIsLightboxOpen(false)}
        title={title}
        size="large"
      >
        <div className="view-image-tool-card__lightbox">
          <img src={source ?? ''} alt={title} />
        </div>
      </Modal>
    </div>
  );
};
