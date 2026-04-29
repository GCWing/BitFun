/**
 * Compact tool card for web_search.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { Globe, Loader2, Link, Clock, Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { systemAPI } from '../../infrastructure/api';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { Tooltip } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import { useToolCardHeightContract } from './useToolCardHeightContract';

const log = createLogger('WebSearchCard');

export const WebSearchCard: React.FC<ToolCardProps> = ({
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

  const getStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
      case 'preparing':
        return <Loader2 className="animate-spin" size={14} />;
      case 'completed':
        return <Check size={14} className="icon-check-done" />;
      default:
        return <Clock size={14} />;
    }
  };

  const getSearchTerm = () => {
    const searchTerm = toolCall?.input?.search_term || toolCall?.input?.query;
    
    if (!searchTerm) {
      return t('toolCards.webSearch.parsingSearchTerm');
    }
    
    return searchTerm;
  };

  const searchResults = useMemo(() => {
    if (!toolResult?.result) return null;
    
    const result = toolResult.result;
    
    if (result.results && Array.isArray(result.results)) {
      return {
        results: result.results,
        summary: result.summary || result.content,
        total: result.results.length
      };
    }
    
    if (result.content) {
      return {
        results: [],
        summary: result.content,
        total: 0
      };
    }
    
    return null;
  }, [toolResult]);

  const handleOpenLink = async (url: string) => {
    if (url && url !== '#') {
      try {
        await systemAPI.openExternal(url);
      } catch (error) {
        log.error('Failed to open external URL', { url, error });
      }
    }
  };

  const searchTerm = getSearchTerm();
  const hasResultData = toolResult?.result !== undefined && toolResult?.result !== null;
  const hasResults = searchResults && searchResults.results.length > 0;

  const handleClick = useCallback(() => {
    if (status === 'completed' && hasResults) {
      applyExpandedState(isExpanded, !isExpanded, setIsExpanded, {
        onExpand,
      });
    }
  }, [applyExpandedState, hasResults, isExpanded, onExpand, status]);

  const renderContent = () => {
    if (status === 'completed') {
      const resultsText = hasResultData && searchResults 
        ? ` (${t('toolCards.webSearch.resultsCount', { count: searchResults.total || 0 })})` 
        : '';
      return `${t('toolCards.webSearch.searchTitle', { term: searchTerm })}${resultsText}`;
    }
    if (status === 'running' || status === 'streaming' || status === 'preparing') {
      return t('toolCards.webSearch.searching', { term: searchTerm });
    }
    if (status === 'pending') {
      return t('toolCards.webSearch.preparingSearch', { term: searchTerm });
    }
    return t('toolCards.webSearch.searchTitle', { term: searchTerm });
  };

  const renderExpandedContent = () => (
    <div className="compact-expanded-results-list">
      {searchResults?.results.map((result: any, index: number) => (
        <div key={index} className="compact-expanded-result-item">
          <Tooltip content={t('toolCards.webSearch.clickToOpenLink')}>
            <div 
              className="compact-expanded-result-title"
              onClick={(e) => {
                e.stopPropagation();
                handleOpenLink(result.url);
              }}
            >
              <Link size={12} className="inline-icon" />
              {result.title || t('toolCards.webSearch.noTitle')}
            </div>
          </Tooltip>
          {result.snippet && (
            <div className="compact-expanded-result-snippet">{result.snippet}</div>
          )}
          <div className="compact-expanded-result-url">{result.url}</div>
        </div>
      ))}
    </div>
  );

  if (status === 'error') {
    return null;
  }

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <CompactToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleClick}
        clickable={Boolean(status === 'completed' && hasResults)}
        header={
          <CompactToolCardHeader
            icon={<Globe size={16} className="web-search-card-icon" />}
            content={renderContent()}
            rightStatusIcon={getStatusIcon()}
            rightStatusIconWithDivider
          />
        }
        expandedContent={hasResults ? renderExpandedContent() : undefined}
      />
    </div>
  );
};
