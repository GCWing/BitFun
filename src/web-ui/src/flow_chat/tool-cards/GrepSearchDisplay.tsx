/**
 * Tool card for GrepSearch text queries.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { GrepSearchToolCard } from '@openbitfun/ui/flow-chat';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { formatSessionViewPreviewText } from '../utils/sessionViewPreview';
export const GrepSearchDisplay: React.FC<ToolCardProps> = ({
  toolItem,
  onExpand
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);
  const toolId = toolItem.id ?? toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const getSearchPattern = (): string => {
    const pattern = toolCall?.input?.pattern || 
                   toolCall?.input?.search_pattern || 
                   toolCall?.input?.query ||
                   toolCall?.input?.text;
    
    if (!pattern) {
      const isEarlyDetection = toolCall?.input?._early_detection === true;
      const isPartialParams = toolCall?.input?._partial_params === true;
      
      if (isEarlyDetection || isPartialParams) {
        return t('toolCards.grepSearch.parsingPattern');
      }
      
      return t('toolCards.grepSearch.parsingPattern');
    }
    
    return pattern;
  };

  const getSearchPath = (): string => {
    return toolCall?.input?.path || t('toolCards.grepSearch.currentDirectory');
  };

  const stats = useMemo(() => {
    if (!toolResult?.result || typeof toolResult.result !== 'object') {
      return { matches: 0, files: 0 };
    }
    
    const fileCount = toolResult.result.file_count || 0;
    const totalMatches = toolResult.result.total_matches || 0;
    
    return {
      matches: totalMatches,
      files: fileCount
    };
  }, [toolResult]);

  const pattern = getSearchPattern();
  const searchPath = getSearchPath();
  const hasDetails = status === 'completed' && stats.matches > 0;
  const hasResultData = toolResult?.result !== undefined && toolResult?.result !== null;

  const handleClick = useCallback(() => {
    if (hasDetails) {
      applyExpandedState(isExpanded, !isExpanded, setIsExpanded, {
        onExpand,
      });
    }
  }, [applyExpandedState, hasDetails, isExpanded, onExpand]);

  const renderAction = () => {
    if (status === 'completed') {
      return `${t('toolCards.grepSearch.searchText')}:`;
    }
    if (status === 'running' || status === 'streaming') {
      return t('toolCards.grepSearch.searchingText');
    }
    if (status === 'pending') {
      return t('toolCards.grepSearch.preparingSearch');
    }
    return undefined;
  };

  const renderContent = () => {
    if (status === 'completed') {
      return `${pattern}${hasResultData ? ` (${t('toolCards.grepSearch.matchesCount', { count: stats.matches })})` : ''}`;
    }
    if (status === 'running' || status === 'streaming') {
      const progressMessage = (toolItem as any)._progressMessage;
      if (progressMessage) {
        return progressMessage;
      }
      return `${pattern}...`;
    }
    if (status === 'pending') {
      return pattern;
    }
    return pattern;
  };

  if (status === 'error') {
    return null;
  }

  return (
    <div ref={cardRootRef} data-openbitfun-adapter="grep-search" data-tool-card-id={toolId ?? ''}>
      <GrepSearchToolCard
        action={renderAction()}
        status={status}
        isExpanded={isExpanded}
        onToggle={hasDetails ? handleClick : undefined}
        summary={renderContent()}
        details={hasDetails ? [
          { label: `${t('toolCards.grepSearch.labelPattern')}:`, value: pattern },
          { label: `${t('toolCards.grepSearch.labelPath')}:`, value: searchPath },
          {
            label: `${t('toolCards.grepSearch.labelStats')}:`,
            value: t('toolCards.grepSearch.matchesAndFiles', { matches: stats.matches, files: stats.files }),
          },
        ] : undefined}
        resultText={hasDetails && toolResult?.result?.result
          ? formatSessionViewPreviewText(String(toolResult.result.result))
          : undefined}
      />
    </div>
  );
};
