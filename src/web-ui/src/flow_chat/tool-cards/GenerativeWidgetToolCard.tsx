import React, { useCallback, useMemo } from 'react';
import { Sparkles } from 'lucide-react';
import { CubeLoading } from '../../component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import GenerativeWidgetFrame from '@/tools/generative-widget/GenerativeWidgetFrame';
import { handleWidgetBridgeEvent } from '@/tools/generative-widget/widgetInteraction';
import './GenerativeWidgetToolCard.scss';

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

  const isLoading = status === 'preparing' || status === 'streaming' || status === 'running' || status === 'pending';
  const isFailed = status === 'error' || toolResult?.success === false;
  const widgetId = resultData?.widget_id || toolCall?.id || toolItem.id;

  const handleWidgetEvent = useCallback((event: any) => {
    handleWidgetBridgeEvent(event, 'tool-card');
  }, []);

  const header = (
    <ToolCardHeader
      icon={<Sparkles size={16} />}
      iconClassName="generative-widget-card__icon"
      action="Generative UI"
      content={<span className="generative-widget-card__title">{title}</span>}
      extra={
        isFailed ? (
          <span className="generative-widget-card__status generative-widget-card__status--error">
            Failed
          </span>
        ) : isLoading ? (
          <span className="generative-widget-card__status">Streaming preview</span>
        ) : (
          <span className="generative-widget-card__status">Interactive preview</span>
        )
      }
      statusIcon={isLoading ? <CubeLoading size="small" /> : null}
    />
  );

  const previewContent = isFailed ? (
    <div className="generative-widget-card__placeholder generative-widget-card__placeholder--error">
      {toolResult?.error || 'Widget rendering failed.'}
    </div>
  ) : widgetCode.trim().length > 0 ? (
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
    <div className="generative-widget-card__placeholder">
      Waiting for widget content...
    </div>
  );

  return (
    <BaseToolCard
      status={status}
      isExpanded={true}
      className="generative-widget-card"
      header={header}
      expandedContent={previewContent}
      errorContent={previewContent}
      isFailed={isFailed}
    />
  );
};

export default GenerativeWidgetToolCard;
