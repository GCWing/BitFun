/**
 * Compact display for the read_file tool.
 */

import React, { useMemo } from 'react';
import { FileText, Loader2, Clock, Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';

export const ReadFileDisplay: React.FC<ToolCardProps> = React.memo(({
  toolItem,
  onOpenInEditor
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;

  const getStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
        return <Loader2 className="animate-spin" size={14} />;
      case 'completed':
        return <Check size={14} className="icon-check-done" />;
      case 'pending':
      default:
        return <Clock size={14} />;
    }
  };

  const filePath = useMemo(() => {
    const path = toolCall?.input?.file_path || toolCall?.input?.target_file || toolCall?.input?.path;
    
    if (!path) {
      const isEarlyDetection = toolCall?.input?._early_detection === true;
      const isPartialParams = toolCall?.input?._partial_params === true;
      
      if (isEarlyDetection || isPartialParams) {
        return t('toolCards.readFile.parsingParams');
      }
      
      return t('toolCards.readFile.parsingParams');
    }
    
    return path;
  }, [t, toolCall?.input]);

  const handleOpenInEditor = () => {
    if (filePath !== t('toolCards.readFile.noFileSpecified') && filePath !== t('toolCards.readFile.parsingParams')) {
      onOpenInEditor?.(filePath);
    }
  };

  const fileName = useMemo(() => {
    if (!filePath || filePath === t('toolCards.readFile.noFileSpecified') || filePath === t('toolCards.readFile.parsingParams')) {
      return filePath || t('toolCards.readFile.noFileSpecified');
    }
    return filePath.split('/').pop() || filePath.split('\\').pop() || filePath;
  }, [filePath, t]);

  const lineRange = useMemo(() => {
    const start_line = toolCall?.input?.start_line;
    const limit = toolCall?.input?.limit;
    
    if (start_line !== undefined || limit !== undefined) {
      const startLine = start_line || 1;
      const endLine = limit ? startLine + limit - 1 : undefined;
      
      if (endLine) {
        return `L${startLine}~L${endLine}`;
      } else if (startLine > 1) {
        return `L${startLine}~EOF`;
      }
    }
    
    return null;
  }, [toolCall?.input?.start_line, toolCall?.input?.limit]);

  const fileSize = useMemo(() => {
    if (!toolResult?.result) return null;
    
    const content = toolResult.result.content || toolResult.result;
    if (typeof content === 'string') {
      const bytes = new TextEncoder().encode(content).length;
      if (bytes < 1024) return `${bytes}B`;
      if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
      return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
    }
    return null;
  }, [toolResult?.result]);

  const canOpenFile = status === 'completed' && filePath !== t('toolCards.readFile.noFileSpecified') && filePath !== t('toolCards.readFile.parsingParams');

  if (status === 'error') {
    return null;
  }

  const renderContent = () => {
    if (status === 'completed') {
      return (
        <>
          {t('toolCards.readFile.readFile')}: {fileName}
          {lineRange && <span className="read-file-meta"> {lineRange}</span>}
          {fileSize && <span className="read-file-meta"> ({fileSize})</span>}
        </>
      );
    }
    if (status === 'running' || status === 'streaming') {
      return (
        <>
          {t('toolCards.readFile.readingFile')} {fileName}
          {lineRange && <span className="read-file-meta"> {lineRange}</span>}
          ...
        </>
      );
    }
    if (status === 'pending') {
      return (
        <>
          {t('toolCards.readFile.preparingRead')} {fileName}
          {lineRange && <span className="read-file-meta"> {lineRange}</span>}
        </>
      );
    }
    return null;
  };

  return (
    <CompactToolCard
      status={status}
      isExpanded={false}
      onClick={() => canOpenFile && handleOpenInEditor()}
      className="read-file-card"
      clickable={canOpenFile}
      header={
        <CompactToolCardHeader
          icon={<FileText size={16} className="read-file-card-icon" />}
          content={renderContent()}
          rightStatusIcon={getStatusIcon()}
          rightStatusIconWithDivider
        />
      }
    />
  );
});
