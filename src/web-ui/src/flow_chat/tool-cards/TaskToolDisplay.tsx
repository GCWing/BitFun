/**
 * TaskTool card display component.
 */

import React, { useState, useEffect, useLayoutEffect, useCallback, useRef, useMemo } from 'react';
import {
  AlertTriangle,
  Split,
  ChevronRight,
} from 'lucide-react';

import { useTranslation } from 'react-i18next';
import { CubeLoading, Button } from '../../component-library';
import { Markdown } from '@/component-library/components/Markdown/Markdown';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard } from './BaseToolCard';
import { ToolCardIconSlot } from './ToolCardIconSlot';
import { ToolCardStatusIcon } from './ToolCardStatusIcon';
import { taskCollapseStateManager } from '../store/TaskCollapseStateManager';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { ToolTimeoutIndicator } from './ToolTimeoutIndicator';
import { getReviewerContextBySubagentId } from '@/shared/services/reviewTeamService';
import type { ReviewerContext } from '@/shared/services/reviewTeamService';
import './TaskToolDisplay.scss';
import './ModelThinkingDisplay.scss';

export const TaskToolDisplay: React.FC<ToolCardProps> = ({
  toolItem,
  interruptionNote,
  onConfirm,
  onReject,
  onOpenInPanel,
  sessionId
}) => {
  const { t } = useTranslation('flow-chat');
  const { t: tAgents } = useTranslation('scenes/agents');
  const { toolCall, toolResult, status, requiresConfirmation, userConfirmed } = toolItem;
  const toolId = toolItem.id ?? toolCall?.id;
  
  // Restore collapse state; default to collapsed until running.
  const [isExpanded, setIsExpanded] = useState(() => {
    const savedState = taskCollapseStateManager.getCollapsedOrUndefined(toolItem.id);
    if (savedState !== undefined) {
      return !savedState;
    }
    return false;
  });
  
  const isRunning = status === 'preparing' || status === 'streaming' || status === 'running';
  
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });
  
  const prevStatusRef = useRef(status);

  const updateCardExpandedState = useCallback((
    nextExpanded: boolean,
    reason: 'manual' | 'auto' = 'manual',
  ) => {
    if (nextExpanded !== isExpanded) {
      /* Sync before the next commit paints so subagent wrapper + task card merge in one frame. */
      taskCollapseStateManager.setCollapsed(toolItem.id, !nextExpanded);
    }
    applyExpandedState(isExpanded, nextExpanded, setIsExpanded, { reason });
  }, [applyExpandedState, isExpanded, toolItem.id]);

  useLayoutEffect(() => {
    const prevStatus = prevStatusRef.current;
    
    if (prevStatus !== status) {
      prevStatusRef.current = status;
      
      if (status === 'completed') {
        updateCardExpandedState(false, 'auto');
      } else if (isRunning) {
        updateCardExpandedState(true, 'auto');
      }
    }
  }, [isRunning, status, updateCardExpandedState]);
  
  useLayoutEffect(() => {
    taskCollapseStateManager.setCollapsed(toolItem.id, !isExpanded);
  }, [isExpanded, toolItem.id]);

  // Detect full-width characters for visual width estimation.
  const isFullWidth = (char: string) => {
    const code = char.charCodeAt(0);
    return (
      (code >= 0x4E00 && code <= 0x9FFF) ||
      (code >= 0x3400 && code <= 0x4DBF) ||
      (code >= 0xAC00 && code <= 0xD7AF) ||
      (code >= 0x3040 && code <= 0x309F) ||
      (code >= 0x30A0 && code <= 0x30FF) ||
      (code >= 0xFF00 && code <= 0xFFEF)
    );
  };

  // Truncate by visual width (full-width counts as 2).
  const truncateByVisualWidth = (str: string, maxWidth: number) => {
    let width = 0;
    let result = '';
    
    for (const char of str) {
      const charWidth = isFullWidth(char) ? 2 : 1;
      
      if (width + charWidth > maxWidth) {
        return result + '...';
      }
      
      width += charWidth;
      result += char;
    }
    
    return result;
  };

  const getTaskInput = () => {
    if (!toolCall?.input) return null;

    const isEarlyDetection = toolCall.input._early_detection === true;
    const isPartialParams = toolCall.input._partial_params === true;

    if (isEarlyDetection || isPartialParams) {
      return null;
    }

    const inputKeys = Object.keys(toolCall.input).filter(key => !key.startsWith('_'));
    if (inputKeys.length === 0) return null;

    const { description, prompt, subagent_type } = toolCall.input;
    const agentType = subagent_type || 'Not provided';

    // For built-in review-team reviewers, surface role context instead of
    // the raw prompt so internal directives stay private.
    const reviewerContext: ReviewerContext | null =
      agentType !== 'Not provided'
        ? getReviewerContextBySubagentId(agentType)
        : null;

    return {
      description: description || (prompt ? truncateByVisualWidth(prompt, 70) : 'Not provided'),
      prompt: prompt || 'Not provided',
      agentType,
      reviewerContext,
    };
  };

  const taskInput = getTaskInput();
  const hasRealPrompt = Boolean(
    taskInput && taskInput.prompt && taskInput.prompt !== 'Not provided',
  );
  const hasInterruptionNote = Boolean(interruptionNote);
  const needsConfirmation =
    requiresConfirmation && !userConfirmed && status !== 'completed';

  /* Prompt body: same scroll + Markdown shell as ModelThinkingDisplay. */
  const promptContentRef = useRef<HTMLDivElement>(null);
  const [promptScrollState, setPromptScrollState] = useState({
    hasScroll: false,
    atTop: true,
    atBottom: true,
  });

  const checkPromptScrollState = useCallback(() => {
    const el = promptContentRef.current;
    if (!el) return;
    setPromptScrollState({
      hasScroll: el.scrollHeight > el.clientHeight,
      atTop: el.scrollTop <= 5,
      atBottom: el.scrollTop + el.clientHeight >= el.scrollHeight - 5,
    });
  }, []);

  useEffect(() => {
    if (!isExpanded || !hasRealPrompt) return;
    const timer = setTimeout(checkPromptScrollState, 50);
    return () => clearTimeout(timer);
  }, [isExpanded, hasRealPrompt, taskInput?.prompt, checkPromptScrollState]);

  const isFailed =
    status === 'error' ||
    (toolResult != null &&
      'success' in toolResult &&
      toolResult.success === false);

  const handleCardClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (
      target.closest('.preview-toggle-btn') ||
      target.closest('.tool-actions') ||
      target.closest('.result-expand-toggle') ||
      target.closest('.task-header-rail__hit')
    ) {
      return;
    }

    if (isFailed) {
      return;
    }

    // Pause auto-scroll while the user toggles the card.
    updateCardExpandedState(!isExpanded);
  }, [isFailed, isExpanded, updateCardExpandedState]);

  const showHeaderExpandHint =
    !isFailed &&
    (hasInterruptionNote ||
      hasRealPrompt ||
      needsConfirmation ||
      Boolean(taskInput?.reviewerContext));

  const taskHeaderLine = useMemo(() => {
    const desc =
      (taskInput?.description || '').trim() || t('toolCards.taskDetailPanel.untitled');
    const raw = taskInput?.agentType;
    let agentTypeLabel: string;
    if (raw && raw !== 'Not provided') {
      const rc = taskInput?.reviewerContext;
      agentTypeLabel = rc
        ? tAgents(`reviewTeams.members.${rc.definitionKey}.funName`, {
            defaultValue: rc.roleName,
          })
        : raw;
    } else {
      agentTypeLabel = t('toolCards.taskTool.defaultAgentKind');
    }
    return t('toolCards.taskTool.headerLine', {
      agentType: agentTypeLabel,
      description: desc,
    });
  }, [taskInput, t, tAgents]);

  const openTaskDetailPanel = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      const panelData = { toolItem, taskInput, sessionId };
      const tabInfo = {
        type: 'task-detail',
        title: taskHeaderLine,
        data: panelData,
        metadata: { taskId: toolItem.id },
      };
      if (onOpenInPanel) {
        onOpenInPanel(tabInfo.type, tabInfo);
      } else {
        window.dispatchEvent(new CustomEvent('agent-create-tab', { detail: tabInfo }));
      }
    },
    [onOpenInPanel, sessionId, taskInput, toolItem, taskHeaderLine],
  );

  const renderToolIcon = () => {
    return <Split size={16} />;
  };

  const renderStatusIcon = () => {
    if (isRunning) {
      return <CubeLoading size="small" />;
    }
    return null;
  };

  const renderHeader = () => (
    <div className="task-header-wrapper">
      <ToolCardIconSlot
        icon={renderToolIcon()}
        iconClassName={`task-icon ${isRunning ? 'is-running' : ''}`}
        expandable={showHeaderExpandHint}
        affordanceKind="expand"
        isExpanded={isExpanded}
        onAffordanceClick={handleCardClick}
      />

      <div className="task-content-wrapper">
        <div className="task-body-columns">
          <div className="task-body-main">
            <div className={`task-header-main ${isFailed ? 'task-header-main--failed' : ''}`}>
              <span className="task-action">{taskHeaderLine}</span>
              <div className="task-header-meta">
                <ToolTimeoutIndicator
                  startTime={toolItem.startTime}
                  isRunning={isRunning}
                  timeoutMs={
                    typeof toolCall?.timeout_seconds === 'number' && toolCall.timeout_seconds > 0
                      ? toolCall.timeout_seconds * 1000
                      : typeof toolCall?.input?.timeout_seconds === 'number' && toolCall.input.timeout_seconds > 0
                      ? toolCall.input.timeout_seconds * 1000
                      : undefined
                  }
                  showControls={true}
                  subagentSessionId={toolItem.subagentSessionId}
                  completedDurationMs={
                    status === 'completed' && toolResult?.result?.duration
                      ? toolResult.result.duration
                      : undefined
                  }
                />
                {isFailed && (
                  <span className="task-failed-badge">{t('toolCards.taskTool.failed')}</span>
                )}
              </div>
            </div>
          </div>
          <div className="task-header-rail">
            <button
              type="button"
              className="task-header-rail__hit"
              onClick={openTaskDetailPanel}
              aria-label={t('toolCards.taskTool.openInPanel')}
              title={t('toolCards.taskTool.openInPanel')}
            />
            <div className="task-header-rail__visual" aria-hidden>
              <ChevronRight size={16} strokeWidth={2} absoluteStrokeWidth />
              <ToolCardStatusIcon icon={renderStatusIcon()} className="task-status-icon--rail" />
            </div>
          </div>
        </div>
      </div>
    </div>
  );

  const renderExpandedContent = () => {
    /* Failure only in header badge; do not keep prompt/confirm in expanded body. */
    if (isFailed) {
      return null;
    }

    const rc = taskInput?.reviewerContext;

    if (
      !hasInterruptionNote &&
      !hasRealPrompt &&
      !needsConfirmation &&
      !rc
    ) {
      return null;
    }

    return (
      <div className="task-expanded-content">
        {interruptionNote && (
          <>
            <div className="task-interruption-note" role="note">
              <AlertTriangle size={14} strokeWidth={2} aria-hidden />
              <span>{interruptionNote}</span>
            </div>
            {(hasRealPrompt || needsConfirmation || taskInput?.reviewerContext) && (
              <div className="task-interruption-divider" aria-hidden />
            )}
          </>
        )}
        {rc ? (
          <div className="task-reviewer-context">
            <div className="task-reviewer-context__role" style={{ color: rc.accentColor }}>
              {tAgents(`reviewTeams.members.${rc.definitionKey}.role`, {
                defaultValue: rc.roleName,
              })}
            </div>
            <div className="task-reviewer-context__description">
              {tAgents(`reviewTeams.members.${rc.definitionKey}.description`, {
                defaultValue: rc.description,
              })}
            </div>
            <ul className="task-reviewer-context__responsibilities">
              {rc.responsibilities.map((resp, idx) => (
                <li key={idx}>
                  {tAgents(`reviewTeams.members.${rc.definitionKey}.responsibilities.${idx}`, {
                    defaultValue: resp,
                  })}
                </li>
              ))}
            </ul>
          </div>
        ) : (
          hasRealPrompt && (
          <div
            className={`thinking-content-wrapper task-prompt-wrapper${promptScrollState.hasScroll ? ' has-scroll' : ''}${
              promptScrollState.atTop ? ' at-top' : ''
            }${promptScrollState.atBottom ? ' at-bottom' : ''}`}
          >
            <div
              ref={promptContentRef}
              className="thinking-content task-prompt-content expanded"
              onScroll={checkPromptScrollState}
            >
              <Markdown
                content={taskInput!.prompt}
                isStreaming={false}
                className="thinking-markdown task-prompt-markdown"
              />
            </div>
          </div>
          )
        )}
        {needsConfirmation && (
          <div className="tool-actions">
            <Button
              className="confirm-button"
              variant="primary"
              size="small"
              onClick={() => onConfirm?.(toolCall?.input)}
              disabled={status === 'streaming'}
            >
              {t('toolCards.taskTool.confirmDelegate')}
            </Button>
            <Button
              className="reject-button"
              variant="ghost"
              size="small"
              onClick={() => onReject?.()}
              disabled={status === 'streaming'}
            >
              {t('toolCards.taskTool.cancel')}
            </Button>
          </div>
        )}
      </div>
    );
  };

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <BaseToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleCardClick}
        className="task-tool-display"
        header={renderHeader()}
        expandedContent={renderExpandedContent()}
        headerExpandAffordance={showHeaderExpandHint}
        isFailed={isFailed}
        requiresConfirmation={requiresConfirmation && !userConfirmed}
      />
    </div>
  );
};
