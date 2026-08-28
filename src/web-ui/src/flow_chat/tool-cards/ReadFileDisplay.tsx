/**
 * Compact display for the read_file tool.
 */

import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { ReadFileToolCard } from '@bitfun/ui/flow-chat';
import { isSessionViewPreviewText } from '../utils/sessionViewPreview';

export const ReadFileDisplay: React.FC<ToolCardProps> = React.memo(({
  toolItem,
  onOpenInEditor,
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status, requiresConfirmation, userConfirmed } = toolItem;

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

  const permissionTargetPath = useMemo(() => {
    const rawInput = toolItem.acpPermission?.toolCall?.rawInput as Record<string, unknown> | undefined;
    const acpFilePath =
      typeof rawInput?.filepath === 'string' && rawInput.filepath.trim().length > 0
        ? rawInput.filepath
        : typeof rawInput?.filePath === 'string' && rawInput.filePath.trim().length > 0
          ? rawInput.filePath
          : typeof rawInput?.parentDir === 'string' && rawInput.parentDir.trim().length > 0
            ? rawInput.parentDir
            : null;

    if (acpFilePath) {
      return acpFilePath;
    }

    return filePath;
  }, [filePath, toolItem.acpPermission?.toolCall?.rawInput]);

  const lineRange = useMemo(() => {
    // Keep legacy `start_line` so older persisted tool calls still render.
    const offset = toolCall?.input?.offset ?? toolCall?.input?.start_line;
    const tail = toolCall?.input?.tail === true;
    const limit = toolCall?.input?.limit;
    
    if (tail && limit !== undefined) {
      return `tail ${limit} lines`;
    }

    if (offset !== undefined || limit !== undefined) {
      const startLine = offset || 1;
      const endLine = limit ? startLine + limit - 1 : undefined;
      
      if (endLine) {
        return `L${startLine}~L${endLine}`;
      } else if (startLine > 1) {
        return `L${startLine}~EOF`;
      }
    }
    
    return null;
  }, [toolCall?.input?.offset, toolCall?.input?.start_line, toolCall?.input?.tail, toolCall?.input?.limit]);

  const fileSize = useMemo(() => {
    if (!toolResult?.result) return null;
    
    const content = toolResult.result.content || toolResult.result;
    if (typeof content === 'string') {
      if (isSessionViewPreviewText(content)) return null;
      const bytes = new TextEncoder().encode(content).length;
      if (bytes < 1024) return `${bytes}B`;
      if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
      return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
    }
    return null;
  }, [toolResult?.result]);

  const canOpenFile = status === 'completed' && filePath !== t('toolCards.readFile.noFileSpecified') && filePath !== t('toolCards.readFile.parsingParams');
  const showConfirmationActions = Boolean(
    requiresConfirmation &&
    !userConfirmed &&
    status !== 'completed' &&
    status !== 'cancelled' &&
    status !== 'rejected' &&
    status !== 'error'
  );

  if (status === 'error') {
    return null;
  }

  const renderContent = () => {
    if (status === 'completed') {
      return (
        <>
          {t('toolCards.readFile.readFile')}: {fileName}
          {lineRange && <> {lineRange}</>}
          {fileSize && <> ({fileSize})</>}
        </>
      );
    }
    if (status === 'running' || status === 'streaming') {
      return (
        <>
          {t('toolCards.readFile.readingFile')} {fileName}
          {lineRange && <> {lineRange}</>}
          ...
        </>
      );
    }
    if (showConfirmationActions || status === 'pending_confirmation') {
      return (
        <>
          {t('toolCards.readFile.permissionRequest')} {permissionTargetPath}
          {lineRange && <> {lineRange}</>}
        </>
      );
    }
    if (status === 'pending') {
      return (
        <>
          {t('toolCards.readFile.preparingRead')} {fileName}
          {lineRange && <> {lineRange}</>}
        </>
      );
    }
    return null;
  };

  return (
    <ReadFileToolCard
      status={status}
      interactive={canOpenFile}
      onOpen={canOpenFile ? handleOpenInEditor : undefined}
      summary={renderContent()}
    />
  );
});
