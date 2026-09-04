import React, { useEffect, useCallback, useRef } from 'react';
import { Button, Icon, IconButton, Tooltip } from '@openbitfun/ui';
import { createPortal } from 'react-dom';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
;
import { RetainedMountBoundary } from '@/shared/presence';
import { useI18n } from '@/infrastructure/i18n';
import { isImeOwnedKeyboardEvent } from '@/shared/utils/ime';
import { DiffEditor } from '../../../tools/editor';
import './DiffFullscreenViewer.css';

interface DiffFullscreenViewerProps {
  isOpen: boolean;
  onClose: () => void;
  filePath: string;
  originalContent: string;
  modifiedContent: string;
  onAcceptFile: () => void;
  onRejectFile: () => void;
  onAcceptBlock: (blockId: string) => void;
  onRejectBlock: (blockId: string) => void;
  loading?: boolean;
}

export const DiffFullscreenViewer: React.FC<DiffFullscreenViewerProps> = ({
  isOpen,
  onClose,
  filePath,
  originalContent,
  modifiedContent,
  onAcceptFile,
  onRejectFile,
  onAcceptBlock: _onAcceptBlock,
  onRejectBlock: _onRejectBlock,
  loading = false
}) => {
  const { t } = useI18n('components');
  const retainedContentRef = useRef({
    filePath,
    originalContent,
    modifiedContent,
    loading,
  });

  if (isOpen) {
    retainedContentRef.current = {
      filePath,
      originalContent,
      modifiedContent,
      loading,
    };
  }

  const retainedContent = retainedContentRef.current;
  // Close on Escape
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen && !isImeOwnedKeyboardEvent(e)) {
        onClose();
      }
    };

    if (isOpen) {
      document.addEventListener('keydown', handleEscape);
      // Disable page scrolling
      document.body.style.overflow = 'hidden';
    }

    return () => {
      document.removeEventListener('keydown', handleEscape);
      document.body.style.overflow = '';
    };
  }, [isOpen, onClose]);

  const handleBackdropClick = useCallback((e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }, [onClose]);

  const fileName = retainedContent.filePath.split(/[/\\]/).pop() || retainedContent.filePath;

  const fullscreenContent = (
    <div
      className="diff-fullscreen-overlay"
      data-state={isOpen ? 'open' : 'closed'}
      aria-hidden={!isOpen}
      {...(!isOpen ? { inert: '' } : {})}
      onClick={handleBackdropClick}
      data-openbitfun-component="diff-fullscreen-viewer"
      data-openbitfun-part="overlay"
    >
      <div className="diff-fullscreen-container" data-openbitfun-component="diff-fullscreen-viewer" data-openbitfun-part="container">
        {/* Top toolbar */}
        <div className="diff-fullscreen-header" data-openbitfun-component="diff-fullscreen-viewer" data-openbitfun-part="header">
          <div className="file-info" data-openbitfun-component="diff-fullscreen-viewer" data-openbitfun-part="fileInfo">
            <div className="file-icon">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                <polyline points="14,2 14,8 20,8"/>
              </svg>
            </div>
            <div className="file-details">
              <div className="file-name">{fileName}</div>
              <div className="file-path-full">{retainedContent.filePath}</div>
            </div>
          </div>

          <div className="header-actions" data-openbitfun-component="diff-fullscreen-viewer" data-openbitfun-part="actions">
            <Tooltip content={t('diffFullscreen.acceptFileTooltip')}>
              <Button
                variant="fill"
                size="sm"
                leadingIcon={<Icon name="check-circle" size="lg" />}
                onClick={onAcceptFile}
                disabled={retainedContent.loading}
              >
                {t('diffFullscreen.acceptFile')}
              </Button>
            </Tooltip>
            
            <Tooltip content={t('diffFullscreen.rejectFileTooltip')}>
              <Button
                variant="outline"
                size="sm"
                leadingIcon={<Icon name="xmark" size="lg" />}
                onClick={onRejectFile}
                disabled={retainedContent.loading}
              >
                {t('diffFullscreen.rejectFile')}
              </Button>
            </Tooltip>

            <div className="header-divider" />

            <Tooltip content={t('tooltip.close')}>
              <IconButton
                size="sm"
                aria-label={t('tooltip.close')}
                icon={<Icon name="xmark" size="lg" />}
                onClick={onClose}
              />
            </Tooltip>
          </div>
        </div>

        {/* Diff content */}
        <div className="diff-fullscreen-content" data-openbitfun-component="diff-fullscreen-viewer" data-openbitfun-part="content">
          <DiffEditor
            originalContent={retainedContent.originalContent}
            modifiedContent={retainedContent.modifiedContent}
            filePath={retainedContent.filePath}
            readOnly={false}
            renderSideBySide={true}
            showMinimap={false}
          />
        </div>

        {/* Loading overlay */}
        {retainedContent.loading && (
          <div className="fullscreen-loading-overlay" data-openbitfun-component="diff-fullscreen-viewer" data-openbitfun-part="loading">
            <div className="loading-spinner" />
            <span>{t('diffFullscreen.processing')}</span>
          </div>
        )}
      </div>
    </div>
  );

  return createPortal(
    <RetainedMountBoundary present={isOpen}>
      {fullscreenContent}
    </RetainedMountBoundary>,
    getAppearanceOverlayHost(),
  );
};
