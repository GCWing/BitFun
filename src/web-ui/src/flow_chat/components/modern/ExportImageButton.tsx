/**
 * Export dialog turns as long images.
 * Uses React rendering to match FlowChat styles.
 */

import React, { useState, useCallback } from 'react';
import { createRoot } from 'react-dom/client';
import { Image, Loader2 } from 'lucide-react';
import { FlowChatStore } from '../../store/FlowChatStore';
import { notificationService } from '@/shared/notification-system';
import { FlowTextBlock } from '../FlowTextBlock';
import { FlowToolCard } from '../FlowToolCard';
import { Tooltip } from '@/component-library';
import type { DialogTurn, FlowTextItem, FlowToolItem } from '../../types/flow-chat';
import { i18nService } from '@/infrastructure/i18n';
import { workspaceAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import { downloadDir, join } from '@tauri-apps/api/path';
import { writeFile } from '@tauri-apps/plugin-fs';
import './ExportImageButton.scss';

const log = createLogger('ExportImageButton');

// Lazy-load html-to-image.
const loadHtmlToImage = () => import('html-to-image');

interface ExportImageButtonProps {
  turnId: string;
  className?: string;
}

// Exported content renderer.
interface ExportContentProps {
  dialogTurn: DialogTurn;
}

const ExportContent: React.FC<ExportContentProps> = ({ dialogTurn }) => {
  return (
    <div className="export-content">
      <div className="export-content__header">
        <img 
          src="/Logo-ICON.png" 
          alt="BitFun" 
          className="export-content__logo"
          onError={(e) => {
            // Hide if the logo fails to load.
            (e.target as HTMLImageElement).style.display = 'none';
          }}
        />
        <div className="export-content__title-group">
          <div className="export-content__title">BitFun</div>
          <div className="export-content__subtitle">{i18nService.t('flow-chat:exportImage.subtitle')}</div>
        </div>
        <div className="export-content__timestamp">
          {i18nService.formatDate(new Date())}
        </div>
      </div>

      {dialogTurn.userMessage?.content && (
        <div className="export-content__user-section">
          <div className="export-content__user-bubble">
            {dialogTurn.userMessage.content}
          </div>
        </div>
      )}

      <div className="export-content__ai-section">
        
        {dialogTurn.modelRounds.map((modelRound) => (
          <div key={modelRound.id} className="export-content__model-round">
            {[...modelRound.items]
              .sort((a, b) => a.timestamp - b.timestamp)
              .map((item) => {
                if (item.type === 'text') {
                  const textItem = item as FlowTextItem;
                  if (textItem.content && textItem.content.trim()) {
                    return (
                      <div key={item.id} className="export-content__text-item">
                        <FlowTextBlock 
                          textItem={{
                            ...textItem,
                            isStreaming: false, // Hide streaming state in exports.
                          }}
                        />
                      </div>
                    );
                  }
                } else if (item.type === 'tool') {
                  const toolItem = item as FlowToolItem;
                  return (
                    <div key={item.id} className="export-content__tool-item">
                      <FlowToolCard 
                        toolItem={toolItem}
                        // Disable interactions in exports.
                      />
                    </div>
                  );
                }
                return null;
              })}
          </div>
        ))}
      </div>

      <div className="export-content__footer">
        <span>{i18nService.t('flow-chat:exportImage.poweredBy')}</span>
        <span className="export-content__footer-brand">BitFun</span>
        <span>•</span>
        <span>{i18nService.t('flow-chat:exportImage.aiAssistant')}</span>
      </div>
    </div>
  );
};

export const ExportImageButton: React.FC<ExportImageButtonProps> = ({
  turnId,
  className = ''
}) => {
  const [isExporting, setIsExporting] = useState(false);

  // Resolve the DialogTurn by id.
  const getDialogTurn = useCallback((): DialogTurn | null => {
    const flowChatStore = FlowChatStore.getInstance();
    const state = flowChatStore.getState();
    
    for (const [, session] of state.sessions) {
      const turn = session.dialogTurns.find((t: DialogTurn) => t.id === turnId);
      if (turn) return turn;
    }
    return null;
  }, [turnId]);

  const handleExport = useCallback(async () => {
    setIsExporting(true);
    
    // Let animations start rendering.
    await new Promise(resolve => setTimeout(resolve, 50));
    
    try {
      const dialogTurn = getDialogTurn();
      
      if (!dialogTurn) {
        notificationService.error(i18nService.t('flow-chat:exportImage.dialogNotFound'));
        return;
      }
      
      // Get theme background color.
      const computedStyle = getComputedStyle(document.documentElement);
      const bgColor = computedStyle.getPropertyValue('--color-bg-flowchat').trim() || '#121214';
      
      // Create hidden wrapper for rendering.
      const wrapper = document.createElement('div');
      wrapper.id = 'export-image-wrapper';
      wrapper.className = 'export-image-wrapper';
      wrapper.style.cssText = `
        position: fixed;
        left: 0;
        top: 0;
        z-index: -9999;
        pointer-events: none;
        background: ${bgColor};
      `;
      
      // Copy CSS variables for consistent styling.
      const cssVars = [
        '--color-bg-flowchat', '--color-bg-elevated', '--color-bg-base',
        '--color-text-primary', '--color-text-secondary', '--color-text-muted',
        '--border-base', '--border-medium', '--border-prominent',
        '--element-bg-base', '--element-bg-soft', '--element-bg-medium', '--element-bg-strong',
        '--color-success', '--color-success-bg', '--color-warning', '--color-error',
        '--color-primary', '--color-accent',
      ];
      cssVars.forEach(varName => {
        const value = computedStyle.getPropertyValue(varName);
        if (value) {
          wrapper.style.setProperty(varName, value.trim());
        }
      });
      
      document.body.appendChild(wrapper);
      
      // Render export content with React.
      const root = createRoot(wrapper);
      root.render(<ExportContent dialogTurn={dialogTurn} />);
      
      // Wait for render.
      await new Promise(resolve => setTimeout(resolve, 200));
      
      // Find rendered container.
      const container = wrapper.querySelector('.export-content') as HTMLElement;
      
      if (!container) {
        throw new Error(i18nService.t('flow-chat:exportImage.containerNotFound'));
      }
      
      // Yield to the main thread.
      await new Promise(resolve => setTimeout(resolve, 0));

      // Generate image via html-to-image.
      const htmlToImage = await loadHtmlToImage();
      
      // Yield again before capture.
      await new Promise(resolve => setTimeout(resolve, 0));
      
      let blob: Blob | null = null;
      
      try {
        blob = await htmlToImage.toBlob(container, {
          quality: 1,
          pixelRatio: 2,
          backgroundColor: bgColor,
          skipFonts: true,
          cacheBust: true,
          filter: (node: HTMLElement) => {
            // Filter out action buttons and other non-export elements.
            if (node.classList?.contains('model-round-item__footer')) return false;
            if (node.classList?.contains('user-message-item__actions')) return false;
            if (node.classList?.contains('tool-card__actions')) return false;
            if (node.classList?.contains('base-tool-card__confirm-actions')) return false;
            return true;
          },
        });
      } catch (e) {
        log.warn('toBlob failed, trying toPng', e);
        
        const dataUrl = await htmlToImage.toPng(container, {
          quality: 1,
          pixelRatio: 2,
          backgroundColor: bgColor,
          skipFonts: true,
          cacheBust: true,
        });
        
        const response = await fetch(dataUrl);
        blob = await response.blob();
      }

      // Cleanup.
      root.unmount();
      document.body.removeChild(wrapper);

      if (!blob) {
        throw new Error(i18nService.t('flow-chat:exportImage.generateFailed'));
      }

      // Save image to downloads directory.
      const timestampStr = i18nService.formatDate(new Date(), {
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
      }).replace(/[/:\s]/g, '-');
      const fileName = `${i18nService.t('flow-chat:exportImage.fileNamePrefix')}_${timestampStr}.png`;
      const downloadsPath = await downloadDir();
      const filePath = await join(downloadsPath, fileName);

      const arrayBuffer = await blob.arrayBuffer();
      await writeFile(filePath, new Uint8Array(arrayBuffer));

      const plainSuccessMessage = i18nService.t('flow-chat:exportImage.exportSuccess', { filePath });
      const successPrefix = i18nService.t('flow-chat:exportImage.exportSuccessPrefix');

      const revealExportedFile = async () => {
        if (typeof window === 'undefined' || !('__TAURI__' in window)) {
          return;
        }
        try {
          await workspaceAPI.revealInExplorer(filePath);
        } catch (error) {
          log.error('Failed to reveal export path in file manager', { filePath, error });
        }
      };

      notificationService.success(plainSuccessMessage, {
        messageNode: (
          <>
            {successPrefix}
            <button
              type="button"
              className="notification-item__path-link"
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                void revealExportedFile();
              }}
            >
              {filePath}
            </button>
          </>
        ),
      });
    } catch (error) {
      log.error('Export failed', error);
      notificationService.error(i18nService.t('flow-chat:exportImage.exportFailed'));
    } finally {
      setIsExporting(false);
    }
  }, [getDialogTurn]);

  return (
    <Tooltip content={isExporting ? i18nService.t('flow-chat:exportImage.exporting') : i18nService.t('flow-chat:exportImage.exportToImage')} placement="top">
      <button
        className={`model-round-item__action-btn model-round-item__export-btn ${className}`}
        onClick={handleExport}
        disabled={isExporting}
      >
        {isExporting ? <Loader2 size={14} className="spinning" /> : <Image size={14} />}
      </button>
    </Tooltip>
  );
};

ExportImageButton.displayName = 'ExportImageButton';
