/**
 * Display component for the LS tool.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { FolderOpen, File, Folder } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';
import { useToolCardHeightContract } from './useToolCardHeightContract';
interface LSEntry {
  name: string;
  path: string;
  is_dir: boolean;
  modified_time: string;
}

export const LSDisplay: React.FC<ToolCardProps> = ({
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

  const getDirectoryPath = (): string => {
    const path = toolCall?.input?.path;
    
    if (!path) {
      const isEarlyDetection = toolCall?.input?._early_detection === true;
      const isPartialParams = toolCall?.input?._partial_params === true;
      
      if (isEarlyDetection || isPartialParams) {
        return t('toolCards.ls.parsingPath');
      }
      
      return t('toolCards.ls.parsingPath');
    }
    
    return path;
  };

  const entries = useMemo((): LSEntry[] => {
    if (!toolResult?.result) return [];
    
    const parsedResult = toolResult.result;
    
    if (parsedResult.entries && Array.isArray(parsedResult.entries)) {
      return parsedResult.entries;
    }
    
    return [];
  }, [toolResult]);

  const stats = useMemo(() => {
    if (entries.length === 0) return { files: 0, directories: 0, total: 0 };
    
    let fileCount = 0;
    let dirCount = 0;
    
    entries.forEach((entry: LSEntry) => {
      if (entry.is_dir) {
        dirCount++;
      } else {
        fileCount++;
      }
    });
    
    return {
      files: fileCount,
      directories: dirCount,
      total: entries.length
    };
  }, [entries]);

  const directoryPath = getDirectoryPath();
  const hasDetails = status === 'completed' && entries.length > 0;
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
      const statsText = stats.directories > 0 
        ? t('toolCards.ls.filesAndDirs', { files: stats.files, directories: stats.directories })
        : t('toolCards.ls.filesCount', { count: stats.files });
      return `${t('toolCards.ls.listDirectory')}: ${directoryPath}${hasResultData ? ` (${statsText})` : ''}`;
    }
    if (status === 'running' || status === 'streaming') {
      return `${t('toolCards.ls.listingDirectory')} ${directoryPath}...`;
    }
    if (status === 'pending') {
      return `${t('toolCards.ls.preparingList')} ${directoryPath}`;
    }
    return directoryPath;
  };

  const renderExpandedContent = () => (
    <>
      <div className="compact-detail-info-inline">
        <span className="compact-detail-inline-item">
          <span className="compact-detail-inline-label">{t('toolCards.ls.labelPath')}:</span>
          <span className="compact-detail-inline-value">{directoryPath}</span>
        </span>
        <span className="compact-detail-inline-separator">|</span>
        <span className="compact-detail-inline-item">
          <span className="compact-detail-inline-label">{t('toolCards.ls.labelStats')}:</span>
          <span className="compact-detail-inline-value">
            {stats.directories > 0 
              ? t('toolCards.ls.filesAndDirs', { files: stats.files, directories: stats.directories })
              : t('toolCards.ls.filesCount', { count: stats.files })}
          </span>
        </span>
        <span className="compact-detail-inline-separator">|</span>
        <span className="compact-detail-inline-item">
          <span className="compact-detail-inline-label">{t('toolCards.ls.labelSort')}:</span>
          <span className="compact-detail-inline-value">{t('toolCards.ls.sortByModifiedTime')}</span>
        </span>
      </div>
      <div className="compact-detail-list" style={{ maxHeight: '400px', overflowY: 'auto' }}>
        {entries.slice(0, 50).map((entry: LSEntry, index: number) => (
          <div key={index} className="compact-list-item" style={{ 
            display: 'flex', 
            alignItems: 'center', 
            gap: '6px', 
            padding: '4px 0', 
            fontSize: '11px',
            color: 'var(--color-text-secondary)'
          }}>
            {entry.is_dir ? (
              <Folder size={12} style={{ flexShrink: 0, color: 'var(--color-text-muted)' }} />
            ) : (
              <File size={12} style={{ flexShrink: 0, color: 'var(--color-text-muted)' }} />
            )}
            <span style={{ flex: 1, fontFamily: 'var(--tool-card-font-mono)', wordBreak: 'break-all' }}>
              {entry.name}
            </span>
            <span style={{ color: 'var(--color-text-muted)', fontSize: '10px', flexShrink: 0 }}>
              {entry.modified_time}
            </span>
          </div>
        ))}
        {entries.length > 50 && (
          <div style={{ 
            textAlign: 'center', 
            padding: '8px 0', 
            color: 'var(--color-text-muted)', 
            fontSize: '11px', 
            fontStyle: 'italic' 
          }}>
            {t('toolCards.ls.moreEntries', { count: entries.length - 50 })}
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
        className="ls-display-card"
        clickable={hasDetails}
        header={
          <CompactToolCardHeader
            icon={<ToolCardStatusSlot status={status} toolIcon={<FolderOpen size={16} className="ls-display-card-icon" />} />}
            content={renderContent()}
          />
        }
        expandedContent={hasDetails ? renderExpandedContent() : undefined}
      />
    </div>
  );
};
