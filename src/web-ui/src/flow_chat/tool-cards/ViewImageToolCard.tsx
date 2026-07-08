import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { ExternalLink, ImageIcon, Loader } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Tooltip } from '@/component-library';
import { workspaceAPI } from '@/infrastructure/api';
import { basenamePath, isBitFunRuntimeUri } from '@/shared/utils/pathUtils';
import type { ToolCardProps } from '../types/flow-chat';
import { DefaultToolCard } from './DefaultToolCard';
import './ViewImageToolCard.scss';

function mimeTypeFromPath(path: string): string {
  const ext = path.toLowerCase().split('.').pop();
  const mimeTypes: Record<string, string> = {
    jpg: 'image/jpeg',
    jpeg: 'image/jpeg',
    png: 'image/png',
    gif: 'image/gif',
    bmp: 'image/bmp',
    webp: 'image/webp',
    svg: 'image/svg+xml',
    ico: 'image/x-icon',
    avif: 'image/avif',
  };
  return mimeTypes[ext || ''] || 'image/jpeg';
}

function resolveImagePath(toolItem: ToolCardProps['toolItem']): string {
  const inputPath = typeof toolItem.toolCall?.input?.path === 'string'
    ? toolItem.toolCall.input.path.trim()
    : '';

  const rawResult = toolItem.toolResult?.result;
  let result: Record<string, unknown> | null = null;
  if (typeof rawResult === 'string' && rawResult.trim().length > 0) {
    try {
      result = JSON.parse(rawResult) as Record<string, unknown>;
    } catch {
      result = null;
    }
  } else if (rawResult && typeof rawResult === 'object') {
    result = rawResult as Record<string, unknown>;
  }

  if (typeof result?.path === 'string' && result.path.trim().length > 0) {
    return result.path.trim();
  }

  return inputPath;
}

export const ViewImageToolCard: React.FC<ToolCardProps> = ({
  toolItem,
  config,
  onOpenInEditor,
  onExpand,
  onOpenInPanel,
  sessionId,
  displayContext,
  interruptionNote,
}) => {
  const { t } = useTranslation('flow-chat');
  const { status } = toolItem;
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [loadState, setLoadState] = useState<'idle' | 'loading' | 'loaded' | 'error'>('idle');
  const [loadError, setLoadError] = useState<string | null>(null);

  const imagePath = useMemo(() => resolveImagePath(toolItem), [toolItem]);
  const fileName = useMemo(() => basenamePath(imagePath) || imagePath, [imagePath]);
  const mimeType = imagePath ? mimeTypeFromPath(imagePath) : 'image/jpeg';
  const showInlinePreview = status === 'completed' && Boolean(imagePath);
  const canOpenInEditor = showInlinePreview && Boolean(onOpenInEditor) && !isBitFunRuntimeUri(imagePath);

  useEffect(() => {
    if (status !== 'completed' || !imagePath) {
      setImageUrl(null);
      setLoadState('idle');
      setLoadError(null);
      return;
    }

    if (isBitFunRuntimeUri(imagePath)) {
      setImageUrl(null);
      setLoadState('error');
      setLoadError(t('toolCards.viewImage.runtimeUriUnsupported'));
      return;
    }

    let cancelled = false;

    const loadImage = async () => {
      setLoadState('loading');
      setLoadError(null);
      setImageUrl(null);

      try {
        const base64Content = await workspaceAPI.readFileContent(imagePath);
        if (cancelled) return;

        setImageUrl(`data:${mimeType};base64,${base64Content}`);
        setLoadState('loaded');
      } catch (error) {
        if (cancelled) return;
        setLoadState('error');
        setLoadError(
          error instanceof Error
            ? error.message
            : t('toolCards.viewImage.loadFailed'),
        );
      }
    };

    void loadImage();

    return () => {
      cancelled = true;
    };
  }, [imagePath, mimeType, status, t]);

  const handleOpenInEditor = useCallback((event: React.MouseEvent) => {
    event.stopPropagation();
    if (imagePath && onOpenInEditor) {
      onOpenInEditor(imagePath);
    }
  }, [imagePath, onOpenInEditor]);

  const renderInlinePreview = () => {
    const openOverlay = canOpenInEditor && loadState !== 'error' ? (
      <Tooltip content={t('copyOutput.openInEditor')}>
        <button
          type="button"
          className="view-image-tool-card__open-overlay"
          onClick={handleOpenInEditor}
          aria-label={t('copyOutput.openInEditor')}
        >
          <ExternalLink size={14} />
          <span>{t('copyOutput.openInEditor')}</span>
        </button>
      </Tooltip>
    ) : null;

    if (loadState === 'loading') {
      return (
        <div className="view-image-tool-card__inline-preview view-image-tool-card__inline-preview--loading">
          {openOverlay}
          <Loader size={22} className="view-image-tool-card__spinner" />
          <span>{t('toolCards.viewImage.loading')}</span>
        </div>
      );
    }

    if (loadState === 'error') {
      return (
        <div className="view-image-tool-card__inline-preview view-image-tool-card__inline-preview--error">
          {loadError || t('toolCards.viewImage.loadFailed')}
        </div>
      );
    }

    if (imageUrl) {
      return (
        <div className="view-image-tool-card__inline-preview">
          {openOverlay}
          <img src={imageUrl} alt={fileName} />
        </div>
      );
    }

    return (
      <div className="view-image-tool-card__inline-preview view-image-tool-card__inline-preview--placeholder">
        {openOverlay}
        <ImageIcon size={28} />
      </div>
    );
  };

  return (
    <div data-testid="view-image-tool-card" className="view-image-tool-card">
      <DefaultToolCard
        toolItem={toolItem}
        config={config}
        onExpand={onExpand}
        onOpenInEditor={onOpenInEditor}
        onOpenInPanel={onOpenInPanel}
        sessionId={sessionId}
        displayContext={displayContext}
        interruptionNote={interruptionNote}
      />

      {showInlinePreview && (
        <div className="view-image-tool-card__inline-preview-row">
          {renderInlinePreview()}
        </div>
      )}
    </div>
  );
};

export default ViewImageToolCard;
