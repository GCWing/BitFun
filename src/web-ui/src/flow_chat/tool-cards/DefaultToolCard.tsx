/**
 * Default tool card component
 * Used for tool types without specific customization
 */

import React, { useMemo, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { DefaultToolCard as DefaultToolCardView } from '@openbitfun/ui/flow-chat';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import {
  formatSessionViewPreviewText,
  isOnlySessionViewPreviewText,
} from '../utils/sessionViewPreview';

const MAX_PREVIEW_CHARS = 4000;
const INLINE_PREVIEW_CHARS = 72;
const INLINE_PREVIEW_SCAN_CHARS = 512;

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
      return formatSessionViewPreviewText(value);
    }

    return formatSessionViewPreviewText(JSON.stringify(value, null, 2));
  } catch {
    return formatSessionViewPreviewText(String(value));
  }
}

function truncatePreview(text: string, maxChars: number = MAX_PREVIEW_CHARS): string {
  if (text.length <= maxChars) return text;
  return `${text.slice(0, maxChars)}\n...`;
}

function getInlinePreview(value: any): string | null {
  if (value === null || value === undefined) return null;

  if (typeof value === 'string') {
    const scanned = value.length > INLINE_PREVIEW_SCAN_CHARS
      ? value.slice(0, INLINE_PREVIEW_SCAN_CHARS)
      : value;
    const normalized = scanned.replace(/\s+/g, ' ').trim();
    if (!normalized) return null;
    if (isOnlySessionViewPreviewText(normalized)) return null;
    return normalized.length > INLINE_PREVIEW_CHARS || scanned.length < value.length
      ? `${normalized.slice(0, INLINE_PREVIEW_CHARS)}...`
      : normalized;
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
  onExpand,
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
  const needsConfirmation = requiresConfirmation && !userConfirmed &&
    status !== 'completed' &&
    status !== 'cancelled' &&
    status !== 'rejected' &&
    status !== 'error';
  const canExpand = hasInput || hasResult || hasError;

  const inputPreview = useMemo(() => {
    if (!isExpanded || !hasInput) return null;
    return truncatePreview(stringifyValue(filteredInput));
  }, [filteredInput, hasInput, isExpanded]);

  const resultPreview = useMemo(() => {
    if (!isExpanded || !hasResult) return null;
    return truncatePreview(stringifyValue(toolResult?.result));
  }, [hasResult, isExpanded, toolResult?.result]);

  const handleToggleExpand = useCallback(() => {
    if (!canExpand) return;

    const nextExpanded = !isExpanded;
    applyExpandedState(isExpanded, nextExpanded, setIsExpanded, {
      onExpand,
    });
  }, [applyExpandedState, canExpand, isExpanded, onExpand]);

  const getStatusText = () => {
    if (needsConfirmation) {
      return t('toolCards.default.waitingConfirm');
    }

    const progressMessage = (toolItem as any)._progressMessage;
    if (progressMessage && (status === 'running' || status === 'streaming')) {
      return progressMessage;
    }

    switch (status) {
      case 'queued':
        return t('toolCards.default.queued');
      case 'waiting':
        return t('toolCards.default.waiting');
      case 'streaming':
      case 'running':
        return t('toolCards.default.executing');
      case 'completed':
        return t('toolCards.default.completed');
      case 'cancelled':
        return t('toolCards.default.cancelled');
      case 'rejected':
        return t('toolCards.default.rejected');
      case 'error':
        return t('toolCards.default.failed');
      default:
        return t('toolCards.default.preparing');
    }
  };

  const getSummaryText = () => {
    if (needsConfirmation) {
      const preview = getInlinePreview(filteredInput);
      return preview
        ? `${t('toolCards.default.waitingConfirm')} - ${preview}`
        : t('toolCards.default.waitingConfirm');
    }

    const progressMessage = (toolItem as any)._progressMessage;
    if (progressMessage && (status === 'running' || status === 'streaming')) {
      return progressMessage;
    }

    if (status === 'queued') {
      const preview = getInlinePreview(filteredInput);
      return preview
        ? `${t('toolCards.default.queued')} - ${preview}`
        : t('toolCards.default.queued');
    }

    if (status === 'waiting') {
      const preview = getInlinePreview(filteredInput);
      return preview
        ? `${t('toolCards.default.waiting')} - ${preview}`
        : t('toolCards.default.waiting');
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

    if (status === 'rejected') {
      return errorMessage || t('toolCards.default.rejected');
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

  const showConfirmationHighlight = needsConfirmation;

  return (
    <div data-openbitfun-adapter="default-tool-card" ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <DefaultToolCardView
        status={status}
        isExpanded={isExpanded}
        onToggle={canExpand ? handleToggleExpand : undefined}
        displayName={config.displayName}
        toolName={config.toolName}
        description={config.description}
        hasDetails={canExpand}
        icon={config.icon ?? undefined}
        summary={getSummaryText()}
        inputLabel={t('toolCards.common.inputParams')}
        inputPreview={inputPreview}
        resultLabel={t('toolCards.common.executionResult')}
        resultPreview={toolResult?.success === false ? undefined : resultPreview}
        error={errorMessage || undefined}
        requiresConfirmation={showConfirmationHighlight}
      />
    </div>
  );
};
