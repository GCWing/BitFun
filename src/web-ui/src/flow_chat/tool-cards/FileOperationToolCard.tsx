/**
 * File operation tool card - refactored based on BaseToolCard
 * Supports Write/Edit/Delete file operations
 */

import React, { useEffect, useCallback, useMemo, useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { XCircle, GitBranch, FileText, ChevronDown, ChevronUp, FileEdit, FilePlus, Trash2, Loader2, Clock, Check } from 'lucide-react';
import { CubeLoading } from '../../component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { useSnapshotState } from '../../tools/snapshot_system/hooks/useSnapshotState';
import { SnapshotEventBus, SNAPSHOT_EVENTS } from '../../tools/snapshot_system/core/SnapshotEventBus';
import { useCurrentWorkspace } from '../../infrastructure/contexts/WorkspaceContext';
import { createCodeEditorTab, createDiffEditorTab } from '../../shared/utils/tabUtils';
import { CodePreview } from '../components/CodePreview';
import { InlineDiffPreview } from '../components/InlineDiffPreview';
import { Tooltip } from '@/component-library';
import { diffLines } from 'diff';
import { createLogger } from '@/shared/utils/logger';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import './FileOperationToolCard.scss';

const log = createLogger('FileOperationToolCard');

interface FileOperationToolCardProps extends ToolCardProps {
  sessionId?: string;
}

export const FileOperationToolCard: React.FC<FileOperationToolCardProps> = ({
  toolItem,
  config,
  sessionId,
  onOpenInEditor
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status, isParamsStreaming, partialParams } = toolItem;
  
  const [isErrorExpanded, setIsErrorExpanded] = useState(false);
  const [isPreviewExpanded, setIsPreviewExpanded] = useState(isParamsStreaming);
  const [operationDiffStats, setOperationDiffStats] = useState<{ additions: number; deletions: number } | null>(null);
  
  const prevIsParamsStreamingRef = useRef(isParamsStreaming);
  const userCollapsedRef = useRef(false);
  const hasInitializedCompletionEffectRef = useRef(false);
  const previousCompletionEndTimeRef = useRef<number | null>(toolItem.endTime ?? null);
  
  useEffect(() => {
    const prevIsParamsStreaming = prevIsParamsStreamingRef.current;
    
    if (prevIsParamsStreaming !== isParamsStreaming) {
      prevIsParamsStreamingRef.current = isParamsStreaming;
      
      if (isParamsStreaming) {
        userCollapsedRef.current = false;
        setIsPreviewExpanded(true);
      } else {
        setIsPreviewExpanded(false);
      }
    }
  }, [isParamsStreaming]);
  
  const {
    files,
    error,
    clearError
  } = useSnapshotState(sessionId);
  const eventBus = SnapshotEventBus.getInstance();
  const { workspace: currentWorkspace } = useCurrentWorkspace();

  const getFilePath = useCallback((): string => {
    const params = partialParams || toolCall?.input;
    if (!params) return '';
    
    if (Object.keys(params).length === 0) return '';
    
    return params.file_path || params.target_file || params.path || params.filename || '';
  }, [toolCall, partialParams]);

  const currentFilePath = getFilePath();
  
  const getOldString = useCallback((): string => {
    const params = partialParams || toolCall?.input;
    if (!params) return '';
    return params.old_string || '';
  }, [toolCall, partialParams]);

  const getNewString = useCallback((): string => {
    const params = partialParams || toolCall?.input;
    if (!params) return '';
    return params.new_string || '';
  }, [toolCall, partialParams]);

  const getContent = useCallback((): string => {
    const params = partialParams || toolCall?.input;
    if (!params) return '';
    return params.content || params.contents || '';
  }, [toolCall, partialParams]);

  const oldStringContent = getOldString();
  const newStringContent = getNewString();
  const contentPreview = getContent();
  
  const isFailed = status === 'error' || (toolResult && 'success' in toolResult && !toolResult.success);
  
  const fileName = currentFilePath ? 
    (currentFilePath.split(/[/\\]/).pop() || t('context.file')) : 
    (isFailed ? t('toolCards.file.unknownFile') : t('toolCards.file.parsingPath'));
  
  const currentFile = files.find(f => f.filePath === currentFilePath);

  useEffect(() => {
    const completionEndTime = toolItem.endTime ?? null;
    const isCompletedSuccess = status === 'completed' && Boolean(toolResult?.success);

    if (!hasInitializedCompletionEffectRef.current) {
      hasInitializedCompletionEffectRef.current = true;
      previousCompletionEndTimeRef.current = completionEndTime;
      return;
    }

    const shouldEmitCompletionEvent =
      isCompletedSuccess &&
      completionEndTime !== null &&
      previousCompletionEndTimeRef.current !== completionEndTime &&
      Boolean(sessionId) &&
      Boolean(currentFilePath);

    previousCompletionEndTimeRef.current = completionEndTime;

    if (!shouldEmitCompletionEvent || !sessionId || !currentFilePath) {
      return;
    }

    eventBus.emit(SNAPSHOT_EVENTS.FILE_OPERATION_COMPLETED, {
      toolName: toolItem.toolName,
      toolResult
    }, sessionId, currentFilePath);
  }, [status, toolResult, sessionId, currentFilePath, toolItem.toolName, toolItem.endTime, eventBus]);

  const getToolDisplayInfo = () => {
    const toolMap: Record<string, { icon: string; name: string }> = {
      'Write': { icon: '', name: t('toolCards.file.write') },
      'Edit': { icon: '', name: t('toolCards.file.edit') },
      'Delete': { icon: '', name: t('toolCards.file.delete') }
    };
    
    return toolMap[toolItem.toolName] || { icon: config.icon, name: config.displayName };
  };

  const toolDisplayInfo = getToolDisplayInfo();

  useEffect(() => {
    if (error) {
      log.error('File operation error', { filePath: currentFilePath, error });
      setTimeout(clearError, 3000);
    }
  }, [error, clearError, currentFilePath]);

  const localDiffStats = useMemo(() => {
    if (status !== 'completed' || isFailed) return null;
    if (toolItem.toolName === 'Write' && contentPreview) {
      const lines = contentPreview.split('\n');
      const count = lines[lines.length - 1] === '' ? lines.length - 1 : lines.length;
      return { additions: count, deletions: 0 };
    }
    if (toolItem.toolName === 'Edit' && (oldStringContent || newStringContent)) {
      const changes = diffLines(oldStringContent, newStringContent);
      let additions = 0;
      let deletions = 0;
      for (const change of changes) {
        const lineCount = change.count ?? 0;
        if (change.added) additions += lineCount;
        else if (change.removed) deletions += lineCount;
      }
      return { additions, deletions };
    }
    return null;
  }, [toolItem.toolName, contentPreview, oldStringContent, newStringContent, status, isFailed]);

  const currentFileDiffStats = useMemo(() => {
    return operationDiffStats ?? localDiffStats ?? { additions: 0, deletions: 0 };
  }, [operationDiffStats, localDiffStats]);

  useEffect(() => {
    if (!sessionId || !toolCall?.id || status !== 'completed' || isFailed) return;
    let cancelled = false;

    (async () => {
      try {
        const { snapshotAPI } = await import('../../infrastructure/api');
        const summary = await snapshotAPI.getOperationSummary(sessionId, toolCall.id);
        if (cancelled) return;
        setOperationDiffStats({
          additions: summary.linesAdded ? Number(summary.linesAdded) : 0,
          deletions: summary.linesRemoved ? Number(summary.linesRemoved) : 0
        });
      } catch (error) {
        log.warn('Failed to load operation summary', { sessionId, toolCallId: toolCall.id, error });
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [sessionId, toolCall?.id, status, isFailed]);

  const isLoading = status === 'preparing' || status === 'streaming' || status === 'running';
  
  const getErrorMessage = () => {
    if (toolResult && 'error' in toolResult) {
      return toolResult.error;
    }
    if (error) {
      return error;
    }
    return t('error.unknown');
  };

  const handleOpenInCodeEditor = useCallback(async () => {
    if (!sessionId || !currentFilePath) return;

    try {
      const { snapshotAPI } = await import('../../infrastructure/api');
      const diffData = await snapshotAPI.getOperationDiff(sessionId, currentFilePath, toolCall?.id);
      const jumpToLine = diffData.anchorLine ? Number(diffData.anchorLine) : undefined;

      window.dispatchEvent(new CustomEvent('expand-right-panel'));

      setTimeout(() => {
        if (toolItem.toolName === 'Delete') {
          createDiffEditorTab(
            currentFilePath,
            fileName,
            diffData.originalContent || '',
            diffData.modifiedContent || '',
            true,
            'agent',
            undefined,
            jumpToLine
          );
          return;
        }

        createCodeEditorTab(
          currentFilePath,
          fileName,
          {
            readOnly: false,
            showLineNumbers: true,
            showMinimap: true,
            theme: 'vs-dark',
            jumpToLine
          },
          'agent'
        );
      }, 250);
    } catch (error) {
      log.error('Failed to open in CodeEditor', { sessionId, filePath: currentFilePath, error });
      window.dispatchEvent(new CustomEvent('expand-right-panel'));
      setTimeout(() => {
        if (toolItem.toolName === 'Delete') {
          createDiffEditorTab(
            currentFilePath,
            fileName,
            '',
            '',
            true,
            'agent'
          );
          return;
        }

        createCodeEditorTab(currentFilePath, fileName, { theme: 'vs-dark' }, 'agent');
      }, 250);
    }
  }, [sessionId, currentFilePath, toolCall?.id, fileName, toolItem.toolName]);

  const handleCardClick = useCallback((e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest('.compact-actions')) {
      return;
    }
    
    if (isFailed) {
      setIsErrorExpanded(!isErrorExpanded);
      return;
    }

    if (toolItem.toolName === 'Delete') {
      return;
    }
    
    if (currentFilePath && sessionId && status === 'completed') {
      handleOpenInCodeEditor();
      return;
    }

    if (currentFilePath && onOpenInEditor) {
      onOpenInEditor(currentFilePath);
    }
  }, [currentFilePath, onOpenInEditor, isFailed, isErrorExpanded, sessionId, status, handleOpenInCodeEditor, toolItem.toolName]);

  const handleOpenBaselineDiff = useCallback(async () => {
    if (!currentFile || !currentWorkspace) {
      log.warn('Cannot open Baseline Diff: missing required info', { hasFile: !!currentFile, hasWorkspace: !!currentWorkspace });
      return;
    }

    const fileName = currentFile.filePath.split(/[/\\]/).pop() || currentFile.filePath;

    try {
      const { snapshotAPI } = await import('../../infrastructure/api');
      
      const diffData = await snapshotAPI.getBaselineSnapshotDiff(
        currentFile.filePath,
        currentWorkspace.rootPath
      );

      window.dispatchEvent(new CustomEvent('expand-right-panel'));

      setTimeout(() => {
        createDiffEditorTab(
          currentFile.filePath,
          fileName,
          diffData.originalContent || '',
          diffData.modifiedContent || '',
          false,
          'agent',
          currentWorkspace.rootPath
        );
      }, 250);
    } catch (error) {
      log.error('Failed to open Baseline Diff', { filePath: currentFile?.filePath, error });
    }
  }, [currentFile, currentWorkspace]);

  const getToolIconInfo = () => {
    const iconMap: Record<string, { icon: React.ReactNode; className: string }> = {
      'Write': { icon: <FilePlus size={16} />, className: 'write-icon' },
      'Edit': { icon: <FileEdit size={16} />, className: 'edit-icon' },
      'Delete': { icon: <Trash2 size={16} />, className: 'delete-icon' }
    };
    
    return iconMap[toolItem.toolName] || { icon: <FileText size={16} />, className: 'file-icon' };
  };

  const renderToolIcon = () => {
    const { icon } = getToolIconInfo();
    return icon;
  };

  const renderStatusIcon = () => {
    if (isLoading) {
      return <CubeLoading size="small" />;
    }
    return null;
  };

  const renderHeader = () => {
    const { className: iconClassName } = getToolIconInfo();
    const isDeleteTool = toolItem.toolName === 'Delete';
    
    const actionText = isDeleteTool 
      ? '' 
      : (isFailed ? `${toolDisplayInfo.name}${t('toolCards.file.failed')}` : `${toolDisplayInfo.name}:`);
    
    return (
      <ToolCardHeader
        icon={renderToolIcon()}
        iconClassName={iconClassName}
        action={actionText}
      content={
        <Tooltip content={currentFilePath || fileName} placement="top">
          <span className={`file-name ${isDeleteTool ? 'file-name--muted' : ''}`}>
            {fileName}
          </span>
        </Tooltip>
      }
      extra={
        <>
          {isParamsStreaming && (status === 'preparing' || status === 'streaming') && (
            <span className="params-streaming-indicator">
              {currentFilePath ? t('toolCards.file.receivingParams') : t('toolCards.file.analyzing')}
            </span>
          )}
          
          {isDeleteTool && !isParamsStreaming && !isFailed && !isLoading && status === 'completed' && (
            <span className="delete-label">{t('toolCards.file.deletedLabel')}</span>
          )}
          
          {!isDeleteTool && !isParamsStreaming && !isFailed && !isLoading && (
            (currentFileDiffStats.additions > 0 || currentFileDiffStats.deletions > 0 || oldStringContent || newStringContent || contentPreview)
          ) && (
            <Tooltip content={isPreviewExpanded ? t('toolCards.file.collapsePreview') : t('toolCards.file.expandPreview')} placement="top">
              <button
                className="diff-preview-group"
                onClick={(e) => {
                  e.stopPropagation();
                  window.dispatchEvent(new CustomEvent('tool-card-toggle'));
                  const next = !isPreviewExpanded;
                  userCollapsedRef.current = !next;
                  setIsPreviewExpanded(next);
                }}
              >
                {currentFileDiffStats.additions > 0 && (
                  <span className="additions">+{currentFileDiffStats.additions}</span>
                )}
                {currentFileDiffStats.deletions > 0 && (
                  <span className="deletions">-{currentFileDiffStats.deletions}</span>
                )}
                {isPreviewExpanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
              </button>
            </Tooltip>
          )}
          
          {!isDeleteTool && !isFailed && !isLoading && status === 'completed' && (
            <div className="compact-actions" onClick={(e) => e.stopPropagation()}>
              <Tooltip content={t('toolCards.file.viewGitDiff')}>
                <button
                  className="compact-action-btn git-diff-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    handleOpenBaselineDiff();
                  }}
                  disabled={!currentFile || !currentWorkspace}
                >
                  <GitBranch size={12} />
                </button>
              </Tooltip>
            </div>
          )}
          
          {isFailed && (
            <div className="error-expand-indicator">
              {isErrorExpanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            </div>
          )}
        </>
      }
      statusIcon={isDeleteTool ? null : renderStatusIcon()}
    />
    );
  };

  const handleCodeLineClick = useCallback(async (lineNumber: number, filePath?: string) => {
    if (!filePath) return;
    
    try {
      const { editorJumpService } = await import('../../shared/services/EditorJumpService');
      await editorJumpService.jumpToFile(filePath, lineNumber, 1);
    } catch (error) {
      log.error('Failed to jump to line', { filePath, lineNumber, error });
    }
  }, []);

  const renderExpandedContent = () => {
    if (isFailed) return null;

    if (toolItem.toolName === 'Edit') {
      if (isParamsStreaming && newStringContent) {
        return (
          <div className="streaming-content-preview">
            <div className="preview-text">
              <CodePreview
                content={newStringContent}
                filePath={currentFilePath}
                isStreaming={isParamsStreaming}
                showLineNumbers={false}
                maxHeight={300}
                autoScrollToBottom={true}
                onLineClick={handleCodeLineClick}
              />
            </div>
          </div>
        );
      }
      
      if (status === 'completed' && !isParamsStreaming && (oldStringContent || newStringContent)) {
        return (
          <div className="streaming-content-preview">
            <div className="preview-text">
              <InlineDiffPreview
                originalContent={oldStringContent}
                modifiedContent={newStringContent}
                filePath={currentFilePath}
                maxHeight={300}
                showLineNumbers={false}
                lineNumberMode="dual"
                showPrefix={false}
                contextLines={-1}
              />
            </div>
          </div>
        );
      }
    }

    if (toolItem.toolName === 'Write') {
      if (isParamsStreaming && contentPreview) {
        return (
          <div className="streaming-content-preview">
            <div className="preview-text">
              <CodePreview
                content={contentPreview}
                filePath={currentFilePath}
                isStreaming={isParamsStreaming}
                showLineNumbers={false}
                maxHeight={300}
                autoScrollToBottom={true}
                onLineClick={handleCodeLineClick}
              />
            </div>
          </div>
        );
      }
      
      if (status === 'completed' && !isParamsStreaming && contentPreview) {
        return (
          <div className="streaming-content-preview">
            <div className="preview-text">
              <InlineDiffPreview
                originalContent=""
                modifiedContent={contentPreview}
                filePath={currentFilePath}
                maxHeight={300}
                showLineNumbers={false}
                lineNumberMode="single"
                showPrefix={true}
                contextLines={-1}
              />
            </div>
          </div>
        );
      }
    }

    return null;
  };

  const renderErrorContent = () => (
    <div className="error-content">
      <div className="error-title">
        <XCircle size={14} />
        <span>{toolDisplayInfo.name}{t('toolCards.file.failed')}</span>
      </div>
      <div className="error-message">{getErrorMessage()}</div>
    </div>
  );

  const isDeleteTool = toolItem.toolName === 'Delete';

  const getDeleteStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
      case 'preparing':
        return <Loader2 className="animate-spin" size={12} />;
      case 'completed':
        return <Check size={12} className="icon-check-done" />;
      case 'pending':
      case 'confirmed':
      case 'pending_confirmation':
      case 'analyzing':
      default:
        return <Clock size={12} />;
    }
  };

  const renderDeleteContent = () => {
    const baseLabel = `${t('toolCards.file.delete')}: ${fileName}`;

    if (status === 'completed') {
      return baseLabel;
    }

    if (status === 'error') {
      return `${t('toolCards.file.delete')}${t('toolCards.file.failed')}: ${fileName}`;
    }

    return baseLabel;
  };

  if (isDeleteTool) {
    return (
      <CompactToolCard
        status={status}
        isExpanded={false}
        className="read-file-card delete-file-card"
        clickable={false}
        header={
          <CompactToolCardHeader
            statusIcon={getDeleteStatusIcon()}
            content={renderDeleteContent()}
            extra={status === 'completed' ? t('toolCards.file.deletedLabel') : undefined}
          />
        }
      />
    );
  }

  return (
    <BaseToolCard
      status={status}
      isExpanded={isPreviewExpanded}
      onClick={handleCardClick}
      className={`file-operation-card ${isDeleteTool ? 'non-clickable' : ''}`}
      header={renderHeader()}
      expandedContent={renderExpandedContent()}
      errorContent={isFailed && isErrorExpanded ? renderErrorContent() : null}
      isFailed={isFailed}
    />
  );
};
