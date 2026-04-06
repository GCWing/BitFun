/**
 * File operation tool card - refactored based on BaseToolCard
 * Supports Write/Edit/Delete file operations
 *
 * Height-stability contract:
 * - Any state-driven height change must go through
 *   `useToolCardHeightContract.applyExpandedState(...)`.
 * - Any status/render-path change that removes expanded content without
 *   toggling local expand state must dispatch
 *   `flowchat:tool-card-collapse-intent` before the shrink happens.
 * - If preview/result variants stop sharing roughly the same visual height in
 *   the future, treat that as another shrink path and protect it explicitly
 *   instead of relying on `VirtualMessageList` fallback compensation.
 */

import React, { useEffect, useCallback, useMemo, useState, useRef, useLayoutEffect } from 'react';
import { useTranslation } from 'react-i18next';
import {
  XCircle,
  GitBranch,
  FileText,
  ChevronDown,
  ChevronUp,
  FileEdit,
  FilePlus,
  FileX2,
  Loader2,
  Clock,
  Check,
} from 'lucide-react';
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
import { useToolCardHeightContract } from './useToolCardHeightContract';
import './FileOperationToolCard.scss';

const log = createLogger('FileOperationToolCard');
const FILE_OPERATION_PREVIEW_ROWS = 4;
const FILE_OPERATION_PREVIEW_ROW_HEIGHT = 22;
// Keep streaming and completed previews at the same height to avoid layout jumps.
const FILE_OPERATION_PREVIEW_MAX_HEIGHT =
  FILE_OPERATION_PREVIEW_ROWS * FILE_OPERATION_PREVIEW_ROW_HEIGHT;

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
  const toolId = toolItem.id ?? toolCall?.id;
  
  const [isErrorExpanded, setIsErrorExpanded] = useState(false);
  const [isContentExpanded, setIsContentExpanded] = useState(status !== 'completed');
  const [operationDiffStats, setOperationDiffStats] = useState<{ additions: number; deletions: number } | null>(null);
  
  const hasInitializedCompletionEffectRef = useRef(false);
  const previousCompletionEndTimeRef = useRef<number | null>(toolItem.endTime ?? null);
  const previousStatusRef = useRef(status);
  const lastStableExpandedHeightRef = useRef<number>(0);
  const {
    cardRootRef,
    applyExpandedState: applyHeightContractExpandedState,
  } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });
  
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

  const applyContentExpandedState = useCallback((
    nextExpanded: boolean,
    reason: 'manual' | 'auto',
  ) => {
    applyHeightContractExpandedState(
      isContentExpanded,
      nextExpanded,
      setIsContentExpanded,
      { reason },
    );
  }, [applyHeightContractExpandedState, isContentExpanded]);

  const applyErrorExpandedState = useCallback((
    nextExpanded: boolean,
    reason: 'manual' | 'auto',
  ) => {
    applyHeightContractExpandedState(
      isErrorExpanded,
      nextExpanded,
      setIsErrorExpanded,
      { reason },
    );
  }, [applyHeightContractExpandedState, isErrorExpanded]);

  useEffect(() => {
    if (error) {
      log.error('File operation error', { filePath: currentFilePath, error });
      setTimeout(clearError, 3000);
    }
  }, [error, clearError, currentFilePath]);

  useEffect(() => {
    if (previousStatusRef.current !== status) {
      if (status === 'completed' && !isFailed) {
        applyContentExpandedState(false, 'auto');
      } else if (status !== 'completed') {
        applyContentExpandedState(true, 'auto');
      }
      previousStatusRef.current = status;
    }
  }, [
    applyContentExpandedState,
    cardRootRef,
    contentPreview,
    currentFilePath,
    isContentExpanded,
    isFailed,
    oldStringContent,
    status,
    toolId,
    toolItem.toolName,
  ]);

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
        // TODO: Persist diff stats with the tool result so historical cards can
        // read a static value instead of recomputing on every remount.
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
  const previewVariant = useMemo(() => {
    if (toolItem.toolName === 'Edit') {
      if (status !== 'completed' && newStringContent) {
        return 'streaming-code';
      }
      if (status === 'completed' && !isParamsStreaming && (oldStringContent || newStringContent)) {
        return 'completed-diff';
      }
    }

    if (toolItem.toolName === 'Write') {
      if (status !== 'completed' && contentPreview) {
        return 'streaming-code';
      }
      if (status === 'completed' && !isParamsStreaming && contentPreview) {
        return 'completed-diff';
      }
    }

    return 'none';
  }, [
    contentPreview,
    isParamsStreaming,
    newStringContent,
    oldStringContent,
    status,
    toolItem.toolName,
  ]);

  useLayoutEffect(() => {
    const measuredHeight = cardRootRef.current?.getBoundingClientRect().height ?? 0;
    if (!isFailed && isContentExpanded && measuredHeight > 0) {
      lastStableExpandedHeightRef.current = measuredHeight;
    }
  }, [cardRootRef, isContentExpanded, isFailed, previewVariant, status]);

  useLayoutEffect(() => {
    const previousStatus = previousStatusRef.current;
    const isNewFailure = previousStatus !== status && status === 'error';
    if (!isNewFailure || !isContentExpanded) {
      return;
    }

    const currentMeasuredHeight = cardRootRef.current?.getBoundingClientRect().height ?? 0;
    const lastStableExpandedHeight = lastStableExpandedHeightRef.current;
    const estimatedShrinkHeight = Math.max(lastStableExpandedHeight, currentMeasuredHeight);

    if (estimatedShrinkHeight <= currentMeasuredHeight + 0.5) {
      return;
    }

    window.dispatchEvent(new CustomEvent('flowchat:tool-card-collapse-intent', {
      detail: {
        toolId: toolId ?? null,
        toolName: toolItem.toolName,
        cardHeight: estimatedShrinkHeight,
        filePath: currentFilePath || null,
        reason: 'auto',
      },
    }));
    window.dispatchEvent(new CustomEvent('tool-card-toggle'));
  }, [
    cardRootRef,
    currentFilePath,
    isContentExpanded,
    previewVariant,
    status,
    toolId,
    toolItem.toolName,
  ]);

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
    if (
      (e.target as HTMLElement).closest(
        '.file-op-git-rail:not(.file-op-git-rail--disabled)',
      )
    ) {
      return;
    }
    
    if (isFailed) {
      applyErrorExpandedState(!isErrorExpanded, 'manual');
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
  }, [
    applyErrorExpandedState,
    currentFilePath,
    handleOpenInCodeEditor,
    isErrorExpanded,
    isFailed,
    onOpenInEditor,
    sessionId,
    status,
    toolItem.toolName,
  ]);

  const handleOpenBaselineDiff = useCallback(async () => {
    if (!currentFilePath || !currentWorkspace || !sessionId) {
      log.warn('Cannot open diff: missing required info', {
        hasFilePath: !!currentFilePath,
        hasWorkspace: !!currentWorkspace,
        hasSessionId: !!sessionId
      });
      return;
    }

    const diffFilePath = currentFile?.filePath || currentFilePath;
    const fileName = diffFilePath.split(/[/\\]/).pop() || diffFilePath;

    try {
      const { snapshotAPI } = await import('../../infrastructure/api');
      
      const diffData = await snapshotAPI.getOperationDiff(
        sessionId,
        diffFilePath,
        toolCall?.id,
        currentWorkspace.rootPath
      );

      if ((diffData.originalContent || '') === (diffData.modifiedContent || '')) {
        log.debug('Skipping empty baseline diff', { filePath: diffFilePath });
        return;
      }

      window.dispatchEvent(new CustomEvent('expand-right-panel'));

      setTimeout(() => {
        createDiffEditorTab(
          diffFilePath,
          fileName,
          diffData.originalContent || '',
          diffData.modifiedContent || '',
          false,
          'agent',
          currentWorkspace.rootPath,
          undefined,
          false,
          {
            titleKind: 'diff',
            duplicateKeyPrefix: 'diff'
          }
        );
      }, 250);
    } catch (error) {
      log.error('Failed to open Baseline Diff', { filePath: currentFilePath, error });
    }
  }, [currentFile, currentFilePath, currentWorkspace, sessionId, toolCall?.id]);

  const getToolIconInfo = () => {
    const iconMap: Record<string, { icon: React.ReactNode; className: string }> = {
      'Write': { icon: <FilePlus size={16} />, className: 'write-icon' },
      'Edit': { icon: <FileEdit size={16} />, className: 'edit-icon' },
      'Delete': { icon: <FileX2 size={16} />, className: 'delete-icon' }
    };
    
    return iconMap[toolItem.toolName] || { icon: <FileText size={16} />, className: 'file-icon' };
  };

  const renderToolIcon = () => {
    const { icon } = getToolIconInfo();
    return icon;
  };

  const renderStatusIcon = () => {
    const shouldShowStatusIcon = (
      status === 'preparing' ||
      status === 'streaming' ||
      (status === 'running' && previewVariant === 'none')
    );

    if (shouldShowStatusIcon) {
      return <CubeLoading size="small" />;
    }
    return null;
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
      if (status !== 'completed' && newStringContent) {
        return (
          <div className="streaming-content-preview">
            <div className="preview-text">
              <CodePreview
                content={newStringContent}
                filePath={currentFilePath}
                isStreaming={isParamsStreaming}
                showLineNumbers={false}
                maxHeight={FILE_OPERATION_PREVIEW_MAX_HEIGHT}
                autoScrollToBottom={isParamsStreaming}
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
                maxHeight={FILE_OPERATION_PREVIEW_MAX_HEIGHT}
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
      if (status !== 'completed' && contentPreview) {
        return (
          <div className="streaming-content-preview">
            <div className="preview-text">
              <CodePreview
                content={contentPreview}
                filePath={currentFilePath}
                isStreaming={isParamsStreaming}
                showLineNumbers={false}
                maxHeight={FILE_OPERATION_PREVIEW_MAX_HEIGHT}
                autoScrollToBottom={isParamsStreaming}
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
                maxHeight={FILE_OPERATION_PREVIEW_MAX_HEIGHT}
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
    if (status === 'error') {
      return `${t('toolCards.file.delete')}${t('toolCards.file.failed')}: ${fileName}`;
    }
    return <>{t('toolCards.file.delete')}: <span className="delete-file-name">{fileName}</span></>;
  };

  const expandedContent = renderExpandedContent();
  const hasExpandableContent =
    status === 'completed' &&
    !isFailed &&
    !isDeleteTool &&
    Boolean(expandedContent);

  const isCardContentExpanded =
    !isDeleteTool &&
    !isFailed &&
    (status === 'completed' ? isContentExpanded : true);

  const opensPanelOnClick =
    !isFailed &&
    !isDeleteTool &&
    (Boolean(currentFilePath && sessionId && status === 'completed') ||
      Boolean(currentFilePath && onOpenInEditor));

  const renderHeader = () => {
    const { className: iconClassName } = getToolIconInfo();
    const gitRailDisabled =
      !currentFilePath || !currentWorkspace || !sessionId;

    const actionText = isDeleteTool
      ? ''
      : (isFailed ? `${toolDisplayInfo.name}${t('toolCards.file.failed')}` : `${toolDisplayInfo.name}:`);

    return (
      <ToolCardHeader
        icon={renderToolIcon()}
        iconClassName={iconClassName}
        headerExpanded={
          isFailed
            ? isErrorExpanded
            : hasExpandableContent
              ? isContentExpanded
              : undefined
        }
        onAffordanceClick={
          hasExpandableContent
            ? () => applyContentExpandedState(!isContentExpanded, 'manual')
            : undefined
        }
        action={actionText}
      content={
        <>
          <Tooltip content={currentFilePath || fileName} placement="top">
            <span className={`file-name ${isDeleteTool ? 'file-name--muted' : ''}`}>
              {fileName}
            </span>
          </Tooltip>
          {!isDeleteTool && !isParamsStreaming && !isFailed && !isLoading && (
            (currentFileDiffStats.additions > 0 || currentFileDiffStats.deletions > 0)
          ) && (
            <span className="diff-preview-group">
              {currentFileDiffStats.additions > 0 && (
                <span className="additions">+{currentFileDiffStats.additions}</span>
              )}
              {currentFileDiffStats.deletions > 0 && (
                <span className="deletions">-{currentFileDiffStats.deletions}</span>
              )}
            </span>
          )}
        </>
      }
      extra={
        <>
          {isParamsStreaming && (status === 'preparing' || status === 'streaming') && (
            <span className="params-streaming-indicator">
              {currentFilePath ? t('toolCards.file.receivingParams') : t('toolCards.file.analyzing')}
            </span>
          )}
          {!isDeleteTool &&
            !isFailed &&
            !isLoading &&
            status === 'completed' &&
            currentFilePath && (
              <Tooltip content={t('toolCards.file.viewGitDiff')} placement="top">
                <div
                  className={`file-op-git-rail${gitRailDisabled ? ' file-op-git-rail--disabled' : ''}`}
                >
                  {!gitRailDisabled && (
                    <button
                      type="button"
                      className="file-op-git-rail__hit"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleOpenBaselineDiff();
                      }}
                      aria-label={t('toolCards.file.viewGitDiff')}
                      title={t('toolCards.file.viewGitDiff')}
                    />
                  )}
                  <div className="file-op-git-rail__visual" aria-hidden>
                    <GitBranch size={16} strokeWidth={2} />
                  </div>
                </div>
              </Tooltip>
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
          />
        }
      />
    );
  }

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <BaseToolCard
        status={status}
        isExpanded={isCardContentExpanded}
        onClick={handleCardClick}
        className={`file-operation-card ${isDeleteTool ? 'non-clickable' : ''}`}
        header={renderHeader()}
        expandedContent={expandedContent}
        errorContent={isFailed && isErrorExpanded ? renderErrorContent() : null}
        isFailed={isFailed}
        headerExpandAffordance={hasExpandableContent || opensPanelOnClick || isFailed}
        headerAffordanceKind={
          hasExpandableContent
            ? 'expand'
            : opensPanelOnClick
              ? 'open-panel-right'
              : 'expand'
        }
      />
    </div>
  );
};
