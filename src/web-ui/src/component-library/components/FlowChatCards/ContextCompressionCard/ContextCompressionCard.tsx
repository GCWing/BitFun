/**
 * ContextCompressionCard - Context Compression Tool Card Component
 * Displays the AI context compression process and results
 */

import React from 'react';
import { Loader2, CheckCircle, XCircle, Archive } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { BaseToolCard, BaseToolCardProps } from '../BaseToolCard';
import './ContextCompressionCard.scss';

export interface ContextCompressionCardProps extends Omit<BaseToolCardProps, 'toolName' | 'displayName'> {
  compressionCount?: number;
  hasSummary?: boolean;
  tokensBefore?: number;
  tokensAfter?: number;
  compressionRatio?: number;
  duration?: number;
  summaryContent?: string;
  trigger?: 'user_message' | 'tool_batch' | 'ai_response' | 'manual';
  compressionTiers?: {
    tier1?: { before: number; after: number; saved: number };
    tier2_3?: { before: number; after: number; saved: number };
    tier4_plus?: { before: number; after: number; saved: number };
  };
}

export const ContextCompressionCard: React.FC<ContextCompressionCardProps> = ({
  compressionCount = 1,
  tokensBefore,
  tokensAfter,
  compressionRatio,
  duration,
  trigger = 'manual',
  input,
  result,
  status = 'pending',
  displayMode = 'standard',
  ...baseProps
}) => {
  const { t } = useI18n('components');

  const resolvedCompressionCount = compressionCount || result?.compression_count || 1;
  const resolvedTokensBefore = tokensBefore || result?.tokens_before || input?.tokens_before;
  const resolvedTokensAfter = tokensAfter || result?.tokens_after || input?.tokens_after;
  const resolvedCompressionRatio = compressionRatio || result?.compression_ratio || 
    (resolvedTokensBefore && resolvedTokensAfter ? (resolvedTokensAfter / resolvedTokensBefore) : undefined);
  const resolvedDuration = duration || result?.duration;
  const resolvedTrigger = trigger || result?.trigger || input?.trigger || 'manual';

  const getTriggerText = (triggerType: string) => {
    switch (triggerType) {
      case 'user_message':
        return t('flowChatCards.contextCompressionCard.triggerBeforeUserMessage');
      case 'tool_batch':
        return t('flowChatCards.contextCompressionCard.triggerAfterToolBatch');
      case 'ai_response':
        return t('flowChatCards.contextCompressionCard.triggerAfterAiResponse');
      case 'manual':
        return t('flowChatCards.contextCompressionCard.triggerManual');
      default:
        return t('flowChatCards.contextCompressionCard.triggerAuto');
    }
  };

  const savedTokens = resolvedTokensBefore && resolvedTokensAfter ? 
    resolvedTokensBefore - resolvedTokensAfter : undefined;

  const getStatusIcon = (size: number = 14) => {
    switch (status) {
      case 'running':
      case 'streaming':
        return <Loader2 className="context-compression-card__status-spinner" size={size} />;
      case 'completed':
        return <CheckCircle className="context-compression-card__status-success" size={size} />;
      case 'error':
        return <XCircle className="context-compression-card__status-error" size={size} />;
      default:
        return <Archive className="context-compression-card__status-pending" size={size} />;
    }
  };

  if (displayMode === 'compact') {
    return (
      <div className={`context-compression-card context-compression-card--compact status-${status}`}>
        <span className="context-compression-card__status-icon">{getStatusIcon(14)}</span>
        
        <span className="context-compression-card__action">
          {status === 'running' || status === 'streaming' ? t('flowChatCards.contextCompressionCard.compressing') : t('flowChatCards.contextCompressionCard.title')}
        </span>
        
        {resolvedTokensBefore !== undefined && resolvedTokensAfter !== undefined && (
          <span className="context-compression-card__tokens">
            {resolvedTokensBefore.toLocaleString()} → {resolvedTokensAfter.toLocaleString()} tokens
          </span>
        )}
        
        {status === 'completed' && savedTokens !== undefined && resolvedCompressionRatio !== undefined && (
          <span className="context-compression-card__result">
            {t('flowChatCards.contextCompressionCard.savedTokens', { count: savedTokens.toLocaleString(), ratio: (resolvedCompressionRatio * 100).toFixed(0) })}
          </span>
        )}
      </div>
    );
  }

  return (
    <BaseToolCard
      toolName="ContextCompression"
      displayName={t('flowChatCards.contextCompressionCard.title')}
      icon={getStatusIcon(16)}
      description={t('flowChatCards.contextCompressionCard.description')}
      status={status}
      displayMode={displayMode}
      input={input}
      result={result}
      primaryColor="#a855f7"
      className="context-compression-card"
      {...baseProps}
    >
      {(status === 'running' || status === 'streaming') && (
        <div className="context-compression-card__processing">
          <Loader2 className="context-compression-card__processing-icon" size={14} />
          <span>{t('flowChatCards.contextCompressionCard.analyzing')}</span>
        </div>
      )}

      {status === 'completed' && (
        <>
          <div className="context-compression-card__simple-row">
            <span className="context-compression-card__simple-label">
              {t('flowChatCards.contextCompressionCard.triggerTime', { trigger: getTriggerText(resolvedTrigger), count: resolvedCompressionCount })}
            </span>
            {resolvedDuration !== undefined && (
              <span className="context-compression-card__simple-duration">
                {t('flowChatCards.contextCompressionCard.duration')} {resolvedDuration < 1000 ? `${resolvedDuration}ms` : `${(resolvedDuration / 1000).toFixed(2)}s`}
              </span>
            )}
          </div>

          {resolvedTokensBefore !== undefined && resolvedTokensAfter !== undefined && (
            <div className="context-compression-card__simple-row context-compression-card__simple-row--stats">
              <span className="context-compression-card__simple-tokens">
                {resolvedTokensBefore?.toLocaleString()} → {resolvedTokensAfter?.toLocaleString()} tokens
              </span>
              {savedTokens !== undefined && resolvedCompressionRatio !== undefined && (
                <span className="context-compression-card__simple-savings">
                  {t('flowChatCards.contextCompressionCard.savedTokens', { count: savedTokens.toLocaleString(), ratio: (resolvedCompressionRatio * 100).toFixed(1) })}
                </span>
              )}
            </div>
          )}
        </>
      )}
    </BaseToolCard>
  );
};
