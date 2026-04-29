/**
 * Tool card for GrepSearch text queries.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { Search, Loader2, Clock, Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { useToolCardHeightContract } from './useToolCardHeightContract';
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

  const getStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
        return <Loader2 className="animate-spin" size={14} />;
      case 'completed':
        return <Check size={14} className="icon-check-done" />;
      default:
        return <Clock size={14} />;
    }
  };

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

  const renderContent = () => {
    if (status === 'completed') {
      return `${t('toolCards.grepSearch.searchText')}: ${pattern}${hasResultData ? ` (${t('toolCards.grepSearch.matchesCount', { count: stats.matches })})` : ''}`;
    }
    if (status === 'running' || status === 'streaming') {
      const progressMessage = (toolItem as any)._progressMessage;
      if (progressMessage) {
        return progressMessage;
      }
      return `${t('toolCards.grepSearch.searchingText')} ${pattern}...`;
    }
    if (status === 'pending') {
      return `${t('toolCards.grepSearch.preparingSearch')} ${pattern}`;
    }
    return pattern;
  };

  const renderExpandedContent = () => (
    <>
      <div className="compact-detail-info-inline">
        <span className="compact-detail-inline-item">
          <span className="compact-detail-inline-label">{t('toolCards.grepSearch.labelPattern')}:</span>
          <span className="compact-detail-inline-value">{pattern}</span>
        </span>
        <span className="compact-detail-inline-separator">|</span>
        <span className="compact-detail-inline-item">
          <span className="compact-detail-inline-label">{t('toolCards.grepSearch.labelPath')}:</span>
          <span className="compact-detail-inline-value">{searchPath}</span>
        </span>
        <span className="compact-detail-inline-separator">|</span>
        <span className="compact-detail-inline-item">
          <span className="compact-detail-inline-label">{t('toolCards.grepSearch.labelStats')}:</span>
          <span className="compact-detail-inline-value">
            {t('toolCards.grepSearch.matchesAndFiles', { matches: stats.matches, files: stats.files })}
          </span>
        </span>
      </div>
      {toolResult?.result?.result && (
        <div className="compact-result-content">
          <pre style={{ 
            whiteSpace: 'pre-wrap', 
            wordBreak: 'break-word',
            fontSize: '12px',
            maxHeight: '400px',
            overflow: 'auto'
          }}>
            {toolResult.result.result}
          </pre>
        </div>
      )}
    </>
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
        className="grep-search-card"
        clickable={hasDetails}
        header={
          <CompactToolCardHeader
            icon={<Search size={16} className="grep-search-card-icon" />}
            content={renderContent()}
            rightStatusIcon={getStatusIcon()}
            rightStatusIconWithDivider
          />
        }
        expandedContent={hasDetails ? renderExpandedContent() : undefined}
      />
    </div>
  );
};
