/**
 * Default tool card component
 * Used for tool types without specific customization
 */

import React, { useMemo, useState, useCallback } from 'react';
import { Loader2, XCircle, Clock, Check, ChevronDown, ChevronRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import './DefaultToolCard.scss';

const MAX_PREVIEW_CHARS = 4000;

function sanitizeToolInput(input: any): any {
  if (input === null || input === undefined) return input;
  if (Array.isArray(input)) return input;
  if (typeof input !== 'object') return input;

  return Object.entries(input).reduce((acc, [key, value]) => {
    if (!key.startsWith('_')) {
      acc[key] = value;
    }
    return acc;
  }, {} as Record<string, any>);
}

function hasVisibleValue(value: any): boolean {
  if (value === null || value === undefined) return false;
  if (typeof value === 'string') return value.trim().length > 0;
  if (Array.isArray(value)) return value.length > 0;
  if (typeof value === 'object') return Object.keys(value).length > 0;
  return true;
}

function stringifyValue(value: any): string {
  try {
    if (typeof value === 'string') {
      return value;
    }

    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function truncatePreview(text: string, maxChars: number = MAX_PREVIEW_CHARS): string {
  if (text.length <= maxChars) return text;
  return `${text.slice(0, maxChars)}\n...`;
}

function getInlinePreview(value: any): string | null {
  if (value === null || value === undefined) return null;

  if (typeof value === 'string') {
    const normalized = value.replace(/\s+/g, ' ').trim();
    if (!normalized) return null;
    return normalized.length > 72 ? `${normalized.slice(0, 72)}...` : normalized;
  }

  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }

  if (Array.isArray(value)) {
    return `Array(${value.length})`;
  }

  if (typeof value === 'object') {
    const entries = Object.entries(value).filter(([key]) => !key.startsWith('_'));
    if (entries.length === 0) return null;

    const [firstKey, firstValue] = entries[0];
    const nestedPreview = getInlinePreview(firstValue);
    return nestedPreview ? `${firstKey}: ${nestedPreview}` : `Object(${entries.length})`;
  }

  return String(value);
}

export const DefaultToolCard: React.FC<ToolCardProps> = ({
  toolItem,
  config,
  onConfirm,
  onReject,
  onExpand
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status, requiresConfirmation, userConfirmed } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);
  const toolId = toolItem.id ?? toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: config.toolName,
  });

  const filteredInput = useMemo(() => sanitizeToolInput(toolCall?.input), [toolCall?.input]);
  const hasInput = useMemo(() => hasVisibleValue(filteredInput), [filteredInput]);
  const hasResult = toolResult !== undefined && toolResult !== null && config.resultDisplayType !== 'hidden';
  const errorMessage = toolResult?.success === false ? toolResult.error || t('toolCards.default.failed') : null;
  const hasError = Boolean(errorMessage);
  const showConfirmationActions = requiresConfirmation && !userConfirmed &&
    status !== 'completed' &&
    status !== 'cancelled' &&
    status !== 'error';
  const canExpand = hasInput || hasResult || hasError || showConfirmationActions;

  const inputPreview = useMemo(() => {
    if (!hasInput) return null;
    return truncatePreview(stringifyValue(filteredInput));
  }, [filteredInput, hasInput]);

  const resultPreview = useMemo(() => {
    if (!hasResult) return null;
    return truncatePreview(stringifyValue(toolResult?.result));
  }, [hasResult, toolResult?.result]);

  const handleConfirm = () => {
    onConfirm?.(toolCall?.input);
  };

  const handleReject = () => {
    onReject?.();
  };

  const handleToggleExpand = useCallback(() => {
    if (!canExpand) return;

    const nextExpanded = !isExpanded;
    applyExpandedState(isExpanded, nextExpanded, setIsExpanded, {
      onExpand,
    });
  }, [applyExpandedState, canExpand, isExpanded, onExpand]);

  const getStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
        return <Loader2 className="animate-spin" size={16} />;
      case 'completed':
        return <Check size={16} className="icon-check-done" />;
      case 'cancelled':
      case 'error':
        return <XCircle size={16} />;
      default:
        return <Clock size={16} />;
    }
  };

  const getStatusText = () => {
    if (requiresConfirmation && !userConfirmed) {
      return t('toolCards.default.waitingConfirm');
    }

    const progressMessage = (toolItem as any)._progressMessage;
    if (progressMessage && (status === 'running' || status === 'streaming')) {
      return progressMessage;
    }

    switch (status) {
      case 'streaming':
      case 'running':
        return t('toolCards.default.executing');
      case 'completed':
        return t('toolCards.default.completed');
      case 'cancelled':
        return t('toolCards.default.cancelled');
      case 'error':
        return t('toolCards.default.failed');
      default:
        return t('toolCards.default.preparing');
    }
  };

  const getSummaryText = () => {
    if (requiresConfirmation && !userConfirmed) {
      const preview = getInlinePreview(filteredInput);
      return preview
        ? `${t('toolCards.default.waitingConfirm')} - ${preview}`
        : t('toolCards.default.waitingConfirm');
    }

    const progressMessage = (toolItem as any)._progressMessage;
    if (progressMessage && (status === 'running' || status === 'streaming')) {
      return progressMessage;
    }

    if (status === 'completed') {
      const preview = getInlinePreview(toolResult?.result) || getInlinePreview(filteredInput);
      return preview
        ? `${t('toolCards.default.completed')} - ${preview}`
        : t('toolCards.default.completed');
    }

    if (status === 'error') {
      return errorMessage || t('toolCards.default.failed');
    }

    if (status === 'running' || status === 'streaming') {
      const preview = getInlinePreview(filteredInput);
      return preview
        ? `${t('toolCards.default.executing')} - ${preview}`
        : t('toolCards.default.executing');
    }

    if (status === 'pending' || status === 'preparing') {
      const preview = getInlinePreview(filteredInput);
      return preview
        ? `${t('toolCards.default.preparing')} - ${preview}`
        : t('toolCards.default.preparing');
    }

    return getStatusText();
  };

  const showConfirmationHighlight = requiresConfirmation && !userConfirmed &&
    status !== 'completed' &&
    status !== 'cancelled' &&
    status !== 'error';

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <CompactToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleToggleExpand}
        className={`default-tool-card ${showConfirmationHighlight ? 'requires-confirmation' : ''}`}
        clickable={canExpand}
        header={
          <CompactToolCardHeader
            icon={getStatusIcon()}
            action={config.displayName}
            content={getSummaryText()}
            extra={config.icon ? <span className="default-tool-card__icon-badge">{config.icon}</span> : undefined}
            rightStatusIcon={canExpand ? (isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />) : undefined}
          />
        }
        expandedContent={canExpand ? (
          <div className="default-tool-card__expanded">
            <div className="default-tool-card__meta">
              <span className="default-tool-card__meta-label">{config.toolName}</span>
              {config.description && (
                <span className="default-tool-card__meta-description">{config.description}</span>
              )}
            </div>

          {hasInput && (
            <div className="default-tool-card__section">
              <div className="default-tool-card__section-label">{t('toolCards.common.inputParams')}</div>
              <pre className="default-tool-card__code-block">{inputPreview}</pre>
            </div>
          )}

          {showConfirmationActions && (
            <div className="default-tool-card__actions">
              <button
                type="button"
                className="default-tool-card__button default-tool-card__button--confirm"
                onClick={handleConfirm}
                disabled={status === 'streaming'}
              >
                {t('toolCards.mcp.confirmExecute')}
              </button>
              <button
                type="button"
                className="default-tool-card__button default-tool-card__button--reject"
                onClick={handleReject}
                disabled={status === 'streaming'}
              >
                {t('toolCards.mcp.cancel')}
              </button>
            </div>
          )}

          {hasResult && (
            <div className="default-tool-card__section">
              <div className="default-tool-card__section-label">{t('toolCards.common.executionResult')}</div>
              {toolResult?.success === false ? (
                <div className="default-tool-card__error-message">{errorMessage}</div>
              ) : (
                <pre className="default-tool-card__code-block">{resultPreview}</pre>
              )}
            </div>
          )}
          </div>
        ) : undefined}
      />
    </div>
  );
};


