import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Terminal } from 'lucide-react';
import type { FlowToolItem } from '../types/flow-chat';
import { TerminalOutputRenderer, type TerminalOutputRendererHandle } from '@/tools/terminal/components';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';
import { ToolCardCopyAction, ToolCardHeaderActions } from './ToolCardHeaderActions';
import { ToolCommandPreview } from './ToolCommandPreview';
import { ToolTimeoutIndicator } from './ToolTimeoutIndicator';
import { DotMatrixLoader } from '../../component-library';
import { useToolCardHeightContract, type ToolCardCollapseReason } from './useToolCardHeightContract';
import { formatSessionViewPreviewText } from '../utils/sessionViewPreview';
import './ExecProcessToolCard.scss';

const EXEC_COLLAPSED_STATUSES = new Set(['completed', 'cancelled', 'error', 'rejected']);
const EXEC_OUTPUT_STREAMING_MAX_ROWS = 4;
const EXEC_OUTPUT_EXPANDED_MAX_ROWS = 15;

export interface ExecProcessCardModel {
  kind: 'command' | 'stdin';
  actionLabel: string;
  primaryText: string;
  emptyText: string;
  copyText: string;
  copyDisabled?: boolean;
  waitingText: string;
  noOutputText: string;
  resultNoticeText?: string;
  resultOutput: string;
  workdir?: string;
  sessionId?: number;
  exitCode?: number;
  wallTimeSeconds?: number;
  remote?: boolean;
  tty?: boolean;
}

interface ExecProcessToolCardViewProps {
  toolItem: FlowToolItem;
  model: ExecProcessCardModel;
  onExpand?: () => void;
}

function isCollapsedStatus(status: string): boolean {
  return EXEC_COLLAPSED_STATUSES.has(status);
}

function getInitialExpandedState(status: string): boolean {
  return !isCollapsedStatus(status);
}

function getAutoExpandedStateForStatus(status: string): boolean | null {
  if (isCollapsedStatus(status)) {
    return false;
  }

  if (status === 'preparing' || status === 'streaming' || status === 'running' || status === 'receiving') {
    return true;
  }

  return null;
}

function readProgressLogs(toolItem: FlowToolItem): string[] {
  const logs = (toolItem as any)._progressLogs;
  return Array.isArray(logs) ? logs.filter((entry): entry is string => typeof entry === 'string') : [];
}

function formatSecondsAsMs(seconds?: number): number | undefined {
  return typeof seconds === 'number' && Number.isFinite(seconds)
    ? Math.max(0, Math.round(seconds * 1000))
    : undefined;
}

function renderFooter(model: ExecProcessCardModel, t: (key: string, options?: Record<string, unknown>) => string) {
  const hasFooter =
    model.workdir ||
    model.sessionId != null ||
    model.exitCode != null ||
    model.wallTimeSeconds != null ||
    model.remote != null ||
    model.tty != null;

  if (!hasFooter) {
    return null;
  }

  return (
    <div className="terminal-result-footer exec-process-result-footer">
      {model.workdir && (
        <span className="exec-process-footer-group exec-process-footer-group--workdir">
          <span className="terminal-result-label">{t('toolCards.terminal.workingDirectory')}</span>
          <span className="terminal-result-value">{model.workdir}</span>
        </span>
      )}
      {(model.sessionId != null || model.remote || model.tty) && (
        <span className="exec-process-footer-group exec-process-footer-group--meta">
          {model.sessionId != null && (
            <span className="exec-process-footer-item">
              <span className="terminal-result-label">{t('toolCards.execProcess.session')}</span>
              <span className="terminal-result-value">#{model.sessionId}</span>
            </span>
          )}
          {model.remote && (
            <span className="exec-process-footer-item terminal-result-value">{t('toolCards.execProcess.remote')}</span>
          )}
          {model.tty && (
            <span className="exec-process-footer-item terminal-result-value">{t('toolCards.execProcess.tty')}</span>
          )}
        </span>
      )}
      {(model.exitCode != null || model.wallTimeSeconds != null) && (
        <span className="exec-process-footer-group exec-process-footer-group--metrics">
          {model.exitCode != null && (
            <span className={`terminal-exit-code ${model.exitCode === 0 ? 'success' : 'error'}`}>
              {t('toolCards.terminal.exitCode', { code: model.exitCode })}
            </span>
          )}
          {model.wallTimeSeconds != null && (
            <span className="terminal-execution-time">
              {t('toolCards.execProcess.wallTime', { seconds: model.wallTimeSeconds.toFixed(3) })}
            </span>
          )}
        </span>
      )}
    </div>
  );
}

