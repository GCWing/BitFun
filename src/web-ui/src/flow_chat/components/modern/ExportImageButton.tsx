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
import type { DialogTurn, FlowTextItem, FlowToolItem, FlowThinkingItem } from '../../types/flow-chat';
import { i18nService } from '@/infrastructure/i18n';
import { workspaceAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import { downloadDir, join } from '@tauri-apps/api/path';
import { writeFile } from '@tauri-apps/plugin-fs';
import { ModelThinkingDisplay } from '../../tool-cards/ModelThinkingDisplay';
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

// Marker class on the logo placeholder so we can locate it for canvas compositing.
const LOGO_PLACEHOLDER_CLASS = 'export-content__logo-placeholder';

const ExportContent: React.FC<ExportContentProps> = ({ dialogTurn }) => {
  return (
    <div className="export-content">
      <div className="export-content__header">
        {/* Placeholder reserves space for the logo. The actual logo is drawn
            onto the final image via canvas compositing to avoid html-to-image
            issues with embedding <img>/data URLs inside an SVG foreignObject. */}
        <div
          className={`export-content__logo ${LOGO_PLACEHOLDER_CLASS}`}
          aria-label="BitFun"
        />
        <div className="export-content__title-group">
          <div className="export-content__title">BitFun</div>
          <div className="export-content__subtitle">{i18nService.t('flow-chat:exportImage.subtitle').replace(/ /g, '\u00A0')}</div>
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
                            isStreaming: false,
                          }}
                        />
                      </div>
                    );
                  }
                } else if (item.type === 'thinking') {
                  const thinkingItem = item as FlowThinkingItem;
                  return (
                    <div key={item.id} className="export-content__thinking-item">
                      <ModelThinkingDisplay 
                        thinkingItem={{
                          ...thinkingItem,
                          isStreaming: false,
                        }}
                        isLastItem={true}
                      />
                    </div>
                  );
                } else if (item.type === 'tool') {
                  const toolItem = item as FlowToolItem;
                  return (
                    <div key={item.id} className="export-content__tool-item">
                      <FlowToolCard toolItem={toolItem} />
                    </div>
                  );
                }
                return null;
              })}
          </div>
        ))}
      </div>

      <div className="export-content__footer">
        <span>{i18nService.t('flow-chat:exportImage.poweredBy').replace(/ /g, '\u00A0')}</span>
        <span className="export-content__footer-brand">BitFun</span>
        <span>•</span>
        <span>{i18nService.t('flow-chat:exportImage.aiAssistant').replace(/ /g, '\u00A0')}</span>
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

      // Pre-load the logo as an HTMLImageElement. We do NOT try to embed it
      // inside the captured DOM (html-to-image is unreliable with <img>/data URLs
      // inside an SVG foreignObject and frequently drops them). Instead we
      // reserve space with a placeholder element and composite the logo onto
      // the final raster via canvas after html-to-image finishes.
      let logoImage: HTMLImageElement | null = null;
      try {
        logoImage = await new Promise<HTMLImageElement>((resolve, reject) => {
          const img = new window.Image();
          img.crossOrigin = 'anonymous';
          img.onload = () => resolve(img);
          img.onerror = reject;
          // Cache-bust to avoid stale decoded copies when re-exporting.
          img.src = `/Logo-ICON.png?t=${Date.now()}`;
        });
      } catch (e) {
        log.warn('Logo preload failed; export will proceed without logo overlay', e);
      }
      
      // Create hidden wrapper for rendering.
      // Use left: -9999px (not z-index: -9999) so elements remain visible to html-to-image.
      const wrapper = document.createElement('div');
      wrapper.id = 'export-image-wrapper';
      wrapper.className = 'export-image-wrapper';
      wrapper.style.cssText = `
        position: fixed;
        left: -9999px;
        top: 0;
        z-index: 9999;
        pointer-events: none;
        background: ${bgColor};
        visibility: visible;
        opacity: 1;
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
      
      // Wait for render and images to load.
      await new Promise(resolve => setTimeout(resolve, 500));
      
      // Find rendered container.
      const container = wrapper.querySelector('.export-content') as HTMLElement;
      
      if (!container) {
        throw new Error(i18nService.t('flow-chat:exportImage.containerNotFound'));
      }
      
      // Yield to the main thread.
      await new Promise(resolve => setTimeout(resolve, 0));

      // Capture the logo placeholder position (relative to the container)
      // BEFORE the DOM is torn down, so we can composite the logo precisely
      // onto the final image.
      const containerRect = container.getBoundingClientRect();
      const placeholderEl = container.querySelector(`.${LOGO_PLACEHOLDER_CLASS}`) as HTMLElement | null;
      let logoBox: { x: number; y: number; w: number; h: number; radius: number } | null = null;
      if (placeholderEl) {
        const r = placeholderEl.getBoundingClientRect();
        const radiusStr = getComputedStyle(placeholderEl).getPropertyValue('border-radius');
        const radius = parseFloat(radiusStr) || 0;
        logoBox = {
          x: r.left - containerRect.left,
          y: r.top - containerRect.top,
          w: r.width,
          h: r.height,
          radius,
        };
      }

      // Generate image via html-to-image.
      const htmlToImage = await loadHtmlToImage();
      
      // Yield again before capture.
      await new Promise(resolve => setTimeout(resolve, 0));
      
      const pixelRatio = 2;
      const captureFilter = (node: HTMLElement) => {
        // Filter out action buttons and other non-export elements.
        if (node.classList?.contains('model-round-item__footer')) return false;
        if (node.classList?.contains('user-message-item__actions')) return false;
        if (node.classList?.contains('tool-card__actions')) return false;
        if (node.classList?.contains('base-tool-card__confirm-actions')) return false;
        if (node.classList?.contains('base-tool-card-expanded')) return false;
        if (node.classList?.contains('compact-tool-card-expanded')) return false;
        return true;
      };

      let baseDataUrl: string;
      try {
        baseDataUrl = await htmlToImage.toPng(container, {
          quality: 1,
          pixelRatio,
          backgroundColor: bgColor,
          skipFonts: true,
          cacheBust: true,
          filter: captureFilter,
        });
      } catch (e) {
        log.warn('toPng failed, retrying with toBlob', e);
        const fallbackBlob = await htmlToImage.toBlob(container, {
          quality: 1,
          pixelRatio,
          backgroundColor: bgColor,
          skipFonts: true,
          cacheBust: true,
          filter: captureFilter,
        });
        if (!fallbackBlob) throw new Error(i18nService.t('flow-chat:exportImage.generateFailed'));
        baseDataUrl = await new Promise<string>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(reader.result as string);
          reader.onerror = reject;
          reader.readAsDataURL(fallbackBlob);
        });
      }

      // Cleanup the offscreen DOM as soon as we have the base capture.
      root.unmount();
      document.body.removeChild(wrapper);

      // Composite the logo onto the captured image so it always appears,
      // regardless of html-to-image's behavior with <img> elements.
      const blob: Blob | null = await new Promise<Blob | null>((resolve, reject) => {
        const baseImg = new window.Image();
        baseImg.onload = () => {
          try {
            const canvas = document.createElement('canvas');
            canvas.width = baseImg.naturalWidth;
            canvas.height = baseImg.naturalHeight;
            const ctx = canvas.getContext('2d');
            if (!ctx) { reject(new Error('no canvas ctx for compositing')); return; }
            ctx.drawImage(baseImg, 0, 0);

            if (logoImage && logoBox) {
              const dx = logoBox.x * pixelRatio;
              const dy = logoBox.y * pixelRatio;
              const dw = logoBox.w * pixelRatio;
              const dh = logoBox.h * pixelRatio;
              const dr = logoBox.radius * pixelRatio;

              ctx.save();
              if (dr > 0) {
                const path = new Path2D();
                const r = Math.min(dr, dw / 2, dh / 2);
                path.moveTo(dx + r, dy);
                path.lineTo(dx + dw - r, dy);
                path.quadraticCurveTo(dx + dw, dy, dx + dw, dy + r);
                path.lineTo(dx + dw, dy + dh - r);
                path.quadraticCurveTo(dx + dw, dy + dh, dx + dw - r, dy + dh);
                path.lineTo(dx + r, dy + dh);
                path.quadraticCurveTo(dx, dy + dh, dx, dy + dh - r);
                path.lineTo(dx, dy + r);
                path.quadraticCurveTo(dx, dy, dx + r, dy);
                path.closePath();
                ctx.clip(path);
              }
              ctx.imageSmoothingEnabled = true;
              ctx.imageSmoothingQuality = 'high';
              ctx.drawImage(logoImage, dx, dy, dw, dh);
              ctx.restore();
            } else {
              log.warn('Logo overlay skipped', { hasLogo: !!logoImage, hasBox: !!logoBox });
            }

            canvas.toBlob((b) => resolve(b), 'image/png');
          } catch (err) {
            reject(err);
          }
        };
        baseImg.onerror = reject;
        baseImg.src = baseDataUrl;
      });

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
