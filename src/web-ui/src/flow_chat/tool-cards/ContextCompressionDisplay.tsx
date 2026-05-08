/**
 * Context compression display for Flow Chat.
 */

import React from 'react';
import { Archive } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { CubeLoading } from '../../component-library';
import type { FlowToolItem } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import './ContextCompressionDisplay.scss';

interface ContextCompressionDisplayProps {
  toolItem?: FlowToolItem;
  compressionData?: {
    session_id: string;
    compression_count: number;
    has_summary: boolean;
    summary_source?: 'model' | 'local_fallback' | 'none';
    tokens_before?: number;
    tokens_after?: number;
    compression_ratio?: number;
    duration?: number;
    summary_content?: string;
    trigger?: 'user_message' | 'tool_batch' | 'ai_response' | 'manual';
    compression_tiers?: {
      tier1?: { before: number; after: number; saved: number };
      tier2_3?: { before: number; after: number; saved: number };
      tier4_plus?: { before: number; after: number; saved: number };
    };
  };
}

export const ContextCompressionDisplay: React.FC<ContextCompressionDisplayProps> = ({
  toolItem,
  compressionData
}) => {
  const { t } = useTranslation('flow-chat');
  const data = toolItem ? {
    compressionCount: toolItem.toolResult?.result?.compression_count || compressionData?.compression_count,
    tokensBefore: toolItem.toolResult?.result?.tokens_before || toolItem.toolCall?.input?.tokens_before || compressionData?.tokens_before,
    tokensAfter: toolItem.toolResult?.result?.tokens_after || compressionData?.tokens_after,
    compressionRatio: toolItem.toolResult?.result?.compression_ratio || compressionData?.compression_ratio,
    duration: toolItem.toolResult?.duration_ms || compressionData?.duration,
    hasSummary: toolItem.toolResult?.result?.has_summary ?? compressionData?.has_summary,
    summarySource: toolItem.toolResult?.result?.summary_source || compressionData?.summary_source,
    trigger: toolItem.toolCall?.input?.trigger || compressionData?.trigger,
    status: (toolItem.status === 'cancelled' || toolItem.status === 'analyzing') ? 'completed' : toolItem.status,
    error: toolItem.toolResult?.error
  } : {
    compressionCount: compressionData?.compression_count,
    tokensBefore: compressionData?.tokens_before,
    tokensAfter: compressionData?.tokens_after,
    compressionRatio: compressionData?.compression_ratio,
    duration: compressionData?.duration,
    hasSummary: compressionData?.has_summary,
    summarySource: compressionData?.summary_source,
    trigger: compressionData?.trigger,
    status: 'completed' as const
  };

  const getTriggerText = (triggerType?: string) => {
    switch (triggerType) {
      case 'user_message':
        return t('toolCards.contextCompression.beforeUserMessage');
      case 'tool_batch':
        return t('toolCards.contextCompression.toolBatchComplete');
      case 'ai_response':
        return t('toolCards.contextCompression.afterAiResponse');
      case 'manual':
        return t('toolCards.contextCompression.manualTrigger');
      default:
        return t('toolCards.contextCompression.autoTrigger');
    }
  };

  const savedTokens = data.tokensBefore && data.tokensAfter ? 
    data.tokensBefore - data.tokensAfter : undefined;

  const isLoading = data.status === 'preparing' || data.status === 'streaming' || data.status === 'running';

  const isFailed = data.status === 'error';
  const usedLocalFallback = data.summarySource === 'local_fallback';
  const usedNoSummary = data.summarySource === 'none';

  const renderToolIcon = () => {
    return <Archive size={16} />;
  };

  const renderStatusIcon = () => {
    if (isLoading) {
      return <CubeLoading size="small" />;
    }
    return null;
  };

  const headerAction =
    isFailed
      ? t('toolCards.contextCompression.contextCompressionFailed')
      : usedLocalFallback && data.status === 'completed'
        ? t('toolCards.contextCompression.localFallbackHeader')
        : t('toolCards.contextCompression.contextCompression');

  const renderHeader = () => (
    <ToolCardHeader
      icon={renderToolIcon()}
      iconClassName="compression-icon"
      action={headerAction}
      content={
        <span className="compression-info">
          {data.tokensBefore !== undefined && data.tokensAfter !== undefined ? (
            <>
              <span className="token-stat">
                {t('toolCards.contextCompression.tokenChange', {
                  before: data.tokensBefore.toLocaleString(),
                  after: data.tokensAfter.toLocaleString(),
                })}
              </span>
              {savedTokens !== undefined && data.compressionRatio !== undefined && (
                <span className="savings-tag">
                  {t('toolCards.contextCompression.savingsTag', {
                    saved: savedTokens.toLocaleString(),
                    ratio: (data.compressionRatio * 100).toFixed(0),
                  })}
                </span>
              )}
            </>
          ) : (
            <span className="processing-text">{t('toolCards.contextCompression.compressingContext')}</span>
          )}
        </span>
      }
      extra={
        <>
          {data.status === 'completed' && data.compressionCount && (
            <span className="compression-meta">
              {getTriggerText(data.trigger)} · {t('toolCards.contextCompression.compressionCount', { count: data.compressionCount })}
            </span>
          )}
        </>
      }
      statusIcon={renderStatusIcon()}
    />
  );

  const renderErrorContent = () => (
    <div className="error-content">
      <div className="error-message">{data.error || t('toolCards.contextCompression.contextCompressionFailed')}</div>
    </div>
  );

  const renderExpandedContent = () => {
    if (!usedNoSummary) {
      return null;
    }

    return (
      <div className="compression-detail-note">
        {t('toolCards.contextCompression.noSummaryNotice', {
          defaultValue: 'No additional summary was generated for this compaction pass.',
        })}
      </div>
    );
  };

  return (
    <BaseToolCard
      status={data.status}
      isExpanded={usedNoSummary}
      className="context-compression-display"
      header={renderHeader()}
      expandedContent={renderExpandedContent()}
      errorContent={renderErrorContent()}
      isFailed={isFailed}
    />
  );
};