export const ExecProcessToolCardView: React.FC<ExecProcessToolCardViewProps> = ({
  toolItem,
  model,
  onExpand,
}) => {
  const { t } = useTranslation('flow-chat');
  const status = toolItem.status || 'pending';
  const isParamsStreaming = Boolean(toolItem.isParamsStreaming);
  const progressLogs = useMemo(() => readProgressLogs(toolItem), [toolItem]);
  const liveOutput = useMemo(() => {
    if (progressLogs.length > 0) {
      return progressLogs.join('');
    }
    const progressMessage = (toolItem as any)._progressMessage;
    return typeof progressMessage === 'string' ? progressMessage : '';
  }, [progressLogs, toolItem]);
  const isRunning = status === 'preparing' || status === 'streaming' || status === 'running' || status === 'receiving';
  const maxRows = isRunning ? EXEC_OUTPUT_STREAMING_MAX_ROWS : EXEC_OUTPUT_EXPANDED_MAX_ROWS;
  const toolId = toolItem.id ?? toolItem.toolCall?.id;
  const icon = <Terminal size={16} className="terminal-card-icon" />;

  const [isExpanded, setIsExpandedState] = useState(() => getInitialExpandedState(status));
  const previousStatusRef = useRef(status);
  const commandRef = useRef<HTMLElement | null>(null);
  const outputRendererRef = useRef<TerminalOutputRendererHandle | null>(null);
  const [isPrimaryTextTruncated, setIsPrimaryTextTruncated] = useState(false);
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const applyExecExpandedState = useCallback((
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
    applyExecExpandedState(!isExpanded, { reason: 'manual' });
  }, [applyExecExpandedState, isExpanded]);

  useEffect(() => {
    const prevStatus = previousStatusRef.current;
    previousStatusRef.current = status;
    if (prevStatus === status) {
      return;
    }

    const nextExpanded = getAutoExpandedStateForStatus(status);
    if (nextExpanded !== null) {
      applyExecExpandedState(nextExpanded, { reason: 'auto' });
    }
  }, [applyExecExpandedState, status]);

  const updatePrimaryTextTruncation = useCallback(() => {
    const element = commandRef.current;
    if (!element) {
      setIsPrimaryTextTruncated(false);
      return;
    }
    const nextValue = element.scrollWidth - element.clientWidth > 1;
    setIsPrimaryTextTruncated((prev) => (prev === nextValue ? prev : nextValue));
  }, []);

  useEffect(() => {
    const element = commandRef.current;
    if (!element) {
      setIsPrimaryTextTruncated(false);
      return;
    }

    const frameId = window.requestAnimationFrame(updatePrimaryTextTruncation);
    const resizeObserver = typeof ResizeObserver !== 'undefined'
      ? new ResizeObserver(updatePrimaryTextTruncation)
      : null;
    resizeObserver?.observe(element);
    if (element.parentElement) {
      resizeObserver?.observe(element.parentElement);
    }
    window.addEventListener('resize', updatePrimaryTextTruncation);

    return () => {
      window.cancelAnimationFrame(frameId);
      resizeObserver?.disconnect();
      window.removeEventListener('resize', updatePrimaryTextTruncation);
    };
  }, [model.primaryText, updatePrimaryTextTruncation]);

  const handleCardClick = useCallback((event: React.MouseEvent) => {
    const target = event.target as HTMLElement;
    if (target.closest('.tool-card-header-actions, .terminal-action-btn')) {
      return;
    }
    toggleExpanded();
  }, [toggleExpanded]);

  const completedDurationMs =
    formatSecondsAsMs(model.wallTimeSeconds) ?? toolItem.toolResult?.duration_ms ?? toolItem.durationMs;
  const timeoutMs = typeof toolItem.toolCall?.input?.yield_time_ms === 'number' && toolItem.toolCall.input.yield_time_ms > 0
    ? toolItem.toolCall.input.yield_time_ms
    : undefined;

  const renderPrimaryText = (variant: 'default' | 'compact' = 'default') => (
    <ToolCommandPreview
      ref={commandRef}
      as={variant === 'compact' ? 'span' : 'code'}
      command={model.primaryText}
      emptyText={model.emptyText}
      className={
        variant === 'compact'
          ? 'terminal-command-compact tool-command-preview--compact'
          : 'terminal-command'
      }
      tooltipContent={model.primaryText && isPrimaryTextTruncated ? model.primaryText : undefined}
    />
  );

  const renderCopyButton = () => (
    <ToolCardCopyAction
      className="terminal-action-btn copy-command-btn"
      getText={() => model.copyText}
      disabled={model.copyDisabled}
      tooltip={t('toolCards.execProcess.copyPrimary')}
      copiedTooltip={t('toolCards.execProcess.primaryCopied')}
      successMessage={t('toolCards.execProcess.primaryCopied')}
      failureMessage={t('toolCards.execProcess.copyPrimaryFailed')}
      ariaLabel={t('toolCards.execProcess.copyPrimary')}
      showSuccessNotification={false}
    />
  );

  const getOutputText = useCallback(() => {
    if (status === 'completed') {
      return formatSessionViewPreviewText(model.resultOutput);
    }

    if (status === 'cancelled') {
      return liveOutput;
    }

    if (liveOutput && isRunning) {
      return liveOutput;
    }

    return '';
  }, [isRunning, liveOutput, model.resultOutput, status]);

  const getVisibleOutputText = useCallback(() => {
    return outputRendererRef.current?.getVisibleText() ?? getOutputText();
  }, [getOutputText]);

  const renderCopyOutputButton = () => (
    <ToolCardCopyAction
      className="terminal-action-btn copy-command-btn exec-process-copy-output-btn"
      getText={getVisibleOutputText}
      disabled={!getOutputText().trim()}
      tooltip={t('toolCards.execProcess.copyOutput')}
      copiedTooltip={t('toolCards.execProcess.outputCopied')}
      successMessage={t('toolCards.execProcess.outputCopied')}
      failureMessage={t('toolCards.execProcess.copyOutputFailed')}
      ariaLabel={t('toolCards.execProcess.copyOutput')}
      showSuccessNotification={false}
    />
  );

  const renderOutputWithCopyAction = (
    output: string,
    options?: { formatSessionPreview?: boolean },
  ) => (
    <div className="exec-process-output-copy-region">
      <div className="exec-process-output-copy-actions">
        {renderCopyOutputButton()}
      </div>
      <TerminalOutputRenderer
        ref={outputRendererRef}
        content={options?.formatSessionPreview ? formatSessionViewPreviewText(output) : output}
        className="terminal-xterm-output"
        maxRows={maxRows}
      />
    </div>
  );

  const renderTimeoutIndicator = () => (
    <span className="terminal-header-duration">
      <ToolTimeoutIndicator
        startTime={toolItem.startTime}
        isRunning={isRunning}
        timeoutMs={timeoutMs}
        showControls={false}
        completedDurationMs={status === 'completed' ? completedDurationMs : undefined}
        completedStatus={
          status === 'completed'
            ? model.exitCode === 0 || model.exitCode == null ? 'success' : 'error'
            : status === 'error' ? 'error' : status === 'cancelled' ? 'cancelled' : undefined
        }
      />
    </span>
  );

  const renderHeaderExtra = () => (
    <span className="terminal-header-extra">
      {renderTimeoutIndicator()}
      <ToolCardHeaderActions className="terminal-header-actions">
        {renderCopyButton()}
      </ToolCardHeaderActions>
    </span>
  );

  const renderLoadingStatusIcon = () => (
    isRunning ? <DotMatrixLoader size="medium" /> : null
  );

  const renderExpandedContent = () => {
    if (!isExpanded) {
      return null;
    }

    if (status === 'completed') {
      return (
        <div className="terminal-result-container">
          {model.resultOutput ? (
            <div className="terminal-result-output">
              {renderOutputWithCopyAction(model.resultOutput, { formatSessionPreview: true })}
            </div>
          ) : model.resultNoticeText ? (
            <div className="terminal-execution-output terminal-waiting exec-process-result-notice">
              <span className="waiting-text">{model.resultNoticeText}</span>
            </div>
          ) : (
            <div className="terminal-execution-output terminal-waiting exec-process-empty-output">
              <span className="waiting-text">{model.noOutputText}</span>
            </div>
          )}
          {renderFooter(model, t)}
        </div>
      );
    }

    if (status === 'cancelled' && liveOutput) {
      return (
        <div className="terminal-result-container cancelled">
          <div className="terminal-result-output">
            {renderOutputWithCopyAction(liveOutput)}
          </div>
          <div className="terminal-result-footer">
            <span className="terminal-cancelled-text">{t('toolCards.terminal.commandInterrupted')}</span>
          </div>
        </div>
      );
    }

    if (liveOutput && isRunning) {
      return (
        <div className="terminal-execution-output">
          {renderOutputWithCopyAction(liveOutput)}
        </div>
      );
    }

    if (isRunning || isParamsStreaming) {
      return (
        <div className="terminal-execution-output terminal-waiting">
          <span className="waiting-text">{isParamsStreaming ? t('toolCards.terminal.receivingParams') : model.waitingText}</span>
        </div>
      );
    }

    return null;
  };

  const renderErrorContent = () => {
    if (status !== 'error') {
      return null;
    }

    return (
      <div className="error-content">
        <div className="error-message">
          {toolItem.toolResult?.error || t('toolCards.terminal.executionFailed')}
        </div>
      </div>
    );
  };

  const renderHeader = () => (
    <ToolCardHeader
      icon={icon}
      action={model.actionLabel}
      content={renderPrimaryText()}
      extra={renderHeaderExtra()}
      statusIcon={renderLoadingStatusIcon()}
    />
  );

  const renderCompactHeader = () => (
    <CompactToolCardHeader
      icon={<ToolCardStatusSlot status={status} toolIcon={icon} defaultIcon="tool" />}
      action={model.actionLabel}
      content={
        <span className="terminal-compact-content">
          {renderPrimaryText('compact')}
          <span className="compact-extra-on-hover terminal-hover-actions">
            {renderTimeoutIndicator()}
            <ToolCardHeaderActions className="terminal-header-actions">
              {renderCopyButton()}
            </ToolCardHeaderActions>
          </span>
        </span>
      }
    />
  );

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      {isExpanded ? (
        <BaseToolCard
          status={status}
          isExpanded={isExpanded}
          onClick={handleCardClick}
          className="terminal-tool-card exec-process-tool-card"
          header={renderHeader()}
          expandedContent={renderExpandedContent()}
          errorContent={renderErrorContent()}
          isFailed={status === 'error'}
        />
      ) : (
        <CompactToolCard
          status={status}
          isExpanded={false}
          onClick={handleCardClick}
          className="terminal-tool-card terminal-tool-card--compact-collapsed exec-process-tool-card"
          clickable
          header={renderCompactHeader()}
        />
      )}
    </div>
  );
};

export default ExecProcessToolCardView;
