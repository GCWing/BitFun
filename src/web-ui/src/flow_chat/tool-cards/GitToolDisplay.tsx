/**
 * Display component for the Git tool.
 */

import React, { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { GitToolCard } from '@bitfun/ui/flow-chat';
import { ToolCardCopyAction } from './ToolCardCopyAction';
import { createLogger } from '@/shared/utils/logger';
import { useToolCardHeightContract } from './useToolCardHeightContract';

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
}) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolCall, toolResult } = toolItem;
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

  const handleCardClick = useCallback(() => {
    if (hasOutput || isFailed) {
      toggleExpanded();
    }
  }, [hasOutput, isFailed, toggleExpanded]);
  const footerItems = [
    resultData?.working_directory?.trim() ? {
      grow: true,
      label: t('toolCards.terminal.workingDirectory'),
      monospace: true,
      value: resultData.working_directory,
    } : null,
    resultData?.exit_code !== undefined ? {
      monospace: true,
      tone: resultData.exit_code === 0 ? 'success' as const : 'danger' as const,
      value: t('toolCards.git.exitCode', { code: resultData.exit_code }),
    } : null,
    resultData?.execution_time_ms !== undefined ? {
      monospace: true,
      value: resultData.execution_time_ms >= 1000
        ? `${(resultData.execution_time_ms / 1000).toFixed(2)}s`
        : `${resultData.execution_time_ms}ms`,
    } : null,
  ].filter((item): item is NonNullable<typeof item> => Boolean(item));

  const errorMeta = inputData?.operation
    ? `${t('toolCards.git.operation', { op: inputData.operation })}${inputData.args ? ` | ${t('toolCards.git.args', { args: inputData.args })}` : ''}`
    : undefined;

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''} data-bf-adapter="git-tool">
      <GitToolCard
        status={isFailed ? 'error' : status}
        isExpanded={isExpanded}
        onToggle={hasOutput || isFailed ? handleCardClick : undefined}
        action={isFailed ? t('toolCards.git.commandFailed') : `${t('toolCards.git.title')}:`}
        command={commandText || t('toolCards.terminal.noCommand')}
        headerActions={(
          <ToolCardCopyAction
            getText={getCopyCommandText}
            tooltip={t('toolCards.git.copyCommand')}
            copiedTooltip={t('toolCards.git.commandCopied')}
            successMessage={t('toolCards.git.commandCopied')}
            failureMessage={t('toolCards.git.copyCommandFailed')}
            ariaLabel={t('toolCards.git.copyCommand')}
          />
        )}
        loading={isLoading}
        statusSummary={isFailed ? t('toolCards.git.failed') : outputSummary}
        statusTone={isFailed ? 'danger' : hasWarning ? 'warning' : 'neutral'}
        stdout={resultData?.stdout?.trim() || undefined}
        stderr={resultData?.stderr?.trim() || undefined}
        stderrLabel={resultData?.stderr
          ? resultData.success ? t('toolCards.git.warning') : t('toolCards.git.error')
          : undefined}
        stderrTone={resultData?.success ? 'warning' : 'danger'}
        footerItems={footerItems}
        error={!resultData && isFailed ? getErrorMessage() : undefined}
        errorMeta={errorMeta}
      />
    </div>
  );
};
