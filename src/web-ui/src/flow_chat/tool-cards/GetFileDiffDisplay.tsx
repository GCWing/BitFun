/**
 * Display component for the GetFileDiff tool.
 */

import React, { useMemo, useState, useCallback } from 'react';
import { GitCompare } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { CubeLoading } from '../../component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { InlineDiffPreview } from '../components/InlineDiffPreview';
import { createLogger } from '@/shared/utils/logger';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import './GetFileDiffDisplay.scss';

const log = createLogger('GetFileDiffDisplay');

interface GetFileDiffResult {
  file_path?: string;
  diff_type?: 'baseline' | 'full';
  diff_format?: string;
  diff_content?: string;
  original_content?: string;
  modified_content?: string;
  stats?: {
    additions?: number;
    deletions?: number;
    total_lines?: number;
  };
  message?: string;
}

export const GetFileDiffDisplay: React.FC<ToolCardProps> = React.memo(({
  toolItem,
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);
  const toolId = toolItem.id ?? toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const resultData = useMemo((): GetFileDiffResult | null => {
    if (!toolResult?.result) return null;
    
    try {
      if (typeof toolResult.result === 'string') {
        return JSON.parse(toolResult.result);
      }
      return toolResult.result as GetFileDiffResult;
    } catch (e) {
      log.error('Failed to parse GetFileDiff result', e);
      return null;
    }
  }, [toolResult]);

  const renderStatusIcon = () => {
    if (status === 'running' || status === 'streaming' || status === 'preparing') {
      return <CubeLoading size="small" />;
    }
    return null;
  };

  const filePath = useMemo(() => {
    if (resultData?.file_path) {
      return resultData.file_path;
    }
    const path = toolCall?.input?.file_path;
    
    if (!path) {
      const isEarlyDetection = toolCall?.input?._early_detection === true;
      const isPartialParams = toolCall?.input?._partial_params === true;
      
      if (isEarlyDetection || isPartialParams) {
        return t('toolCards.readFile.parsingParams');
      }
      
      return t('toolCards.readFile.parsingParams');
    }
    
    return path;
  }, [toolCall?.input, resultData, t]);

  const fileName = useMemo(() => {
    if (!filePath || filePath === t('toolCards.readFile.parsingParams')) {
      return filePath || '';
    }
    return filePath.split('/').pop() || filePath.split('\\').pop() || filePath;
  }, [filePath, t]);

  const diffTypeLabel = useMemo(() => {
    if (!resultData?.diff_type) return null;
    
    const typeMap: Record<string, string> = {
      'baseline': 'Baseline',
      'full': 'Full'
    };
    
    return typeMap[resultData.diff_type] || resultData.diff_type;
  }, [resultData]);

  const stats = useMemo(() => {
    return resultData?.stats || null;
  }, [resultData]);

  const hasDiffContent = useMemo(() => {
    return resultData && (resultData.original_content || resultData.modified_content || resultData.diff_content);
  }, [resultData]);

  const toggleExpanded = useCallback(() => {
    applyExpandedState(isExpanded, !isExpanded, setIsExpanded);
  }, [applyExpandedState, isExpanded]);

  const handleCardClick = useCallback(() => {
    if (hasDiffContent && status === 'completed') {
      toggleExpanded();
    }
  }, [hasDiffContent, status, toggleExpanded]);

  const renderToolIcon = () => {
    return <GitCompare size={16} />;
  };

  const isFailed = status === 'error';

  const getActionText = () => {
    if (isFailed) {
      return t('toolCards.getFileDiff.failed', { defaultValue: 'Diff failed' });
    }
    if (status === 'running' || status === 'streaming') {
      return t('toolCards.getFileDiff.gettingDiff', { defaultValue: 'Getting diff' });
    }
    if (status === 'pending' || status === 'preparing') {
      return t('toolCards.getFileDiff.preparing', { defaultValue: 'Preparing diff' });
    }
    return t('toolCards.getFileDiff.diffFile', { defaultValue: 'Diff' });
  };

  const renderHeader = () => (
    <ToolCardHeader
      icon={renderToolIcon()}
      iconClassName="diff-icon"
      action={`${getActionText()}:`}
      content={
        <span className="diff-tool-info">
          <span className="diff-file-name">{fileName}</span>
          {diffTypeLabel && status === 'completed' && (
            <span className="diff-type-tag">{diffTypeLabel}</span>
          )}
        </span>
      }
      extra={
        <>
          {!isFailed && status === 'completed' && stats && (stats.additions !== undefined || stats.deletions !== undefined) && (
            <span className="diff-stats">
              {stats.additions !== undefined && stats.additions > 0 && (
                <span className="additions">+{stats.additions}</span>
              )}
              {stats.deletions !== undefined && stats.deletions > 0 && (
                <span className="deletions">-{stats.deletions}</span>
              )}
            </span>
          )}
        </>
      }
      statusIcon={renderStatusIcon()}
    />
  );

  const renderExpandedContent = () => {
    if (!resultData) return null;

    const { original_content, modified_content, diff_content, diff_type } = resultData;

    if (diff_type === 'full' && modified_content) {
      return (
        <div className="diff-expanded-content">
          <div className="diff-message">{resultData.message}</div>
          <pre className="diff-content-preview">{modified_content}</pre>
        </div>
      );
    }

    if (original_content !== undefined && modified_content !== undefined) {
      return (
        <div className="diff-expanded-content">
          {resultData.message && (
            <div className="diff-message">{resultData.message}</div>
          )}
          <InlineDiffPreview
            originalContent={original_content}
            modifiedContent={modified_content}
            filePath={filePath}
            maxHeight={400}
            showLineNumbers={true}
            lineNumberMode="dual"
            showPrefix={true}
            contextLines={-1}
          />
        </div>
      );
    }

    if (diff_content) {
      return (
        <div className="diff-expanded-content">
          {resultData.message && (
            <div className="diff-message">{resultData.message}</div>
          )}
          <pre className="diff-content-preview">{diff_content}</pre>
        </div>
      );
    }

    return null;
  };

  const renderErrorContent = () => (
    <div className="error-content">
      <div className="error-message">
        {t('toolCards.getFileDiff.failed', { defaultValue: 'Failed to get file diff' })}
      </div>
    </div>
  );

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <BaseToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleCardClick}
        className="get-file-diff-card"
        header={renderHeader()}
        expandedContent={renderExpandedContent()}
        errorContent={isFailed ? renderErrorContent() : null}
        isFailed={isFailed}
        headerExpandAffordance={Boolean(hasDiffContent && status === 'completed')}
      />
    </div>
  );
});
