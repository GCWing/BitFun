import React, { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { ToolCardProps } from '../types/flow-chat';
import { systemAPI } from '../../infrastructure/api';
import { WebFetchToolCard as WebFetchToolCardView } from '@openbitfun/ui/flow-chat';
import { ToolCardCopyAction } from './ToolCardCopyAction';
import { createLogger } from '@/shared/utils/logger';
import { useToolCardHeightContract } from './useToolCardHeightContract';

const log = createLogger('WebFetchCard');

interface ParsedWebFetchResult {
  title: string | null;
  url: string;
  format: string;
  content: string;
  contentLength: number | null;
}

function parseWebFetchResult(toolItem: ToolCardProps['toolItem']): ParsedWebFetchResult | null {
  const result = toolItem.toolResult?.result;
  const title = typeof result?.title === 'string' && result.title.trim().length > 0
    ? result.title
    : null;
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
    title,
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
  const headerTitle = parsedResult?.title?.trim() || '';
  const errorMessage = toolResult?.error || t('toolCards.webFetch.fetchFailed');
  const hasContent = Boolean(parsedResult?.content?.trim());
  const hasContentLength = parsedResult?.contentLength != null && parsedResult.contentLength > 0;
  const contentLength = hasContentLength ? parsedResult?.contentLength ?? undefined : undefined;
  const isExpandable = status === 'completed'
    ? Boolean(parsedResult?.url || hasContent)
    : status === 'error';
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

  const getDetails = () => {
    const details: string[] = [];
    if (parsedResult?.format) {
      details.push(parsedResult.format);
    }
    if (contentLength != null) {
      details.push(t('toolCards.webFetch.contentLength', { count: contentLength }));
    } else if (hasContent) {
      details.push(t('toolCards.webFetch.contentAvailable'));
    }

    return details;
  };

  const renderContent = () => {
    if (status === 'completed') {
      return headerTitle || `"${url}"`;
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

  const renderAction = () => (
    status === 'completed'
      ? t('toolCards.webFetch.readLabel')
      : undefined
  );

  return (
    <div data-openbitfun-adapter="web-fetch" ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <WebFetchToolCardView
        status={status}
        isExpanded={isExpanded}
        onToggle={isExpandable ? handleClick : undefined}
        action={renderAction()}
        title={renderContent()}
        url={status === 'completed' ? parsedResult?.url : undefined}
        openUrlLabel={t('toolCards.webFetch.clickToOpenLink')}
        onOpenUrl={parsedResult?.url ? (event) => {
          event.stopPropagation();
          void handleOpenLink(parsedResult.url);
        } : undefined}
        details={status === 'completed' ? getDetails() : undefined}
        content={status === 'completed' && hasContent ? parsedResult?.content : undefined}
        emptyContent={status === 'completed' && !hasContent ? t('toolCards.webFetch.noContent') : undefined}
        error={status === 'error' ? errorMessage : undefined}
        copyAction={hasContent ? (
          <ToolCardCopyAction
            getText={() => parsedResult?.content ?? ''}
            tooltip={t('toolCards.webFetch.copyResult')}
            copiedTooltip={t('toolCards.webFetch.resultCopied')}
            successMessage={t('toolCards.webFetch.resultCopied')}
            failureMessage={t('toolCards.webFetch.copyResultFailed')}
            ariaLabel={t('toolCards.webFetch.copyResult')}
            showSuccessNotification={false}
          />
        ) : undefined}
      />
    </div>
  );
};
