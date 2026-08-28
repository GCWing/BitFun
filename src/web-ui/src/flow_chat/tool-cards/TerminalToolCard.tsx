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

import React, { useState, useRef, useCallback, useEffect, useLayoutEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { createTerminalTab } from '@/shared/utils/tabUtils';
import {
  CommandToolCard,
  type CommandToolCardFooterItem,
} from '@bitfun/ui/flow-chat';
import { LazyTerminalOutputRenderer } from '@/tools/terminal/components/LazyTerminalOutputRenderer';
import { createLogger } from '@/shared/utils/logger';
import { notificationService } from '@/shared/notification-system';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { useToolCardCompletionGracePeriod } from './useToolCardCompletionGracePeriod';
import { getTerminalViewState, resolveCanCancelTool } from './terminalToolCardState';
import { ToolTimeoutIndicator } from './ToolTimeoutIndicator';
import { useCopyTextAction } from '../hooks/useCopyTextAction';
import { formatSessionViewPreviewText } from '../utils/sessionViewPreview';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/peerDeviceContextState';

const log = createLogger('TerminalToolCard');
const TERMINAL_COLLAPSED_STATUSES = new Set(['completed', 'cancelled', 'error', 'rejected']);
const TERMINAL_OUTPUT_STREAMING_MAX_ROWS = 4;  // Compact while streaming/executing
const TERMINAL_OUTPUT_EXPANDED_MAX_ROWS = 15;  // Comfortable reading when manually expanded

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

function getAutoExpandedStateForTerminalStatus(
  status: string,
  isLastItem: boolean | undefined,
  keepTailPreview: boolean,
): boolean | null {
  if (isCollapsedTerminalStatus(status)) {
    // A card that was already mounted while live keeps its compact output
    // visible briefly at the tail. It collapses when a newer conversation
    // item takes over or when the completion preview grace period expires.
    return isLastItem === true && keepTailPreview ? null : false;
  }

  if (status === 'pending_confirmation') {
    return false;
  }

  if (status === 'preparing' || status === 'streaming' || status === 'running') {
    return true;
  }

  return null;
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
  const output = formatSessionViewPreviewText(outputField || combinedOutput);

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
  onExpand,
  terminalSessionId: propTerminalSessionId,
  isLastItem,
}) => {
  const { t } = useTranslation('flow-chat');
  const peerDevice = usePeerDeviceModeOptional();
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
  const userToggledRef = useRef(false);
  const {
    cardRootRef,
    applyExpandedState,
  } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });
  const {
    begin: beginCompletionPreview,
    isActive: isCompletionPreviewActive,
  } = useToolCardCompletionGracePeriod({
    eligible:
      isCollapsedTerminalStatus(status) &&
      isLastItem === true &&
      isExpanded &&
      !userToggledRef.current,
  });
  const applyTerminalExpandedState = useCallback((nextExpanded: boolean) => {
    if (nextExpanded === isExpanded) {
      return;
    }

    applyExpandedState(isExpanded, nextExpanded, setIsExpandedState, { onExpand });
  }, [applyExpandedState, isExpanded, onExpand]);

  const toggleExpanded = useCallback(() => {
    userToggledRef.current = true;
    applyTerminalExpandedState(!isExpanded);
  }, [applyTerminalExpandedState, isExpanded]);

  const [interruptRequested, setInterruptRequested] = useState(false);

  useEffect(() => {
    if (status !== 'running') {
      setInterruptRequested(false);
    }
  }, [status]);

  useLayoutEffect(() => {
    if (userToggledRef.current) {
      return;
    }

    const keepTailPreview = isCollapsedTerminalStatus(status) && beginCompletionPreview();
    const nextExpanded = getAutoExpandedStateForTerminalStatus(status, isLastItem, keepTailPreview);
    if (nextExpanded !== null) {
      applyTerminalExpandedState(nextExpanded);
    }
  }, [
    applyTerminalExpandedState,
    beginCompletionPreview,
    isCompletionPreviewActive,
    isLastItem,
    status,
  ]);

  const showConfirmButtons = status === 'pending_confirmation';
  const canExecuteCommand = Boolean(command?.trim());
  const getCopyCommandText = useCallback(
    () => (typeof command === 'string' ? command : ''),
    [command],
  );
  const { copied: commandCopied, copy: copyCommand } = useCopyTextAction({
    getText: getCopyCommandText,
    successMessage: t('toolCards.terminal.commandCopied'),
    failureMessage: t('toolCards.terminal.copyCommandFailed'),
  });

  // The Interrupt button is only meaningful if the current host can actually
  // cancel a running tool. Resolution is centralized in resolveCanCancelTool so
  // the same rule (local → true; cancelTool true/false; null resolved by
  // hostKind — old Desktop supports it, old CLI doesn't) is unit-testable and
  // stays consistent with the tool-catalog host-kind resolution. See PR #2428
  // round 5 #1.
  const peerActive = Boolean(peerDevice?.peerMode.active);
  const canCancelTool = resolveCanCancelTool(
    peerActive,
    peerDevice?.currentPeerCapabilities ?? null,
  );

  const viewState = useMemo(() => {
    return getTerminalViewState({
      status,
      liveOutput,
      isParamsStreaming,
      interruptRequested,
      showConfirmButtons,
      wasInterrupted: parsedResult.wasInterrupted,
      canCancelTool,
    });
  }, [
    canCancelTool,
    isParamsStreaming,
    interruptRequested,
    liveOutput,
    parsedResult.wasInterrupted,
    showConfirmButtons,
    status,
  ]);
  const waitingMessage = viewState.waitingMessageKey ? t(viewState.waitingMessageKey) : null;

  const handleInterrupt = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();

    const toolUseId = toolCall?.id;
    if (!toolUseId || interruptRequested) {
      return;
    }

    setInterruptRequested(true);

    try {
      await api.invoke('cancel_tool', {
        request: {
          toolUseId,
          reason: 'User cancelled',
        },
      });
    } catch (error) {
      setInterruptRequested(false);
      log.error('Failed to send cancel signal', { toolUseId, error });
      // Surface the failure to the user instead of silently restoring the
      // button: a "not supported on ... peer host" error (or a transport
      // failure) means the target command keeps running and the click did
      // nothing visible. See PR #2428 round 5 #1.
      notificationService.error(t('toolCards.terminal.interruptFailed'));
    }
  }, [interruptRequested, t, toolCall?.id]);

  const handleOpenInPanel = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    if (!terminalSessionId) {
      return;
    }

    const terminalName = `Chat-${terminalSessionId.slice(0, 8)}`;
    createTerminalTab(terminalSessionId, terminalName);
  }, [terminalSessionId]);

  const compactSettledPreview =
    isExpanded &&
    isLastItem === true &&
    isCollapsedTerminalStatus(status) &&
    !userToggledRef.current;
  const isStreamingPhase =
    viewState.displayPhase === 'live_output' ||
    viewState.displayPhase === 'receiving_params' ||
    viewState.displayPhase === 'executing';
  const maxRows = isStreamingPhase || compactSettledPreview
    ? TERMINAL_OUTPUT_STREAMING_MAX_ROWS
    : TERMINAL_OUTPUT_EXPANDED_MAX_ROWS;
  const outputText = viewState.displayPhase === 'live_output' || viewState.showCancelledResult
    ? liveOutput
    : viewState.showCompletedResult
      ? parsedResult.output
      : '';
  const footerItems: CommandToolCardFooterItem[] = [];

  if (viewState.showCompletedResult) {
    if (parsedResult.workingDir) {
      footerItems.push({
        grow: true,
        label: t('toolCards.terminal.workingDirectory'),
        value: parsedResult.workingDir,
      });
    }
    footerItems.push({
      monospace: true,
      tone: parsedResult.exitCode === 0 ? 'success' : 'danger',
      value: (
        <span
          data-testid="chat-shell-command-exit-code"
          data-exit-code={parsedResult.exitCode}
          data-status={parsedResult.exitCode === 0 ? 'success' : 'error'}
        >
          {t('toolCards.terminal.exitCode', { code: parsedResult.exitCode })}
        </span>
      ),
    });
    if (parsedResult.executionTimeMs !== undefined) {
      footerItems.push({ monospace: true, value: `${parsedResult.executionTimeMs}ms` });
    }
  } else if (viewState.showCancelledResult) {
    footerItems.push({
      tone: 'warning',
      value: t('toolCards.terminal.commandInterrupted'),
    });
  }

  return (
    <div ref={cardRootRef} data-bf-adapter="terminal-tool-card" data-tool-card-id={toolId ?? ''}>
      <CommandToolCard
        action={t('toolCards.terminal.executeCommand')}
        command={typeof command === 'string' ? command : ''}
        commandTestId="chat-shell-command-text"
        copyAction={{
          copied: commandCopied,
          copiedLabel: t('toolCards.terminal.commandCopied'),
          disabled: !canExecuteCommand,
          label: t('toolCards.terminal.copyCommand'),
          onPress: copyCommand,
        }}
        data-bf-state={[isExpanded && 'expanded', viewState.isFailed && 'error'].filter(Boolean).join(' ') || undefined}
        data-expanded={isExpanded ? 'true' : 'false'}
        data-status={status}
        data-terminal-session-id={terminalSessionId || ''}
        data-testid="chat-shell-command-card"
        emptyCommand={t(showConfirmButtons ? 'toolCards.terminal.commandEmpty' : 'toolCards.terminal.noCommand')}
        error={viewState.isFailed
          ? toolResult?.error || t('toolCards.terminal.executionFailed')
          : undefined}
        footerItems={footerItems}
        interruptAction={viewState.showInterruptButton ? {
          label: t('toolCards.terminal.interrupt'),
          onPress: handleInterrupt,
        } : undefined}
        isExpanded={isExpanded}
        onToggle={toggleExpanded}
        openAction={terminalSessionId ? {
          label: t('toolCards.terminal.openInPanel'),
          onPress: handleOpenInPanel,
          testId: 'chat-shell-tool-open-panel',
        } : undefined}
        output={outputText ? (
          <div data-testid="chat-shell-command-output">
            <LazyTerminalOutputRenderer content={outputText} maxRows={maxRows} />
          </div>
        ) : undefined}
        outputDensity={isStreamingPhase || compactSettledPreview ? 'compact' : 'expanded'}
        requiresConfirmation={showConfirmButtons}
        status={status}
        statusLabel={viewState.statusLabel
          ? t(`toolCards.terminal.${viewState.statusLabel}`)
          : undefined}
        statusSummary={(
          <ToolTimeoutIndicator
            startTime={toolItem.startTime}
            isRunning={status === 'preparing' || status === 'streaming' || status === 'running'}
            timeoutMs={
              typeof toolCall?.input?.timeout_ms === 'number' && toolCall.input.timeout_ms > 0
                ? toolCall.input.timeout_ms
                : undefined
            }
            showControls={false}
            completedDurationMs={status === 'completed' ? parsedResult.executionTimeMs : undefined}
          />
        )}
        statusTone={viewState.statusClassName === 'status-error' ? 'danger' : 'warning'}
        toggleTestId="chat-shell-command-toggle"
        waitingContent={waitingMessage ?? undefined}
      />
    </div>
  );
};

export default TerminalToolCard;
