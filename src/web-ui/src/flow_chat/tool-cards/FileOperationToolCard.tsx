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
  FileEdit,
  FilePlus,
  FileX2,
  ChevronRight,
  Loader2,
  Clock,
  Check,
  X,
} from 'lucide-react';
import { CubeLoading, IconButton } from '../../component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { useSnapshotState } from '../../tools/snapshot_system/hooks/useSnapshotState';
import { SnapshotEventBus, SNAPSHOT_EVENTS } from '../../tools/snapshot_system/core/SnapshotEventBus';
import { useOptionalCurrentWorkspace } from '../../infrastructure/contexts/WorkspaceContext';
import { createDiffEditorTab } from '../../shared/utils/tabUtils';
import { fileTabManager } from '../../shared/services/FileTabManager';
import { CodePreview } from '../components/CodePreview';
import { InlineDiffPreview } from '../components/InlineDiffPreview';
import { Tooltip } from '@/component-library';
import { diffLines } from 'diff';
import { createLogger } from '@/shared/utils/logger';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { hasNonFileUriScheme } from '@/shared/utils/pathUtils';
import { notificationService } from '@/shared/notification-system';
import { useGitState } from '@/tools/git/hooks/useGitState';
import { ToolCardHeaderActions } from './ToolCardHeaderActions';
import './FileOperationToolCard.scss';

const log = createLogger('FileOperationToolCard');
const FILE_OPERATION_STREAMING_MAX_HEIGHT = 4 * 22; // 88px – compact while streaming
const FILE_OPERATION_DIFF_MAX_HEIGHT = 15 * 22;     // 330px – comfortable diff reading when expanded

interface FileOperationToolCardProps extends ToolCardProps {
  sessionId?: string;
}

