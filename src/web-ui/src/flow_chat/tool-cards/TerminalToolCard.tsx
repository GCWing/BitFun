/**
 * Terminal tool card component
 * Displays command execution lifecycle:
 * - receive tool parameters
 * - wait for terminal output after launch
 * - stream real output and final result
 *
 * Design notes:
 * - Final lifecycle always comes from backend tool status
 * - The only local interaction guard is `interruptRequested`, used to prevent
 *   duplicate cancel clicks before the backend status catches up
 * - Live terminal output is rendered from store-managed progress logs
 * - Clicking "Open Terminal in right panel" opens the full Terminal tab
 */

import React, { useState, useRef, useCallback, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { Terminal, Play, X, ExternalLink, Square } from 'lucide-react';
import { createTerminalTab } from '@/shared/utils/tabUtils';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { CubeLoading, IconButton, Tooltip } from '../../component-library';
import { TerminalOutputRenderer } from '@/tools/terminal/components';
import { createLogger } from '@/shared/utils/logger';
import { useToolCardHeightContract, type ToolCardCollapseReason } from './useToolCardHeightContract';
import { getTerminalViewState, type TerminalViewState } from './terminalToolCardState';
import { ToolTimeoutIndicator } from './ToolTimeoutIndicator';
import './TerminalToolCard.scss';

const log = createLogger('TerminalToolCard');
const TERMINAL_COLLAPSED_STATUSES = new Set(['completed', 'cancelled', 'error', 'rejected']);
const TERMINAL_OUTPUT_STREAMING_MAX_HEIGHT = 4 * 18 + 16;  // 88px – compact while streaming/executing
const TERMINAL_OUTPUT_EXPANDED_MAX_HEIGHT = 15 * 18 + 16;  // 286px – comfortable reading when manually expanded

interface TerminalToolCardProps extends ToolCardProps {
  terminalSessionId?: string;
}

interface ParsedTerminalResult {
  output: string;
  exitCode: number;
  workingDir: string;
  executionTimeMs?: number;
  wasInterrupted: boolean;
  terminalSessionId?: string;
}

function normalizeTerminalSessionId(value: unknown): string | undefined {
  if (typeof value !== 'string' || value.startsWith('FlowChat-')) {
    return undefined;
  }

  return value;
}

function isCollapsedTerminalStatus(status: string): boolean {
  return TERMINAL_COLLAPSED_STATUSES.has(status);
}

function getInitialTerminalExpandedState(status: string): boolean {
  return !(isCollapsedTerminalStatus(status) || status === 'pending_confirmation');
}

function getAutoExpandedStateForTerminalStatus(status: string): boolean | null {
  if (isCollapsedTerminalStatus(status) || status === 'pending_confirmation') {
    return false;
  }

  if (status === 'preparing' || status === 'streaming' || status === 'running') {
    return true;
  }

  return null;
}

function renderTerminalExpandedContent(params: {
  viewState: TerminalViewState;
  liveOutput: string;
  parsedResult: ParsedTerminalResult;
  waitingMessage: string | null;
  t: (key: string, options?: Record<string, unknown>) => string;
}): React.ReactNode {
  const { viewState, liveOutput, parsedResult, waitingMessage, t } = params;

  const isStreamingPhase =
    viewState.displayPhase === 'live_output' ||
    viewState.displayPhase === 'receiving_params' ||
    viewState.displayPhase === 'executing';

  const maxHeight = isStreamingPhase
    ? TERMINAL_OUTPUT_STREAMING_MAX_HEIGHT
    : TERMINAL_OUTPUT_EXPANDED_MAX_HEIGHT;

  return (
    <>
      {viewState.displayPhase === 'live_output' && (
        <div className="terminal-execution-output">
          <TerminalOutputRenderer
            content={liveOutput}
            className="terminal-xterm-output"
            maxHeight={maxHeight}
          />
        </div>
      )}

      {(viewState.displayPhase === 'receiving_params' || viewState.displayPhase === 'executing') && waitingMessage && (
        <div className="terminal-execution-output terminal-waiting">
          <span className="waiting-text">{waitingMessage}</span>
        </div>
      )}

      {viewState.showCompletedResult && (
        <div className="terminal-result-container">
          {parsedResult.output && (
            <div className="terminal-result-output">
              <TerminalOutputRenderer
                content={parsedResult.output}
                className="terminal-xterm-output"
                maxHeight={maxHeight}
              />
            </div>
          )}
          <div className="terminal-result-footer">
            {parsedResult.workingDir && (
              <>
                <span className="terminal-result-label">{t('toolCards.terminal.workingDirectory')}</span>
                <span className="terminal-result-value">{parsedResult.workingDir}</span>
              </>
            )}
            <span className={`terminal-exit-code ${parsedResult.exitCode === 0 ? 'success' : 'error'}`}>
              {t('toolCards.terminal.exitCode', { code: parsedResult.exitCode })}
            </span>
            {parsedResult.executionTimeMs && (
              <span className="terminal-execution-time">
                {parsedResult.executionTimeMs}ms
              </span>
            )}
          </div>
        </div>
      )}

      {viewState.showCancelledResult && (
        <div className="terminal-result-container cancelled">
          <div className="terminal-result-output">
            <TerminalOutputRenderer
              content={liveOutput}
              className="terminal-xterm-output"
              maxHeight={maxHeight}
            />
          </div>
          <div className="terminal-result-footer">
            <span className="terminal-cancelled-text">{t('toolCards.terminal.commandInterrupted')}</span>
          </div>
        </div>
      )}
    </>
  );
}

function renderTerminalErrorContent(errorMessage: string): React.ReactNode {
  return (
    <div className="error-content">
      <div className="error-message">{errorMessage}</div>
    </div>
  );
}

function parseTerminalResult(raw: unknown, durationMs?: number): ParsedTerminalResult {
  let record: Record<string, unknown> | null = null;

  if (raw != null && typeof raw === 'string') {
    try {
      record = JSON.parse(raw) as Record<string, unknown>;
    } catch {
      record = null;
    }
  } else if (raw != null && typeof raw === 'object') {
    record = raw as Record<string, unknown>;
  }

  if (!record) {
    return {
      output: '',
      exitCode: 0,
      workingDir: '',
      executionTimeMs: undefined,
      wasInterrupted: false,
      terminalSessionId: undefined,
    };
  }

  const stdout = typeof record.stdout === 'string' ? record.stdout : '';
  const stderr = typeof record.stderr === 'string' ? record.stderr : '';
  const combinedOutput = [stdout, stderr].filter((value) => value.length > 0).join('\n');
  const outputField = typeof record.output === 'string' ? record.output : '';
  const output = outputField || combinedOutput;

  return {
    output,
    exitCode: typeof record.exit_code === 'number' ? record.exit_code : 0,
    workingDir: typeof record.working_directory === 'string' ? record.working_directory : '',
    executionTimeMs:
      typeof record.execution_time_ms === 'number'
        ? record.execution_time_ms
        : typeof record.duration_ms === 'number'
          ? record.duration_ms
          : durationMs,
    wasInterrupted: Boolean(record.interrupted),
    terminalSessionId: normalizeTerminalSessionId(record.terminal_session_id),
  };
}

export const TerminalToolCard: React.FC<TerminalToolCardProps> = ({
  toolItem,
  onConfirm,
  onReject,
  onExpand,
  terminalSessionId: propTerminalSessionId,
}) => {
  const { t } = useTranslation('flow-chat');
  const toolCall = toolItem.toolCall;
  const toolResult = toolItem.toolResult;
  const command = toolCall?.input?.command;
  const status = toolItem.status || 'pending';
  const isParamsStreaming = Boolean(toolItem.isParamsStreaming);
  const progressMessage = typeof (toolItem as any)._progressMessage === 'string'
    ? (toolItem as any)._progressMessage
    : '';

  const parsedResult = useMemo(
    () => parseTerminalResult(toolResult?.result, toolResult?.duration_ms),
    [toolResult?.duration_ms, toolResult?.result],
  );

  const terminalSessionId = useMemo(
    () => normalizeTerminalSessionId(toolItem.terminalSessionId)
      ?? parsedResult.terminalSessionId
      ?? normalizeTerminalSessionId(propTerminalSessionId),
    [parsedResult.terminalSessionId, propTerminalSessionId, toolItem.terminalSessionId],
  );

  const progressLogs = useMemo(() => {
    const logs = (toolItem as any)._progressLogs;
    if (!Array.isArray(logs)) {
      return [];
    }

    return logs.filter((entry): entry is string => typeof entry === 'string');
  }, [toolItem]);

  const liveOutput = useMemo(() => {
    if (progressLogs.length > 0) {
      return progressLogs.join('');
    }

    return progressMessage;
  }, [progressLogs, progressMessage]);

  const toolId = toolItem.id ?? toolCall?.id;
  const [isExpanded, setIsExpandedState] = useState(() => getInitialTerminalExpandedState(status));
  const previousStatusRef = useRef(status);
  const {
    cardRootRef,
    applyExpandedState,
  } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });
  const applyTerminalExpandedState = useCallback((
    nextExpanded: boolean,
    options?: { reason?: ToolCardCollapseReason },
  ) => {
    if (nextExpanded === isExpanded) {
      return;
    }

    applyExpandedState(isExpanded, nextExpanded, setIsExpandedState, {
      reason: options?.reason ?? 'manual',
      onExpand,
    });
  }, [applyExpandedState, isExpanded, onExpand]);

  const toggleExpanded = useCallback(() => {
    applyTerminalExpandedState(!isExpanded, { reason: 'manual' });
  }, [applyTerminalExpandedState, isExpanded]);

  const [interruptRequested, setInterruptRequested] = useState(false);
  const [isCommandTruncated, setIsCommandTruncated] = useState(false);
  const commandRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (status !== 'running') {
      setInterruptRequested(false);
    }
  }, [status]);

  useEffect(() => {
    const prevStatus = previousStatusRef.current;
    previousStatusRef.current = status;

    if (prevStatus === status) {
      return;
    }

    const nextExpanded = getAutoExpandedStateForTerminalStatus(status);
    if (nextExpanded !== null) {
      applyTerminalExpandedState(nextExpanded, { reason: 'auto' });
    }
  }, [applyTerminalExpandedState, status]);

  const updateCommandTruncation = useCallback(() => {
    const element = commandRef.current;
    if (!element) {
      setIsCommandTruncated(false);
      return;
    }

    const nextValue = element.scrollWidth - element.clientWidth > 1;
    setIsCommandTruncated((prev) => (prev === nextValue ? prev : nextValue));
  }, []);

  useEffect(() => {
    const element = commandRef.current;
    if (!element) {
      setIsCommandTruncated(false);
      return;
    }

    const frameId = window.requestAnimationFrame(updateCommandTruncation);
    const resizeObserver = typeof ResizeObserver !== 'undefined'
      ? new ResizeObserver(() => {
          updateCommandTruncation();
        })
      : null;

    resizeObserver?.observe(element);
    if (element.parentElement) {
      resizeObserver?.observe(element.parentElement);
    }

    window.addEventListener('resize', updateCommandTruncation);

    return () => {
      window.cancelAnimationFrame(frameId);
      resizeObserver?.disconnect();
      window.removeEventListener('resize', updateCommandTruncation);
    };
  }, [command, updateCommandTruncation]);

  const showConfirmButtons = status === 'pending_confirmation';
  const canExecuteCommand = Boolean(command?.trim());

  const viewState = useMemo(() => {
    return getTerminalViewState({
      status,
      liveOutput,
      isParamsStreaming,
      interruptRequested,
      showConfirmButtons,
      wasInterrupted: parsedResult.wasInterrupted,
    });
  }, [
    isParamsStreaming,
    interruptRequested,
    liveOutput,
    parsedResult.wasInterrupted,
    showConfirmButtons,
    status,
  ]);
  const waitingMessage = viewState.waitingMessageKey ? t(viewState.waitingMessageKey) : null;

  const handleExecute = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();

    if (!canExecuteCommand) {
      return;
    }

    applyTerminalExpandedState(true, { reason: 'manual' });
    onConfirm?.(toolCall?.input);
  }, [applyTerminalExpandedState, canExecuteCommand, onConfirm, toolCall?.input]);

  const handleReject = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onReject?.();
  }, [onReject]);

  const handleInterrupt = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();

    const toolUseId = toolCall?.id;
    if (!toolUseId || interruptRequested) {
      return;
    }

    setInterruptRequested(true);

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('cancel_tool', {
        request: {
          toolUseId,
          reason: 'User cancelled',
        },
      });
    } catch (error) {
      setInterruptRequested(false);
      log.error('Failed to send cancel signal', { toolUseId, error });
    }
  }, [interruptRequested, toolCall?.id]);

  const handleOpenInPanel = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    if (!terminalSessionId) {
      return;
    }

    const terminalName = `Chat-${terminalSessionId.slice(0, 8)}`;
    createTerminalTab(terminalSessionId, terminalName);
  }, [terminalSessionId]);

  const handleCardClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest('.terminal-action-btn, .terminal-confirm-actions')) {
      return;
    }

    toggleExpanded();
  }, [toggleExpanded]);

  const renderStatusIcon = () => {
    if (terminalSessionId) {
      return (
        <IconButton
          className="terminal-action-btn external-btn"
          variant="ghost"
          size="xs"
          onClick={handleOpenInPanel}
          tooltip={t('toolCards.terminal.openInPanel')}
        >
          <ExternalLink size={12} />
        </IconButton>
      );
    }

    if (viewState.isLoading) {
      return <CubeLoading size="small" />;
    }

    return null;
  };

  const renderCommandContent = () => {
    const commandNode = (
      <code ref={commandRef} className="terminal-command">
        {command
          || <span className="command-empty">{t(showConfirmButtons ? 'toolCards.terminal.commandEmpty' : 'toolCards.terminal.noCommand')}</span>}
      </code>
    );

    if (command && isCommandTruncated) {
      return (
        <Tooltip
          content={<div className="terminal-command-tooltip-content">{command}</div>}
          placement="bottom"
          className="terminal-command-tooltip"
          interactive
        >
          {commandNode}
        </Tooltip>
      );
    }

    return commandNode;
  };

  const renderStatusText = () => {
    if (!viewState.statusLabel || !viewState.statusClassName) {
      return null;
    }

    return (
      <span className={`terminal-status-text ${viewState.statusClassName}`}>
        {t(`toolCards.terminal.${viewState.statusLabel}`)}
      </span>
    );
  };

  const renderHeader = () => (
    <ToolCardHeader
      icon={<Terminal size={14} className="terminal-card-icon" />}
      action={t('toolCards.terminal.executeCommand')}
      content={renderCommandContent()}
      extra={(
        <>
          <ToolTimeoutIndicator
            startTime={toolItem.startTime}
            isRunning={status === 'preparing' || status === 'streaming' || status === 'running'}
            timeoutMs={
              typeof toolCall?.input?.timeout_ms === 'number' && toolCall.input.timeout_ms > 0
                ? toolCall.input.timeout_ms
                : undefined
            }
            showControls={false}
            completedDurationMs={
              status === 'completed' && parsedResult?.executionTimeMs
                ? parsedResult.executionTimeMs
                : undefined
            }
          />
          {viewState.hasHeaderExtra && renderStatusText()}

          {showConfirmButtons && (
            <div className="terminal-confirm-actions" onClick={(e) => e.stopPropagation()}>
              <IconButton
                className="terminal-action-btn execute-btn"
                variant="success"
                size="xs"
                onClick={handleExecute}
                disabled={!canExecuteCommand}
                tooltip={
                  canExecuteCommand
                    ? t('toolCards.terminal.executeCommandTitle')
                    : t('toolCards.terminal.commandEmptyWarning')
                }
              >
                <Play size={12} fill="currentColor" />
              </IconButton>
              <IconButton
                className="terminal-action-btn cancel-btn"
                variant="danger"
                size="xs"
                onClick={handleReject}
                tooltip={t('toolCards.terminal.cancel')}
              >
                <X size={14} />
              </IconButton>
            </div>
          )}

          {viewState.showInterruptButton && (
            <IconButton
              className="terminal-action-btn interrupt-btn"
              variant="warning"
              size="xs"
              onClick={handleInterrupt}
              tooltip={t('toolCards.terminal.interrupt')}
            >
              <Square size={12} fill="currentColor" />
            </IconButton>
          )}
        </>
      )}
      statusIcon={renderStatusIcon()}
    />
  );
  const expandedContent = isExpanded
    ? renderTerminalExpandedContent({ viewState, liveOutput, parsedResult, waitingMessage, t })
    : null;
  const errorContent = viewState.isFailed
    ? renderTerminalErrorContent(toolResult?.error || t('toolCards.terminal.executionFailed'))
    : null;

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <BaseToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleCardClick}
        className="terminal-tool-card"
        header={renderHeader()}
        expandedContent={expandedContent}
        errorContent={errorContent}
        isFailed={viewState.isFailed}
        requiresConfirmation={showConfirmButtons}
      />
    </div>
  );
};

export default TerminalToolCard;
