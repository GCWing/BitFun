/**
 * Terminal tool card component
 * Displays command execution output (streaming progress + final result)
 * 
 * Status-driven design:
 * - All button display logic depends entirely on backend status, no local state redundancy
 * - Confirm button: only shown when status === 'pending_confirmation'
 * - Interrupt button: only shown when status === 'running'
 * 
 * - Uses _progressMessage to display real-time progress (from ToolExecutionProgress event)
 * - Uses output field to display completed results (no longer distinguishes stdout/stderr)
 * - Clicking "Open Terminal in right panel" button opens full Terminal tab
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
import { useToolCardHeightContract } from './useToolCardHeightContract';
import './TerminalToolCard.scss';

const log = createLogger('TerminalToolCard');
const TERMINAL_OUTPUT_PREVIEW_ROWS = 4;
const TERMINAL_OUTPUT_ESTIMATED_LINE_HEIGHT = 18;
const TERMINAL_OUTPUT_VERTICAL_PADDING = 16;
const TERMINAL_OUTPUT_PREVIEW_MAX_HEIGHT =
  TERMINAL_OUTPUT_PREVIEW_ROWS * TERMINAL_OUTPUT_ESTIMATED_LINE_HEIGHT + TERMINAL_OUTPUT_VERTICAL_PADDING;

interface TerminalToolCardProps extends ToolCardProps {
  terminalSessionId?: string;
}

const TERMINAL_STATES = ['completed', 'cancelled', 'error', 'rejected'] as const;

function isTerminalStatus(status: string): boolean {
  return TERMINAL_STATES.includes(status as typeof TERMINAL_STATES[number]);
}

interface ExpandedStateCache {
  expanded: boolean;
  isManual: boolean;
}
const expandedStateCache = new Map<string, ExpandedStateCache>();

function getCachedExpandedState(toolId: string | undefined): ExpandedStateCache | undefined {
  if (!toolId) return undefined;
  return expandedStateCache.get(toolId);
}

function setCachedExpandedState(toolId: string | undefined, expanded: boolean, isManual: boolean): void {
  if (!toolId) return;
  expandedStateCache.set(toolId, { expanded, isManual });
}

function getInitialExpandedState(toolId: string | undefined, status: string): boolean {
  const cached = getCachedExpandedState(toolId);
  if (cached !== undefined) {
    return cached.expanded;
  }
  if (isTerminalStatus(status) || status === 'pending_confirmation') {
    return false;
  }
  return true;
}

export const TerminalToolCard: React.FC<TerminalToolCardProps> = ({
  toolItem,
  onConfirm,
  onReject,
  onExpand,
  terminalSessionId: propTerminalSessionId
}) => {
  const { t } = useTranslation('flow-chat');
  const toolCall = toolItem.toolCall;
  const toolResult = toolItem.toolResult;
  const command = toolCall?.input?.command;
  
  const status = toolItem.status || 'pending';
  const progressMessage = (toolItem as any)._progressMessage || '';
  
  const terminalSessionId = useMemo(() => {
    if (toolItem.terminalSessionId && !toolItem.terminalSessionId.startsWith('FlowChat-')) {
      return toolItem.terminalSessionId;
    }

    if (toolResult?.result?.terminal_session_id) {
      const id = toolResult.result.terminal_session_id;
      if (typeof id === 'string' && !id.startsWith('FlowChat-')) {
        return id;
      }
    }
    
    if (propTerminalSessionId && !propTerminalSessionId.startsWith('FlowChat-')) {
      return propTerminalSessionId;
    }
    
    return undefined;
  }, [toolItem.terminalSessionId, toolResult, propTerminalSessionId]);

  const showConfirmButtons = status === 'pending_confirmation';
  const showInterruptButton = status === 'running';
  const canEditCommand = showConfirmButtons;
  
  const [userAction, setUserAction] = useState<'none' | 'rejected' | 'interrupted'>('none');
  const toolId = toolItem.id ?? toolCall?.id;
  const isTerminalState = isTerminalStatus(status);
  
  const [isExpanded, setIsExpanded] = useState(() => getInitialExpandedState(toolId, status));
  const [isExecuting, setIsExecuting] = useState(false);
  const [isEditingCommand, setIsEditingCommand] = useState(false);
  const [isCommandTruncated, setIsCommandTruncated] = useState(false);
  const [editedCommand, setEditedCommand] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const commandRef = useRef<HTMLElement | null>(null);
  const hasInitializedExpand = useRef(false);
  const previousStatusRef = useRef<string>(status);
  const {
    cardRootRef,
    applyExpandedState: applyHeightContractExpandedState,
  } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });
  
  const [accumulatedOutput, setAccumulatedOutput] = useState('');

  const applyExpandedState = useCallback((
    nextExpanded: boolean,
    isManual: boolean,
    reason: 'manual' | 'auto'
  ) => {
    if (nextExpanded !== isExpanded) {
      applyHeightContractExpandedState(isExpanded, nextExpanded, (nextValue) => {
        setIsExpanded(nextValue);
        setCachedExpandedState(toolId, nextValue, isManual);
      }, {
        reason,
        onExpand,
      });
    } else if (isManual) {
      setCachedExpandedState(toolId, nextExpanded, isManual);
    }
  }, [applyHeightContractExpandedState, isExpanded, onExpand, toolId]);

  useEffect(() => {
    if (terminalSessionId && !hasInitializedExpand.current) {
      if (isTerminalState) {
        hasInitializedExpand.current = true;
        return;
      }
      
      const cached = getCachedExpandedState(toolId);
      if (cached === undefined || !cached.isManual) {
        applyExpandedState(true, false, 'auto');
        setCachedExpandedState(toolId, true, false);
      }
      hasInitializedExpand.current = true;
    }
  }, [applyExpandedState, terminalSessionId, toolId, isTerminalState]);

  useEffect(() => {
    const prevStatus = previousStatusRef.current;
    previousStatusRef.current = status;
    
    const cached = getCachedExpandedState(toolId);
    if (cached?.isManual) {
      return;
    }
    
    if (status === 'running' && prevStatus !== 'running') {
      applyExpandedState(true, false, 'auto');
    }
    
    if (!isTerminalStatus(prevStatus) && isTerminalStatus(status) && isExpanded) {
      applyExpandedState(false, false, 'auto');
    }
  }, [applyExpandedState, isExpanded, status, toolId]);
  
  useEffect(() => {
    if (progressMessage && (status === 'running' || status === 'streaming')) {
      setAccumulatedOutput(prev => prev + progressMessage);
    }
  }, [progressMessage, status]);
  
  useEffect(() => {
    if (status === 'completed' || status === 'error' || status === 'cancelled') {
      setAccumulatedOutput('');
    }
  }, [status]);

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
    if (isEditingCommand) {
      setIsCommandTruncated(false);
      return;
    }

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
  }, [command, isEditingCommand, updateCommandTruncation]);

  const handleStartEdit = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setEditedCommand(command || '');
    setIsEditingCommand(true);
    setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 0);
  }, [command]);

  const handleSaveEdit = useCallback(() => {
    setIsEditingCommand(false);
    if (toolCall?.input) {
      toolCall.input.command = editedCommand;
    }
  }, [editedCommand, toolCall]);

  const handleCancelEdit = useCallback(() => {
    setIsEditingCommand(false);
    setEditedCommand(command || '');
  }, [command]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleSaveEdit();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      handleCancelEdit();
    }
  }, [handleSaveEdit, handleCancelEdit]);

  const handleExecute = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    const commandToExecute = isEditingCommand ? editedCommand : command;
    
    if (!commandToExecute || commandToExecute.trim() === '') {
      return;
    }

    setIsExecuting(true);
    applyExpandedState(true, true, 'manual');
    setAccumulatedOutput('');

    try {
      const inputToConfirm = { 
        ...(toolCall?.input || {}), 
        command: commandToExecute 
      };
      
      onConfirm?.(inputToConfirm);
    } catch (error) {
      log.error('Command confirmation failed', { command: commandToExecute, error });
    } finally {
      setIsExecuting(false);
    }
  }, [command, editedCommand, isEditingCommand, toolCall?.input, onConfirm]);

  const handleReject = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setUserAction('rejected');
    onReject?.();
  }, [onReject]);

  const handleInterrupt = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    
    const toolUseId = toolCall?.id;
    if (!toolUseId) {
      return;
    }

    setUserAction('interrupted');
    
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('cancel_tool', {
        request: {
          toolUseId: toolUseId,
          reason: 'User cancelled'
        }
      });
    } catch (error) {
      log.error('Failed to send cancel signal', { toolUseId, error });
    }
  }, [toolCall?.id]);

  const toggleExpand = useCallback(() => {
    const newExpanded = !isExpanded;
    applyExpandedState(newExpanded, true, 'manual');
  }, [applyExpandedState, isExpanded]);
  
  const handleOpenInPanel = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    if (!terminalSessionId) {
      return;
    }

    const terminalName = `Chat-${terminalSessionId.slice(0, 8)}`;
    createTerminalTab(terminalSessionId, terminalName);
  }, [terminalSessionId]);

  const {
    output,
    exitCode,
    workingDir,
    executionTimeMs,
    wasInterrupted,
  } = useMemo(() => {
    const raw = toolResult?.result;
    let rec: Record<string, unknown> | null = null;
    if (raw != null && typeof raw === 'string') {
      try {
        rec = JSON.parse(raw) as Record<string, unknown>;
      } catch {
        rec = null;
      }
    } else if (raw != null && typeof raw === 'object') {
      rec = raw as Record<string, unknown>;
    }

    if (!rec) {
      return {
        output: '',
        exitCode: 0,
        workingDir: '',
        executionTimeMs: undefined as number | undefined,
        wasInterrupted: false,
      };
    }

    const stdout = typeof rec.stdout === 'string' ? rec.stdout : '';
    const stderr = typeof rec.stderr === 'string' ? rec.stderr : '';
    const combinedOut = [stdout, stderr].filter((s) => s.length > 0).join('\n');
    const outputField = typeof rec.output === 'string' ? rec.output : '';
    const output = outputField || combinedOut;

    const exitRaw = rec.exit_code;
    const exitCode = typeof exitRaw === 'number' ? exitRaw : 0;

    const workingDir =
      typeof rec.working_directory === 'string' ? rec.working_directory : '';

    const execInResult =
      typeof rec.execution_time_ms === 'number' ? rec.execution_time_ms : undefined;
    const durationInResult =
      typeof rec.duration_ms === 'number' ? rec.duration_ms : undefined;
    const executionTimeMs =
      execInResult ?? durationInResult ?? toolResult?.duration_ms;

    const wasInterrupted = Boolean(rec.interrupted);

    return { output, exitCode, workingDir, executionTimeMs, wasInterrupted };
  }, [toolResult?.result, toolResult?.duration_ms]);

  const isLoading = status === 'preparing' || status === 'streaming' || status === 'running';
  const isFailed = status === 'error';

  const renderToolIcon = () => {
    return <Terminal size={16} />;
  };

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

    if (isLoading) {
      return <CubeLoading size="small" />;
    }
    return null;
  };

  const renderCommandContent = () => {
    if (isEditingCommand && canEditCommand) {
      return (
        <input
          ref={inputRef}
          type="text"
          className="terminal-command-input"
          value={editedCommand}
          onChange={(e) => setEditedCommand(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={handleSaveEdit}
          onClick={(e) => e.stopPropagation()}
          placeholder={t('toolCards.terminal.inputPlaceholder')}
        />
      );
    }

    const commandNode = (
      <code 
        ref={commandRef}
        className={`terminal-command ${canEditCommand ? 'editable' : ''}`}
        onClick={canEditCommand ? handleStartEdit : undefined}
        title={canEditCommand && !isCommandTruncated ? t('toolCards.terminal.clickToEditCommand') : undefined}
      >
        {command || (canEditCommand ? <span className="command-empty">{t('toolCards.terminal.commandEmpty')}</span> : <span className="command-empty">{t('toolCards.terminal.noCommand')}</span>)}
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
    if (!isTerminalState) {
      return null;
    }
    
    if (userAction === 'rejected') {
      return <span className="terminal-status-text status-rejected">{t('toolCards.terminal.rejected')}</span>;
    }
    if (userAction === 'interrupted' || wasInterrupted) {
      return <span className="terminal-status-text status-cancelled">{t('toolCards.terminal.cancelled')}</span>;
    }
    
    switch (status) {
      case 'completed':
        return null;
      case 'cancelled':
        return <span className="terminal-status-text status-cancelled">{t('toolCards.terminal.cancelled')}</span>;
      case 'error':
        return <span className="terminal-status-text status-error">{t('toolCards.terminal.failed')}</span>;
      default:
        if ((status as string) === 'rejected') {
          return <span className="terminal-status-text status-rejected">{t('toolCards.terminal.rejected')}</span>;
        }
        return null;
    }
  };

  const renderHeader = () => {
    const statusText = renderStatusText();
    const hasHeaderExtra = Boolean(statusText || showConfirmButtons || showInterruptButton);

    return (
      <ToolCardHeader
        icon={renderToolIcon()}
        iconClassName="terminal-icon"
        action={t('toolCards.terminal.executeCommand')}
        content={renderCommandContent()}
        extra={hasHeaderExtra ? (
          <>
            {statusText}

            {showConfirmButtons && (
              <div className="terminal-confirm-actions" onClick={(e) => e.stopPropagation()}>
                <IconButton 
                  className="terminal-action-btn execute-btn"
                  variant="success"
                  size="xs"
                  onClick={handleExecute}
                  disabled={isExecuting || (!isEditingCommand && !command) || (isEditingCommand && !editedCommand)}
                  tooltip={
                    (!isEditingCommand && !command) || (isEditingCommand && !editedCommand)
                      ? t('toolCards.terminal.commandEmptyWarning')
                      : t('toolCards.terminal.executeCommandTitle')
                  }
                >
                  <Play size={12} fill="currentColor" />
                </IconButton>
                <IconButton 
                  className="terminal-action-btn cancel-btn"
                  variant="danger"
                  size="xs"
                  onClick={handleReject}
                  disabled={isExecuting}
                  tooltip={t('toolCards.terminal.cancel')}
                >
                  <X size={14} />
                </IconButton>
              </div>
            )}

            {showInterruptButton && (
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
        ) : undefined}
        statusIcon={renderStatusIcon()}
      />
    );
  };

  const renderExpandedContent = () => {
    return (
      <>
        {(status === 'running' || status === 'streaming') && accumulatedOutput && (
          <div className="terminal-execution-output">
            <TerminalOutputRenderer 
              content={accumulatedOutput}
              className="terminal-xterm-output"
              maxHeight={TERMINAL_OUTPUT_PREVIEW_MAX_HEIGHT}
            />
          </div>
        )}
        
        {(status === 'running' || status === 'streaming') && !accumulatedOutput && (
          <div className="terminal-execution-output terminal-waiting">
            <span className="waiting-text">{t('toolCards.terminal.executingCommand')}</span>
          </div>
        )}

        {status === 'completed' && (
          <div className="terminal-result-container">
            {output && (
              <div className="terminal-result-output">
                <TerminalOutputRenderer 
                  content={output}
                  className="terminal-xterm-output"
                  maxHeight={TERMINAL_OUTPUT_PREVIEW_MAX_HEIGHT}
                />
              </div>
            )}
            <div className="terminal-result-footer">
              {workingDir && (
                <>
                  <span className="terminal-result-label">{t('toolCards.terminal.workingDirectory')}</span>
                  <span className="terminal-result-value">{workingDir}</span>
                </>
              )}
              <span className={`terminal-exit-code ${exitCode === 0 ? 'success' : 'error'}`}>
                {t('toolCards.terminal.exitCode', { code: exitCode })}
              </span>
              {executionTimeMs && (
                <span className="terminal-execution-time">
                  {executionTimeMs}ms
                </span>
              )}
            </div>
          </div>
        )}
        
        {status === 'cancelled' && accumulatedOutput && (
          <div className="terminal-result-container cancelled">
            <div className="terminal-result-output">
              <TerminalOutputRenderer 
                content={accumulatedOutput}
                className="terminal-xterm-output"
                maxHeight={TERMINAL_OUTPUT_PREVIEW_MAX_HEIGHT}
              />
            </div>
            <div className="terminal-result-footer">
              <span className="terminal-cancelled-text">{t('toolCards.terminal.commandInterrupted')}</span>
            </div>
          </div>
        )}
      </>
    );
  };

  const renderErrorContent = () => (
    <div className="error-content">
      <div className="error-message">
        {toolResult?.error || t('toolCards.terminal.executionFailed')}
      </div>
    </div>
  );

  const handleCardClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest('.terminal-action-btn, .terminal-command-input, .terminal-confirm-actions')) {
      return;
    }
    toggleExpand();
  }, [toggleExpand]);

  return (
    <div ref={cardRootRef} data-tool-card-id={toolItem.id ?? toolCall?.id ?? ''}>
      <BaseToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleCardClick}
        className="terminal-tool-card"
        header={renderHeader()}
        expandedContent={isExpanded ? renderExpandedContent() : null}
        errorContent={isFailed ? renderErrorContent() : null}
        isFailed={isFailed}
        requiresConfirmation={showConfirmButtons}
      />
    </div>
  );
};

export default TerminalToolCard;
