/**
 * Tool card for GlobSearch file matching.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { GlobSearchToolCard } from '@openbitfun/ui/flow-chat';
import { useToolCardHeightContract } from './useToolCardHeightContract';
export const GlobSearchDisplay: React.FC<ToolCardProps> = ({
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
                   toolCall?.input?.glob_pattern || 
                   toolCall?.input?.file_pattern;
    
    if (!pattern) {
      const isEarlyDetection = toolCall?.input?._early_detection === true;
      const isPartialParams = toolCall?.input?._partial_params === true;
      
      if (isEarlyDetection || isPartialParams) {
        return t('toolCards.globSearch.parsingPattern');
      }
      
      return t('toolCards.globSearch.parsingPattern');
    }
    
    return pattern;
  };

  const getSearchPath = (): string => {
    return toolCall?.input?.path || toolCall?.input?.target_directory || t('toolCards.globSearch.currentDirectory');
  };

  const files = useMemo(() => {
    if (!toolResult?.result) return [];
    
    const parsedResult = toolResult.result;
    
    if (Array.isArray(parsedResult)) {
      return parsedResult;
    }
    if (parsedResult.files && Array.isArray(parsedResult.files)) {
      return parsedResult.files;
    }
    if (parsedResult.matches && Array.isArray(parsedResult.matches)) {
      return parsedResult.matches;
    }
    
    return [];
  }, [toolResult]);

  const stats = useMemo(() => {
    if (files.length === 0) return { files: 0, directories: 0 };
    
    let fileCount = 0;
    let dirCount = 0;
    
    files.forEach((file: any) => {
      const fileName = typeof file === 'string' ? file : (file.name || file.path || '');
      if (fileName.includes('/') && fileName.endsWith('/')) {
        dirCount++;
      } else {
        fileCount++;
      }
    });
    
    return {
      files: fileCount,
      directories: dirCount
    };
  }, [files]);

  const pattern = getSearchPattern();
  const searchPath = getSearchPath();
  const hasDetails = status === 'completed' && files.length > 0;
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
      return `${t('toolCards.globSearch.searchFile')}:`;
    }
    if (status === 'running' || status === 'streaming') {
      return t('toolCards.globSearch.searchingFile');
    }
    if (status === 'pending') {
      return t('toolCards.globSearch.preparingSearch');
    }
    return undefined;
  };

  const renderContent = () => {
    if (status === 'completed') {
      return `${pattern}${hasResultData ? ` (${t('toolCards.globSearch.filesCount', { count: stats.files })})` : ''}`;
    }
    if (status === 'running' || status === 'streaming') {
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
    <div ref={cardRootRef} data-openbitfun-adapter="glob-search" data-tool-card-id={toolId ?? ''}>
      <GlobSearchToolCard
        action={renderAction()}
        status={status}
        isExpanded={isExpanded}
        onToggle={hasDetails ? handleClick : undefined}
        summary={renderContent()}
        details={hasDetails ? [
          { label: `${t('toolCards.globSearch.labelPattern')}:`, value: pattern },
          { label: `${t('toolCards.globSearch.labelPath')}:`, value: searchPath },
          {
            label: `${t('toolCards.globSearch.labelStats')}:`,
            value: stats.directories > 0
              ? t('toolCards.globSearch.filesAndDirs', { files: stats.files, directories: stats.directories })
              : t('toolCards.globSearch.filesCount', { count: stats.files }),
          },
        ] : undefined}
        results={hasDetails ? files.slice(0, 50).map((file: any, index: number) => {
          const fileName = typeof file === 'string' ? file : (file.name || file.path || '');
          return {
            icon: fileName.endsWith('/') ? 'directory' as const : 'file' as const,
            key: `${fileName}-${index}`,
            title: fileName,
          };
        }) : undefined}
        moreResultsLabel={files.length > 50
          ? t('toolCards.globSearch.moreFiles', { count: files.length - 50 })
          : undefined}
      />
    </div>
  );
};
