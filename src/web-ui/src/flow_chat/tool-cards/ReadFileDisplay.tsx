/**
 * Compact display for the read_file tool.
 */

import React, { useMemo } from 'react';
import { Check, FileText, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { IconButton } from '../../component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { AcpPermissionActions } from './AcpPermissionActions';
import { hasAcpPermissionOptions } from './AcpPermissionActions.utils';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { ToolCardHeaderActions } from './ToolCardHeaderActions';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';

export const ReadFileDisplay: React.FC<ToolCardProps> = React.memo(({
  toolItem,
  onConfirm,
  onReject,
  onOpenInEditor
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
  const showConfirmationActions = Boolean(
    requiresConfirmation &&
    !userConfirmed &&
    status !== 'completed' &&
    status !== 'cancelled' &&
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
    if (showConfirmationActions || status === 'pending_confirmation') {
      return (
        <>
          {t('toolCards.readFile.permissionRequest', { defaultValue: 'Requesting read permission:' })} {permissionTargetPath}
          {lineRange && <span className="read-file-meta"> {lineRange}</span>}
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

  const renderActions = () => {
    if (!showConfirmationActions) {
      return undefined;
    }

    return (
      <ToolCardHeaderActions>
        {hasAcpPermissionOptions(toolItem) ? (
          <AcpPermissionActions
            toolItem={toolItem}
            input={toolCall?.input}
            presentation="text"
            onConfirm={onConfirm}
            onReject={onReject}
          />
        ) : (
          <>
            <IconButton
              className="tool-card-header-action read-file-confirm-btn"
              variant="success"
              size="xs"
              onClick={(event) => {
                event.stopPropagation();
                onConfirm?.(toolCall?.input);
              }}
              tooltip={t('toolCards.default.waitingConfirm')}
            >
              <Check size={12} />
            </IconButton>
            <IconButton
              className="tool-card-header-action read-file-reject-btn"
              variant="danger"
              size="xs"
              onClick={(event) => {
                event.stopPropagation();
                onReject?.();
              }}
              tooltip={t('toolCards.acpPermission.reject')}
            >
              <X size={12} />
            </IconButton>
          </>
        )}
      </ToolCardHeaderActions>
    );
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
          icon={<ToolCardStatusSlot status={status} toolIcon={<FileText size={16} className="read-file-card-icon" />} />}
          content={renderContent()}
          extra={renderActions()}
        />
      }
    />
  );
});