export const FileOperationToolCard: React.FC<FileOperationToolCardProps> = ({
  toolItem,
  config,
  sessionId,
  onOpenInEditor,
  onConfirm,
  onReject,
}) => {
  const { t } = useTranslation('flow-chat');
  const {
    toolCall,
    toolResult,
    status,
    isParamsStreaming,
    partialParams,
    requiresConfirmation,
    userConfirmed,
  } = toolItem;
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
  const { workspace: currentWorkspace } = useOptionalCurrentWorkspace();
  const { isRepository: workspaceIsGitRepo } = useGitState({
    repositoryPath: currentWorkspace?.rootPath ?? '',
    layers: ['basic'],
    participateInWindowFocusRefresh: false,
  });

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
  const showConfirmationActions = Boolean(
    requiresConfirmation &&
    !userConfirmed &&
    status !== 'completed' &&
    status !== 'cancelled' &&
    status !== 'error'
  );
  
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

  const getSingleLineErrorMessage = () => {
    return String(getErrorMessage()).replace(/\s+/g, ' ').trim();
  };

  const handleOpenInCodeEditor = useCallback(async () => {
    if (!currentFilePath) return;

    if (!sessionId || !currentFilePath || hasNonFileUriScheme(currentFilePath)) {
      fileTabManager.openFile({
        filePath: currentFilePath,
        fileName,
        mode: 'agent',
      });
      return;
    }

    try {
      const { snapshotAPI } = await import('../../infrastructure/api');
      const diffData = await snapshotAPI.getOperationDiff(sessionId, currentFilePath, toolCall?.id);
      const jumpToLine = diffData.anchorLine ? Number(diffData.anchorLine) : undefined;

      if (toolItem.toolName === 'Delete') {
        window.dispatchEvent(new CustomEvent('expand-right-panel'));
        setTimeout(() => {
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
        }, 250);
        return;
      }

      fileTabManager.openFile({
        filePath: currentFilePath,
        fileName,
        jumpToLine,
        mode: 'agent',
      });
    } catch (error) {
      log.error('Failed to open in CodeEditor', { sessionId, filePath: currentFilePath, error });
      if (toolItem.toolName === 'Delete') {
        window.dispatchEvent(new CustomEvent('expand-right-panel'));
        setTimeout(() => {
          createDiffEditorTab(
            currentFilePath,
            fileName,
            '',
            '',
            true,
            'agent'
          );
        }, 250);
        return;
      }

      fileTabManager.openFile({
        filePath: currentFilePath,
        fileName,
        mode: 'agent',
      });
    }
  }, [sessionId, currentFilePath, toolCall?.id, fileName, toolItem.toolName]);

  const canOpenFullCode =
    !isFailed &&
    toolItem.toolName !== 'Delete' &&
    status === 'completed' &&
    Boolean(currentFilePath) &&
    Boolean(sessionId || onOpenInEditor);

  const handleCardClick = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();

    if (
      (e.target as HTMLElement).closest(
        '.file-op-diff-pill, .file-op-open-full-button, .tool-card-header-actions',
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

    applyContentExpandedState(!isContentExpanded, 'manual');
  }, [
    applyContentExpandedState,
    applyErrorExpandedState,
    isContentExpanded,
    isErrorExpanded,
    isFailed,
    toolItem.toolName,
  ]);

  const handleConfirmClick = useCallback((e: React.MouseEvent<HTMLButtonElement>) => {
    e.preventDefault();
    e.stopPropagation();
    onConfirm?.(toolCall?.input);
  }, [onConfirm, toolCall?.input]);

  const handleRejectClick = useCallback((e: React.MouseEvent<HTMLButtonElement>) => {
    e.preventDefault();
    e.stopPropagation();
    onReject?.();
  }, [onReject]);

  const handleOpenFullCodeClick = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();

    if (!canOpenFullCode || !currentFilePath) {
      return;
    }

    if (sessionId) {
      handleOpenInCodeEditor();
      return;
    }

    onOpenInEditor?.(currentFilePath);
  }, [
    canOpenFullCode,
    currentFilePath,
    handleOpenInCodeEditor,
    onOpenInEditor,
    sessionId,
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

      const originalContent = diffData.originalContent || '';
      const modifiedContent = diffData.modifiedContent || '';

      if (originalContent === modifiedContent) {
        log.info('Baseline diff has no changes, skipping diff editor', {
          filePath: diffFilePath,
          originalLength: originalContent.length,
          modifiedLength: modifiedContent.length,
          operationId: toolCall?.id,
          anchorLine: diffData.anchorLine,
        });
        notificationService.info(
          `No changes to display for ${fileName}: baseline and current content are identical.`
        );
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

    const previewMaxHeight = status === 'completed'
      ? FILE_OPERATION_DIFF_MAX_HEIGHT
      : FILE_OPERATION_STREAMING_MAX_HEIGHT;

    if (toolItem.toolName === 'Edit') {
      if (status !== 'completed' && newStringContent) {
        return (
          <div className="streaming-content-preview">
            <div className="preview-text">
              <CodePreview
                content={newStringContent}
                filePath={currentFilePath}
                isStreaming={isParamsStreaming}
                showLineNumbers={isContentExpanded}
                maxHeight={previewMaxHeight}
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
                maxHeight={previewMaxHeight}
                showLineNumbers={isContentExpanded}
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
                showLineNumbers={isContentExpanded}
                maxHeight={previewMaxHeight}
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
                maxHeight={previewMaxHeight}
                showLineNumbers={isContentExpanded}
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
        return <Loader2 className="animate-spin" size={16} />;
      case 'completed':
        return <Check size={16} className="icon-check-done" />;
      case 'pending':
      case 'confirmed':
      case 'pending_confirmation':
      case 'analyzing':
      default:
        return <Clock size={16} />;
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
    !isFailed &&
    Boolean(expandedContent);

  const isCardContentExpanded =
    !isFailed &&
    !isDeleteTool &&
    isContentExpanded;

  const renderHeader = () => {
    const { className: iconClassName } = getToolIconInfo();
    const gitDiffDisabled =
      !currentFilePath || !currentWorkspace || !sessionId;
    const hasDiffStats =
      currentFileDiffStats.additions > 0 || currentFileDiffStats.deletions > 0;

    const actionText = isDeleteTool
      ? ''
      : (isFailed ? `${toolDisplayInfo.name}${t('toolCards.file.failed')}` : `${toolDisplayInfo.name}:`);

    return (
      <ToolCardHeader
        icon={renderToolIcon()}
        iconClassName={iconClassName}
        headerExpanded={hasExpandableContent ? isContentExpanded : undefined}
        onAffordanceClick={
          hasExpandableContent
            ? () => applyContentExpandedState(!isContentExpanded, 'manual')
            : undefined
        }
        action={actionText}
      content={
        isFailed ? (
          <span className="file-error-message-inline">
            {getSingleLineErrorMessage()}
          </span>
        ) : (
          <>
            <Tooltip content={currentFilePath || fileName} placement="top">
              <span className={`file-name ${isDeleteTool ? 'file-name--muted' : ''}`}>
                {fileName}
              </span>
            </Tooltip>
            {!isDeleteTool && !isParamsStreaming && !isLoading && hasDiffStats && (
              <Tooltip content={t('toolCards.file.viewGitDiff')} placement="top">
                <button
                  type="button"
                  className={`file-op-diff-pill${gitDiffDisabled ? ' file-op-diff-pill--disabled' : ''}`}
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    if (!gitDiffDisabled) {
                      handleOpenBaselineDiff();
                    }
                  }}
                  aria-label={t('toolCards.file.viewGitDiff')}
                  title={t('toolCards.file.viewGitDiff')}
                >
                {currentFileDiffStats.additions > 0 && (
                  <span className="additions">+{currentFileDiffStats.additions}</span>
                )}
                {currentFileDiffStats.deletions > 0 && (
                  <span className="deletions">-{currentFileDiffStats.deletions}</span>
                )}
                  {workspaceIsGitRepo ? (
                    <GitBranch size={12} strokeWidth={2} aria-hidden />
                  ) : null}
                </button>
              </Tooltip>
            )}
          </>
        )
      }
      extra={
        <ToolCardHeaderActions className="file-op-header-actions">
          {isParamsStreaming && (status === 'preparing' || status === 'streaming') && (
            <span className="params-streaming-indicator">
              {currentFilePath ? t('toolCards.file.receivingParams') : t('toolCards.file.analyzing')}
            </span>
          )}
          {showConfirmationActions && (
            <>
              <IconButton
                className="tool-card-header-action file-op-header-action file-op-confirm-btn"
                variant="success"
                size="xs"
                onClick={handleConfirmClick}
                tooltip={t('toolCards.mcp.confirmExecute')}
              >
                <Check size={12} />
              </IconButton>
              <IconButton
                className="tool-card-header-action file-op-header-action file-op-reject-btn"
                variant="danger"
                size="xs"
                onClick={handleRejectClick}
                tooltip={t('toolCards.mcp.cancel')}
              >
                <X size={12} />
              </IconButton>
            </>
          )}
          {canOpenFullCode && (
            <Tooltip content={t('toolCards.file.openFullCodeHint')} placement="top">
              <button
                type="button"
                className="file-op-open-full-button"
                onClick={handleOpenFullCodeClick}
                aria-label={t('toolCards.file.openFullCodeHint')}
              >
                <ChevronRight size={14} strokeWidth={2} absoluteStrokeWidth />
              </button>
            </Tooltip>
          )}
        </ToolCardHeaderActions>
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
            icon={getDeleteStatusIcon()}
            content={renderDeleteContent()}
            extra={showConfirmationActions ? (
              <ToolCardHeaderActions className="file-op-header-actions">
                <IconButton
                  className="tool-card-header-action file-op-header-action file-op-confirm-btn"
                  variant="success"
                  size="xs"
                  onClick={handleConfirmClick}
                  tooltip={t('toolCards.mcp.confirmExecute')}
                >
                  <Check size={12} />
                </IconButton>
                <IconButton
                  className="tool-card-header-action file-op-header-action file-op-reject-btn"
                  variant="danger"
                  size="xs"
                  onClick={handleRejectClick}
                  tooltip={t('toolCards.mcp.cancel')}
                >
                  <X size={12} />
                </IconButton>
              </ToolCardHeaderActions>
            ) : undefined}
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
        requiresConfirmation={showConfirmationActions}
        headerExpandAffordance={hasExpandableContent}
        headerAffordanceKind="expand"
      />
    </div>
  );
};
