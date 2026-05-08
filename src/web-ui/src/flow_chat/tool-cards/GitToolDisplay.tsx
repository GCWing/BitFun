/**
 * Display component for the Git tool.
 */

import React, { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { GitBranch, Check, X, AlertTriangle } from 'lucide-react';
import { CubeLoading, IconButton } from '../../component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { ToolCardCopyAction, ToolCardHeaderActions } from './ToolCardHeaderActions';
import { ToolCommandPreview } from './ToolCommandPreview';
import { createLogger } from '@/shared/utils/logger';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import './GitToolDisplay.scss';

const log = createLogger('GitToolDisplay');

interface GitToolInput {
  operation?: string;
  args?: string;
  working_directory?: string;
  timeout?: number;
}

interface GitToolResultData {
  success?: boolean;
  exit_code?: number;
  stdout?: string;
  stderr?: string;
  execution_time_ms?: number;
  working_directory?: string;
  command?: string;
  operation?: string;
  timestamp?: string;
}

export const GitToolDisplay: React.FC<ToolCardProps> = ({
  toolItem,
  onConfirm,
  onReject
}) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolCall, toolResult, requiresConfirmation, userConfirmed } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);
  const toolId = toolItem.id ?? toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const getInputData = (): GitToolInput | null => {
    if (!toolCall?.input) return null;
    
    const isEarlyDetection = toolCall.input._early_detection === true;
    const isPartialParams = toolCall.input._partial_params === true;
    
    if (isEarlyDetection || isPartialParams) {
      return null;
    }
    
    return toolCall.input as GitToolInput;
  };

  const getResultData = (): GitToolResultData | null => {
    if (!toolResult?.result) return null;
    
    try {
      if (typeof toolResult.result === 'string') {
        return JSON.parse(toolResult.result);
      }
      return toolResult.result as GitToolResultData;
    } catch (e) {
      log.error('Failed to parse result', e);
      return null;
    }
  };

  const inputData = getInputData();
  const resultData = getResultData();

  const getCommandDisplay = () => {
    if (resultData?.command) return resultData.command;
    if (!inputData?.operation) return 'git';
    
    let cmd = `git ${inputData.operation}`;
    if (inputData.args) {
      cmd += ` ${inputData.args}`;
    }
    return cmd;
  };

  const getOutputSummary = () => {
    if (!resultData) return null;
    
    const stdout = resultData.stdout?.trim() || '';
    const stderr = resultData.stderr?.trim() || '';
    
    if (!stdout && !stderr) return t('toolCards.git.noOutput');
    
    const output = stdout || stderr;
    const firstLine = output.split('\n')[0];
    if (firstLine.length > 60) {
      return firstLine.substring(0, 60) + '...';
    }
    return firstLine;
  };

  const outputSummary = getOutputSummary();
  const hasOutput = resultData && (resultData.stdout || resultData.stderr);
  const commandText = getCommandDisplay();
  
  const isLoading = status === 'preparing' || status === 'streaming' || status === 'running';

  const isFailed = status === 'error' || (resultData && resultData.exit_code !== 0);

  const hasWarning = resultData && resultData.success && resultData.stderr;

  const toggleExpanded = useCallback(() => {
    applyExpandedState(isExpanded, !isExpanded, setIsExpanded);
  }, [applyExpandedState, isExpanded]);

  const getCopyCommandText = useCallback(() => commandText, [commandText]);

  const getErrorMessage = () => {
    if (toolResult && 'error' in toolResult) {
      return toolResult.error;
    }
    if (resultData?.stderr) {
      return resultData.stderr;
    }
    return t('toolCards.git.executionFailed');
  };

  const handleCardClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest('.tool-card-header-actions, .git-action-buttons')) {
      return;
    }
    
    if (hasOutput || isFailed) {
      toggleExpanded();
    }
  }, [hasOutput, isFailed, toggleExpanded]);

  const renderStatusIcon = () => {
    if (isLoading) {
      return <CubeLoading size="small" />;
    }
    if (status === 'completed' && !isFailed && hasWarning) {
      return <AlertTriangle className="icon-warning" size={14} />;
    }
    return null;
  };

  const renderHeader = () => (
    <CompactToolCardHeader
      icon={<GitBranch size={16} className="git-card-icon" />}
      showDivider
      action={isFailed ? t('toolCards.git.commandFailed') : undefined}
      content={
        <span className="git-tool-info">
          <ToolCommandPreview
            command={commandText}
            emptyText={t('toolCards.terminal.noCommand')}
            className="git-command-preview tool-command-preview--compact"
          />
        </span>
      }
      extra={
        <span className="git-header-extra">
          {!isFailed && outputSummary && status === 'completed' && (
            <span className="output-summary">
              {outputSummary}
            </span>
          )}

          {isFailed && (
            <span className="error-indicator">
              <span className="error-text">{t('toolCards.git.failed')}</span>
            </span>
          )}

          <ToolCardHeaderActions className="git-action-buttons">
            <ToolCardCopyAction
              className="git-copy-btn"
              getText={getCopyCommandText}
              tooltip={t('toolCards.git.copyCommand', { defaultValue: 'Copy git command' })}
              copiedTooltip={t('toolCards.git.commandCopied', { defaultValue: 'Git command copied' })}
              successMessage={t('toolCards.git.commandCopied', { defaultValue: 'Git command copied' })}
              failureMessage={t('toolCards.git.copyCommandFailed', { defaultValue: 'Failed to copy git command' })}
              ariaLabel={t('toolCards.git.copyCommand', { defaultValue: 'Copy git command' })}
            />

            {requiresConfirmation && !userConfirmed && status !== 'completed' && (
              <>
              <IconButton
                className="tool-card-header-action git-confirm-btn"
                variant="success"
                size="xs"
                onClick={(e) => {
                  e.stopPropagation();
                  onConfirm?.(toolCall?.input);
                }}
                disabled={status === 'streaming'}
                tooltip={t('toolCards.git.confirmExecute')}
              >
                <Check size={12} />
              </IconButton>
              <IconButton
                className="tool-card-header-action git-reject-btn"
                variant="danger"
                size="xs"
                onClick={(e) => {
                  e.stopPropagation();
                  onReject?.();
                }}
                disabled={status === 'streaming'}
                tooltip={t('toolCards.git.cancel')}
              >
                <X size={12} />
              </IconButton>
              </>
            )}
          </ToolCardHeaderActions>
        </span>
      }
      rightStatusIcon={renderStatusIcon()}
    />
  );

  const renderExpandedContent = () => {
    if (!resultData) return null;

    const { stdout, stderr, exit_code, execution_time_ms, working_directory } = resultData;
    const hasStdout = Boolean(stdout?.trim());
    const hasStderr = Boolean(stderr?.trim());
    const showFooter =
      exit_code !== undefined ||
      execution_time_ms !== undefined ||
      Boolean(working_directory?.trim());

    return (
      <div className="git-result-container">
        {(hasStdout || hasStderr) && (
          <div className="git-result-output">
            {hasStdout && <pre className="git-output-block git-output-stdout">{stdout}</pre>}
            {hasStderr && (
              <div className="git-stderr-block">
                <div className="git-output-label">
                  {resultData.success ? t('toolCards.git.warning') : t('toolCards.git.error')}
                </div>
                <pre
                  className={`git-output-block ${resultData.success ? 'git-output-warning' : 'git-output-stderr'}`}
                >
                  {stderr}
                </pre>
              </div>
            )}
          </div>
        )}

        {showFooter && (
          <div className="git-result-footer">
            {working_directory?.trim() && (
              <>
                <span className="git-result-label">{t('toolCards.terminal.workingDirectory')}</span>
                <span className="git-result-value" title={working_directory}>
                  {working_directory}
                </span>
              </>
            )}
            {exit_code !== undefined && (
              <span className={`git-exit-code ${exit_code === 0 ? 'success' : 'error'}`}>
                {t('toolCards.git.exitCode', { code: exit_code })}
              </span>
            )}
            {execution_time_ms !== undefined && (
              <span className="git-execution-time">
                {execution_time_ms >= 1000
                  ? `${(execution_time_ms / 1000).toFixed(2)}s`
                  : `${execution_time_ms}ms`}
              </span>
            )}
          </div>
        )}
      </div>
    );
  };

  const renderErrorContent = () => (
    <div className="error-content">
      <div className="error-message">{getErrorMessage()}</div>
      {inputData?.operation && (
        <div className="error-meta">
          <span className="error-operation">{t('toolCards.git.operation', { op: inputData.operation })}</span>
          {inputData.args && (
            <>
              <span className="error-separator">|</span>
              <span className="error-args">{t('toolCards.git.args', { args: inputData.args })}</span>
            </>
          )}
        </div>
      )}
    </div>
  );

  /** Failure is summarized in the header; full details live here and only show when expanded. */
  const renderDetailsWhenExpanded = (): React.ReactNode => {
    if (resultData) {
      return renderExpandedContent();
    }
    if (isFailed) {
      return renderErrorContent();
    }
    return null;
  };

  const expandedContent = isExpanded
    ? renderDetailsWhenExpanded()
    : null;

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <CompactToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleCardClick}
        className="git-tool-display compact-tool-card-wrapper--dense-command"
        clickable
        header={renderHeader()}
        expandedContent={expandedContent}
      />
    </div>
  );
};
