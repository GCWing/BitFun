import React, { useCallback, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Image, Loader2 } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { notificationService } from '@/shared/notification-system';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import GenerativeWidgetFrame from '@/tools/generative-widget/GenerativeWidgetFrame';
import GenerativeWidgetStaticRenderer from '@/tools/generative-widget/GenerativeWidgetStaticRenderer';
import { handleWidgetBridgeEvent } from '@/tools/generative-widget/widgetInteraction';
import { captureElementToDownloadsPng } from '../utils/captureElementToDownloadsPng';
import { createLogger } from '@/shared/utils/logger';
import './GenerativeWidgetToolCard.scss';

const log = createLogger('GenerativeWidgetToolCard');

type WidgetResult = {
  widget_id?: string;
  title?: string;
  widget_code?: string;
  width?: number;
  height?: number;
  is_svg?: boolean;
};

function parseWidgetResult(raw: unknown): WidgetResult | null {
  if (!raw) return null;
  if (typeof raw === 'string') {
    try {
      return JSON.parse(raw) as WidgetResult;
    } catch {
      return null;
    }
  }
  if (typeof raw === 'object') {
    return raw as WidgetResult;
  }
  return null;
}

export const GenerativeWidgetToolCard: React.FC<ToolCardProps> = ({ toolItem }) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolCall, toolResult, partialParams, isParamsStreaming } = toolItem;
  const resultData = useMemo(() => parseWidgetResult(toolResult?.result), [toolResult?.result]);

  const liveParams = isParamsStreaming ? partialParams : toolCall?.input;
  const widgetCode = useMemo(() => {
    const fromStreaming = liveParams?.widget_code;
    if (typeof fromStreaming === 'string' && fromStreaming.length > 0) {
      return fromStreaming;
    }

    const fromResult = resultData?.widget_code;
    if (typeof fromResult === 'string' && fromResult.length > 0) {
      return fromResult;
    }

    const fromInput = toolCall?.input?.widget_code;
    return typeof fromInput === 'string' ? fromInput : '';
  }, [liveParams, resultData?.widget_code, toolCall?.input]);

  const title = useMemo(() => {
    const fromStreaming = liveParams?.title;
    if (typeof fromStreaming === 'string' && fromStreaming.trim().length > 0) {
      return fromStreaming.trim();
    }

    const fromResult = resultData?.title;
    if (typeof fromResult === 'string' && fromResult.trim().length > 0) {
      return fromResult.trim();
    }

    const fromInput = toolCall?.input?.title;
    if (typeof fromInput === 'string' && fromInput.trim().length > 0) {
      return fromInput.trim();
    }

    return 'Generative UI';
  }, [liveParams, resultData?.title, toolCall?.input]);

  const isFailed = status === 'error' || toolResult?.success === false;
  const failureText = toolResult?.error || 'Widget rendering failed.';
  const widgetId = resultData?.widget_id || toolCall?.id || toolItem.id;
  const hasRenderableWidget = widgetCode.trim().length > 0 && !isFailed;

  const captureRootRef = useRef<HTMLDivElement>(null);
  const exportPreviewRef = useRef<HTMLDivElement>(null);
  const [isExporting, setIsExporting] = useState(false);
  const [shouldRenderExportClone, setShouldRenderExportClone] = useState(false);
  const [exportWidth, setExportWidth] = useState<number | null>(null);

  const handleWidgetEvent = useCallback((event: any) => {
    handleWidgetBridgeEvent(event, 'tool-card');
  }, []);

  const handleExportImage = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      const fallbackRoot = captureRootRef.current;
      if (!fallbackRoot) {
        notificationService.error(t('exportImage.containerNotFound'));
        return;
      }

      setIsExporting(true);
      try {
        let target = fallbackRoot;

        if (hasRenderableWidget) {
          const nextWidth = captureRootRef.current?.clientWidth || 720;
          setExportWidth(nextWidth);
          setShouldRenderExportClone(true);
          await new Promise((r) => setTimeout(r, 180));
          if (exportPreviewRef.current) {
            target = exportPreviewRef.current;
          }
        } else {
          await new Promise((r) => setTimeout(r, 50));
        }

        await captureElementToDownloadsPng(target, t('toolCards.generativeWidget.exportFileNamePrefix'));
      } catch (err) {
        log.error('Generative UI export image failed', err);
        notificationService.error(t('exportImage.exportFailed'));
      } finally {
        setShouldRenderExportClone(false);
        setIsExporting(false);
      }
    },
    [hasRenderableWidget, t],
  );

  const header = (
    <ToolCardHeader
      content={
        isFailed ? (
          <div className="generative-widget-card__header-line generative-widget-card__header-line--failure">
            <div className="generative-widget-card__header-title-wrap">
              <Tooltip content={t('toolCards.generativeWidget.headerTooltip')}>
                <span className="generative-widget-card__header-label">{`${title}\uFF1A`}</span>
              </Tooltip>
              <span className="generative-widget-card__header-failure-text">{failureText}</span>
            </div>
          </div>
        ) : (
          <div className="generative-widget-card__header-line">
            <div className="generative-widget-card__header-title-wrap">
              <Tooltip content={t('toolCards.generativeWidget.headerTooltip')}>
                <span className="generative-widget-card__header-label">{`${title}\uFF1A`}</span>
              </Tooltip>
            </div>
            <Tooltip
              content={isExporting ? t('exportImage.exporting') : t('exportImage.exportToImage')}
              placement="top"
            >
              <button
                type="button"
                className="generative-widget-card__export-image-btn"
                onClick={handleExportImage}
                disabled={isExporting}
                aria-label={t('exportImage.exportToImage')}
              >
                {isExporting ? <Loader2 size={14} className="spinning" /> : <Image size={14} />}
              </button>
            </Tooltip>
          </div>
        )
      }
    />
  );

  const previewInner = isFailed ? null : widgetCode.trim().length > 0 ? (
    <div className="generative-widget-card__preview">
      <GenerativeWidgetFrame
        widgetId={widgetId}
        title={title}
        widgetCode={widgetCode}
        executeScripts={status === 'completed'}
        onWidgetEvent={handleWidgetEvent}
      />
    </div>
  ) : (
    <div className="generative-widget-card__placeholder">Waiting for widget content...</div>
  );

  const expandedBody = (
    <div ref={captureRootRef} className="generative-widget-card__capture-root">
      {previewInner}
    </div>
  );

  return (
    <>
      <BaseToolCard
        status={status}
        isExpanded={true}
        className="generative-widget-card"
        header={header}
        expandedContent={expandedBody}
        isFailed={isFailed}
      />
      {shouldRenderExportClone && hasRenderableWidget && (
        <div className="generative-widget-card__export-stage">
          <div
            ref={exportPreviewRef}
            className="generative-widget-card__export-stage-inner"
            style={{ width: exportWidth ? `${exportWidth}px` : '720px' }}
          >
            <div className="generative-widget-card__preview generative-widget-card__preview--export">
              <GenerativeWidgetStaticRenderer widgetCode={widgetCode} />
            </div>
          </div>
        </div>
      )}
    </>
  );
};

export default GenerativeWidgetToolCard;
