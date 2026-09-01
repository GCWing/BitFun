/**
 * Snapshot fullscreen diff viewer for all session file changes.
 */

import React, { useState, useEffect, useCallback } from 'react';
import { Button, IconButton } from '@bitfun/ui';
import { createPortal } from 'react-dom';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { X, CheckCircle, XCircle, FileText, ChevronLeft, ChevronRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@bitfun/ui';
import { DiffEditor } from '../../tools/editor';
import type { SnapshotFile } from '../../tools/snapshot_system/core/SnapshotStateManager';
import { createLogger } from '@/shared/utils/logger';
import { isImeOwnedKeyboardEvent } from '@/shared/utils/ime';
import './SnapshotFullscreenDiffViewer.css';

const log = createLogger('SnapshotFullscreenDiffViewer');

interface SnapshotFullscreenDiffViewerProps {
  isOpen: boolean;
  onClose: () => void;
  sessionId?: string;
  files: SnapshotFile[];
  onAcceptFile: (filePath: string) => Promise<void>;
  onRejectFile: (filePath: string) => Promise<void>;
  onAcceptBlock: (filePath: string, blockId: string) => Promise<void>;
  onRejectBlock: (filePath: string, blockId: string) => Promise<void>;
  loading?: boolean;
}

export const SnapshotFullscreenDiffViewer: React.FC<SnapshotFullscreenDiffViewerProps> = ({
  isOpen,
  onClose,
  files,
  onAcceptFile,
  onRejectFile,
  onAcceptBlock: _onAcceptBlock,
  onRejectBlock: _onRejectBlock,
  loading = false
}) => {
  const { t } = useTranslation('flow-chat');
  const [selectedFileIndex, setSelectedFileIndex] = useState(0);

  // Close on Escape key.
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen && !isImeOwnedKeyboardEvent(e)) {
        onClose();
      }
    };

    if (isOpen) {
      document.addEventListener('keydown', handleEscape);
      // Prevent background scrolling while open.
      document.body.style.overflow = 'hidden';
    }

    return () => {
      document.removeEventListener('keydown', handleEscape);
      document.body.style.overflow = '';
    };
  }, [isOpen, onClose]);

  // Keyboard navigation across files.
  useEffect(() => {
    const handleKeyboard = (e: KeyboardEvent) => {
      if (!isOpen || files.length <= 1) return;
      
      if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedFileIndex(prev => prev > 0 ? prev - 1 : files.length - 1);
      } else if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedFileIndex(prev => prev < files.length - 1 ? prev + 1 : 0);
      }
    };

    if (isOpen) {
      document.addEventListener('keydown', handleKeyboard);
    }

    return () => {
      document.removeEventListener('keydown', handleKeyboard);
    };
  }, [isOpen, files.length]);

  // Reset selection when opening.
  useEffect(() => {
    if (isOpen && files.length > 0) {
      setSelectedFileIndex(0);
    }
  }, [isOpen, files.length]);

  const handleBackdropClick = useCallback((e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }, [onClose]);

  // File-level actions with error logging.
  const handleFileAction = useCallback(async (action: 'accept' | 'reject') => {
    if (selectedFileIndex >= files.length) return;
    
    const file = files[selectedFileIndex];
    try {
      if (action === 'accept') {
        await onAcceptFile(file.filePath);
      } else {
        await onRejectFile(file.filePath);
      }
    } catch (error) {
      log.error(`File ${action} operation failed`, { filePath: file.filePath, action, error });
    }
  }, [selectedFileIndex, files, onAcceptFile, onRejectFile]);

  // Batch actions with error logging.
  const handleBatchAction = useCallback(async (action: 'accept' | 'reject') => {
    try {
      for (const file of files) {
        if (action === 'accept') {
          await onAcceptFile(file.filePath);
        } else {
          await onRejectFile(file.filePath);
        }
      }
    } catch (error) {
      log.error(`Batch ${action} operation failed`, { action, fileCount: files.length, error });
    }
  }, [files, onAcceptFile, onRejectFile]);

  if (!isOpen || files.length === 0) return null;

  const currentFile = files[selectedFileIndex];
  const fileName = currentFile?.filePath.split(/[/\\]/).pop() || '';

  // Aggregate change stats for the header.
  const stats = {
    totalFiles: files.length,
    totalAdditions: files.reduce((sum, file) => {
      const diff = file.modifiedContent.split('\n').length - file.originalContent.split('\n').length;
      return sum + Math.max(0, diff);
    }, 0),
    totalDeletions: files.reduce((sum, file) => {
      const diff = file.originalContent.split('\n').length - file.modifiedContent.split('\n').length;
      return sum + Math.max(0, diff);
    }, 0)
  };

  const fullscreenContent = (
    <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="overlay" className="snapshot-fullscreen-overlay" onClick={handleBackdropClick}>
      <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="root" className="snapshot-fullscreen-container">
        <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="header" className="snapshot-fullscreen-header">
          <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="sessionInfo" className="session-info">
            <div className="session-icon">
              <FileText size={20} />
            </div>
            <div className="session-details">
              <div className="session-title">{t('toolCards.snapshot.fileDiff')}</div>
              <div className="session-stats">
                {t('toolCards.snapshot.filesCount', { count: stats.totalFiles })}
                {stats.totalAdditions > 0 && <span className="additions">+{stats.totalAdditions}</span>}
                {stats.totalDeletions > 0 && <span className="deletions">-{stats.totalDeletions}</span>}
              </div>
            </div>
          </div>

          <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="headerActions" className="header-actions">
            <Tooltip content={t('toolCards.snapshot.acceptAllTooltip')}>
              <Button
                type="button"
                variant="fill"
                size="sm"
                leadingIcon={<CheckCircle size={16} />}
                onClick={() => handleBatchAction('accept')}
                disabled={loading}
              >
                {t('toolCards.snapshot.acceptAll')}
              </Button>
            </Tooltip>
            
            <Tooltip content={t('toolCards.snapshot.rejectAllTooltip')}>
              <Button
                type="button"
                variant="outline"
                size="sm"
                leadingIcon={<XCircle size={16} />}
                onClick={() => handleBatchAction('reject')}
                disabled={loading}
              >
                {t('toolCards.snapshot.rejectAll')}
              </Button>
            </Tooltip>

            <div className="header-divider" />

            <Tooltip content={t('toolCards.snapshot.close')}>
              <IconButton
                type="button"
                size="sm"
                onClick={onClose}
                aria-label={t('toolCards.snapshot.close')}
                icon={<X size={16} />}
              />
            </Tooltip>
          </div>
        </div>

        {files.length > 1 && (
          <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="navigation" className="file-navigation">
            <Tooltip content={t('toolCards.snapshot.prevFile')}>
              <IconButton
                type="button"
                size="sm"
                onClick={() => setSelectedFileIndex(prev => prev > 0 ? prev - 1 : files.length - 1)}
                disabled={loading}
                aria-label={t('toolCards.snapshot.prevFile')}
                icon={<ChevronLeft size={16} />}
              />
            </Tooltip>

            <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="tabs" className="file-tabs">
              {files.map((file, index) => {
                const name = file.filePath.split(/[/\\]/).pop() || '';
                return (
                  <button data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="tab" data-bf-state={index === selectedFileIndex ? 'active' : undefined}
                    key={index}
                    className={`file-tab ${index === selectedFileIndex ? 'active' : ''}`}
                    onClick={() => setSelectedFileIndex(index)}
                    title={file.filePath}
                  >
                    <span className="file-name">{name}</span>
                    <span className="file-status" data-status={file.fileStatus}>
                      {file.fileStatus === 'pending' ? '●' : 
                       file.fileStatus === 'accepted' ? '✓' : 
                       file.fileStatus === 'rejected' ? '✗' : '◐'}
                    </span>
                  </button>
                );
              })}
            </div>

            <Tooltip content={t('toolCards.snapshot.nextFile')}>
              <IconButton
                type="button"
                size="sm"
                onClick={() => setSelectedFileIndex(prev => prev < files.length - 1 ? prev + 1 : 0)}
                disabled={loading}
                aria-label={t('toolCards.snapshot.nextFile')}
                icon={<ChevronRight size={16} />}
              />
            </Tooltip>
          </div>
        )}

        <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="currentFile" className="current-file-header">
          <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="fileInfo" className="file-info">
            <div className="file-icon">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                <polyline points="14,2 14,8 20,8"/>
              </svg>
            </div>
            <div className="file-details">
              <div className="file-name">{fileName}</div>
              <div className="file-path-full">{currentFile.filePath}</div>
            </div>
          </div>

          <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="fileActions" className="current-file-actions">
            <Tooltip content={t('toolCards.snapshot.acceptFileTooltip')}>
              <Button
                type="button"
                variant="fill"
                size="sm"
                leadingIcon={<CheckCircle size={16} />}
                onClick={() => handleFileAction('accept')}
                disabled={loading}
              >
                {t('toolCards.snapshot.acceptFile')}
              </Button>
            </Tooltip>
            
            <Tooltip content={t('toolCards.snapshot.rejectFileTooltip')}>
              <Button
                type="button"
                variant="outline"
                size="sm"
                leadingIcon={<XCircle size={16} />}
                onClick={() => handleFileAction('reject')}
                disabled={loading}
              >
                {t('toolCards.snapshot.rejectFile')}
              </Button>
            </Tooltip>
          </div>
        </div>

        <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="content" className="snapshot-fullscreen-content">
          {currentFile && (
            <DiffEditor
              originalContent={currentFile.originalContent}
              modifiedContent={currentFile.modifiedContent}
              filePath={currentFile.filePath}
              readOnly={false}
              renderSideBySide={true}
              showMinimap={false}
            />
          )}
        </div>

        {loading && (
          <div data-bf-component="snapshot-fullscreen-diff-viewer" data-bf-part="loading" className="fullscreen-loading-overlay">
            <div className="loading-spinner" />
            <span>{t('toolCards.snapshot.processing')}</span>
          </div>
        )}
      </div>
    </div>
  );

  return createPortal(fullscreenContent, getAppearanceOverlayHost());
};
