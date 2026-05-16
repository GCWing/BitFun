/**
 * Tool card for GlobSearch file matching.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { FolderSearch, File, Folder } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';
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

  const renderContent = () => {
    if (status === 'completed') {
      return `${t('toolCards.globSearch.searchFile')}: ${pattern}${hasResultData ? ` (${t('toolCards.globSearch.filesCount', { count: stats.files })})` : ''}`;
    }
    if (status === 'running' || status === 'streaming') {
      return `${t('toolCards.globSearch.searchingFile')} ${pattern}...`;
    }
    if (status === 'pending') {
      return `${t('toolCards.globSearch.preparingSearch')} ${pattern}`;
    }
    return pattern;
  };

  const renderExpandedContent = () => (
    <>
      <div className="compact-detail-info-inline">
        <span className="compact-detail-inline-item">
          <span className="compact-detail-inline-label">{t('toolCards.globSearch.labelPattern')}:</span>
          <span className="compact-detail-inline-value">{pattern}</span>
        </span>
        <span className="compact-detail-inline-separator">|</span>
        <span className="compact-detail-inline-item">
          <span className="compact-detail-inline-label">{t('toolCards.globSearch.labelPath')}:</span>
          <span className="compact-detail-inline-value">{searchPath}</span>
        </span>
        <span className="compact-detail-inline-separator">|</span>
        <span className="compact-detail-inline-item">
          <span className="compact-detail-inline-label">{t('toolCards.globSearch.labelStats')}:</span>
          <span className="compact-detail-inline-value">
            {stats.directories > 0 
              ? t('toolCards.globSearch.filesAndDirs', { files: stats.files, directories: stats.directories })
              : t('toolCards.globSearch.filesCount', { count: stats.files })}
          </span>
        </span>
      </div>
      <div className="compact-detail-list" style={{ maxHeight: '400px', overflowY: 'auto' }}>
        {files.slice(0, 50).map((file: any, index: number) => {
          const fileName = typeof file === 'string' ? file : (file.name || file.path || '');
          const isDir = fileName.endsWith('/');
          return (
            <div key={index} className="compact-list-item" style={{ 
              display: 'flex', 
              alignItems: 'center', 
              gap: '6px', 
              padding: '4px 0', 
              fontSize: '11px',
              color: 'var(--color-text-secondary)'
            }}>
              {isDir ? (
                <Folder size={12} style={{ flexShrink: 0, color: 'var(--color-text-muted)' }} />
              ) : (
                <File size={12} style={{ flexShrink: 0, color: 'var(--color-text-muted)' }} />
              )}
              <span style={{ flex: 1, fontFamily: 'var(--tool-card-font-mono)', wordBreak: 'break-all' }}>
                {fileName}
              </span>
            </div>
          );
        })}
        {files.length > 50 && (
          <div style={{ 
            textAlign: 'center', 
            padding: '8px 0', 
            color: 'var(--color-text-muted)', 
            fontSize: '11px', 
            fontStyle: 'italic' 
          }}>
            {t('toolCards.globSearch.moreFiles', { count: files.length - 50 })}
          </div>
        )}
      </div>
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
        className="glob-search-card"
        clickable={hasDetails}
        header={
          <CompactToolCardHeader
            icon={<ToolCardStatusSlot status={status} toolIcon={<FolderSearch size={16} className="glob-search-card-icon" />} />}
            content={renderContent()}
          />
        }
        expandedContent={hasDetails ? renderExpandedContent() : undefined}
      />
    </div>
  );
};
