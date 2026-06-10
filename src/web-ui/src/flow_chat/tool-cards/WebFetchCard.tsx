import React, { useCallback, useMemo, useState } from 'react';
import { Globe, Link } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { ToolCardProps } from '../types/flow-chat';
import { systemAPI } from '../../infrastructure/api';
import { Tooltip } from '@/component-library';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';
import { createLogger } from '@/shared/utils/logger';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import './WebFetchCard.scss';

const log = createLogger('WebFetchCard');

interface ParsedWebFetchResult {
  url: string;
  format: string;
  content: string;
  contentLength: number | null;
}

function parseWebFetchResult(toolItem: ToolCardProps['toolItem']): ParsedWebFetchResult | null {
  const result = toolItem.toolResult?.result;
  const url = result?.url || toolItem.toolCall?.input?.url || '';
  const format = result?.format || toolItem.toolCall?.input?.format || 'text';
  const contentValue = result?.content ?? toolItem.toolResult?.resultForAssistant ?? '';
  const content = typeof contentValue === 'string'
    ? contentValue
    : contentValue == null
      ? ''
      : JSON.stringify(contentValue, null, 2);
  const contentLength = typeof result?.content_length === 'number'
    ? result.content_length
    : content.length > 0
      ? content.length
      : null;

  if (!url && !content) {
    return null;
  }

  return {
    url,
    format,
    content,
    contentLength,
  };
}

export const WebFetchCard: React.FC<ToolCardProps> = ({
  toolItem,
  onExpand,
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);
  const toolId = toolItem.id ?? toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const parsedResult = useMemo(() => parseWebFetchResult(toolItem), [toolItem]);
  const url = parsedResult?.url || toolCall?.input?.url || t('toolCards.webFetch.parsingUrl');
  const errorMessage = toolResult?.error || t('toolCards.webFetch.fetchFailed');
  const hasContent = Boolean(parsedResult?.content?.trim());
  const hasContentLength = parsedResult?.contentLength != null && parsedResult.contentLength > 0;
  const contentLength = hasContentLength ? parsedResult?.contentLength ?? undefined : undefined;
  const isExpandable = status === 'completed'
    ? Boolean(parsedResult?.url || hasContent)
    : status === 'error';
  const headerToolIcon = <Globe size={16} />;

  const handleOpenLink = async (linkUrl: string) => {
    if (!linkUrl || linkUrl === '#') return;

    try {
      await systemAPI.openExternal(linkUrl);
    } catch (error) {
      log.error('Failed to open external URL', { url: linkUrl, error });
    }
  };

  const handleClick = useCallback(() => {
    if (!isExpandable) return;

    applyExpandedState(isExpanded, !isExpanded, setIsExpanded, {
      onExpand,
    });
  }, [applyExpandedState, isExpandable, isExpanded, onExpand]);

  const renderContent = () => {
    if (status === 'completed') {
      const details: string[] = [];
      if (parsedResult?.format) {
        details.push(parsedResult.format);
      }
      if (contentLength != null) {
        details.push(t('toolCards.webFetch.contentLength', { count: contentLength }));
      } else if (hasContent) {
        details.push(t('toolCards.webFetch.contentAvailable'));
      }

      const suffix = details.length > 0 ? ` (${details.join(', ')})` : '';
      return `${t('toolCards.webFetch.readTitle', { url })}${suffix}`;
    }

    if (status === 'error') {
      return errorMessage;
    }

    if (status === 'running' || status === 'streaming' || status === 'preparing') {
      return t('toolCards.webFetch.reading', { url });
    }

    if (status === 'pending') {
      return t('toolCards.webFetch.preparingRead', { url });
    }

    return t('toolCards.webFetch.readTitle', { url });
  };

  const renderExpandedContent = () => {
    if (status === 'error') {
      return (
        <div className="compact-result-content web-fetch-card__content">
          <pre>{errorMessage}</pre>
        </div>
      );
    }

    return (
      <div className="web-fetch-card__expanded">
        {parsedResult?.url && (
          <div className="compact-expanded-result-item web-fetch-card__meta-card">
            <Tooltip content={t('toolCards.webFetch.clickToOpenLink')}>
              <div
                className="compact-expanded-result-title"
                onClick={(event) => {
                  event.stopPropagation();
                  void handleOpenLink(parsedResult.url);
                }}
              >
                <Link size={12} className="inline-icon" />
                {parsedResult.url}
              </div>
            </Tooltip>
          </div>
        )}

        <div className="compact-result-content web-fetch-card__content">
          <pre>{hasContent ? parsedResult?.content : t('toolCards.webFetch.noContent')}</pre>
        </div>
      </div>
    );
  };

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <CompactToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleClick}
        className="web-fetch-card"
        clickable={isExpandable}
        header={(
          <CompactToolCardHeader
            icon={(
              <ToolCardStatusSlot
                status={status}
                toolIcon={headerToolIcon}
                defaultIcon={status === 'completed' || status === 'error' ? 'tool' : 'status'}
              />
            )}
            content={renderContent()}
          />
        )}
        expandedContent={isExpandable ? renderExpandedContent() : undefined}
      />
    </div>
  );
};
