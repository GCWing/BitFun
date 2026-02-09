/**
 * Tool card for Mermaid interactive diagrams.
 */

import React, { useCallback } from 'react';
import { CheckCircle, Eye, Network } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { CubeLoading } from '../../component-library';
import type { ToolCardProps, FlowToolItem } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { flowChatStore } from '../store/FlowChatStore';
import { createLogger } from '@/shared/utils/logger';
import './MermaidInteractiveDisplay.scss';

const log = createLogger('MermaidInteractiveDisplay');

/**
 * Read the latest toolCall.input from flowChatStore for a given tool item.
 * This ensures we always get the most up-to-date data (e.g. after mermaid code fix),
 * even if the component has not re-rendered with new props.
 */
function getLatestToolInput(toolItemId: string, toolCallId: string): any | null {
  try {
    const state = flowChatStore.getState();
    const activeSessionId = state.activeSessionId;
    if (!activeSessionId) return null;

    const session = state.sessions.get(activeSessionId);
    if (!session) return null;

    for (const turn of session.dialogTurns) {
      for (const round of turn.modelRounds) {
        const item = round.items.find(
          (it: any) => it.type === 'tool' && (it.toolCall?.id === toolCallId || it.id === toolItemId)
        ) as FlowToolItem | undefined;
        if (item) {
          return item.toolCall?.input ?? null;
        }
      }
    }
  } catch {
    // Fallback to props data
  }
  return null;
}

export const MermaidInteractiveDisplay: React.FC<ToolCardProps> = ({
  toolItem
}) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolCall, toolResult } = toolItem;

  const getInputData = () => {
    if (!toolCall?.input) return null;
    
    const isEarlyDetection = toolCall.input._early_detection === true;
    const isPartialParams = toolCall.input._partial_params === true;
    
    if (isEarlyDetection || isPartialParams) {
      return null;
    }
    
    const inputKeys = Object.keys(toolCall.input).filter(key => !key.startsWith('_'));
    if (inputKeys.length === 0) return null;
    
    return toolCall.input;
  };

  const getResultData = () => {
    if (!toolResult?.result) return null;
    
    try {
      if (typeof toolResult.result === 'string') {
        return JSON.parse(toolResult.result);
      }
      return toolResult.result;
    } catch (e) {
      log.error('Failed to parse result', e);
      return null;
    }
  };

  const handleOpenMermaid = useCallback(() => {
    // Read the latest data from store first, fallback to props if unavailable.
    const latestInput = getLatestToolInput(toolItem.id, toolCall.id);
    const inputData = latestInput || getInputData();
    const resultData = getResultData();
    
    if (!inputData) {
      return;
    }

    const mermaidCode = inputData.mermaid_code || '';
    const title = inputData.title || t('toolCards.diagram.mermaidInteractive');
    const mode = inputData.mode || 'interactive';
    const nodeMetadata = inputData.node_metadata || {};
    const highlights = inputData.highlights || { executed: [], failed: [], current: null };
    const allowModeSwitch = inputData.allow_mode_switch !== false;
    const enableNavigation = inputData.enable_navigation !== false;
    const enableTooltips = inputData.enable_tooltips !== false;

    const duplicateCheckKey = `mermaid-interactive-${toolCall.id}`;
    const eventData = {
      type: 'mermaid-editor',
      title: title,
      data: {
        mermaid_code: mermaidCode,
        sourceCode: mermaidCode,
        mode: mode,
        allow_mode_switch: allowModeSwitch,
        session_id: resultData?.panel_id || `mermaid-${Date.now()}`,
        interactive_config: {
          node_metadata: nodeMetadata,
          highlights: highlights,
          enable_navigation: enableNavigation,
          enable_tooltips: enableTooltips
        },
        // Source tracking for write-back after edits/fixes.
        _source: {
          type: 'tool-call',
          toolCallId: toolCall.id,
          toolItemId: toolItem.id,
        }
      },
      metadata: {
        duplicateCheckKey,
        fromTool: true,
        toolName: 'MermaidInteractive'
      },
      checkDuplicate: true,
      duplicateCheckKey,
      replaceExisting: true
    };

    window.dispatchEvent(new CustomEvent('expand-right-panel'));

    setTimeout(() => {
      window.dispatchEvent(new CustomEvent('agent-create-tab', {
        detail: eventData
      }));
    }, 100);
  }, [toolCall, toolResult]);

  const inputData = getInputData();

  const getChartInfo = () => {
    if (!inputData) return null;

    const mermaidCode = inputData.mermaid_code || '';
    const nodeMetadata = inputData.node_metadata || {};
    const mode = inputData.mode || 'interactive';

    const nodeCount = Object.keys(nodeMetadata).length;
    
    return {
      mode,
      nodeCount,
      hasMetadata: nodeCount > 0,
      codeLines: mermaidCode.split('\n').length
    };
  };

  const chartInfo = getChartInfo();
  const title = inputData?.title || t('toolCards.diagram.mermaidInteractive');

  if ((status as string) === 'error') {
    return null;
  }

  const isClickable = status === 'completed';
  const isLoading = status === 'running' || status === 'streaming' || status === 'pending';

  const renderToolIcon = () => {
    return <Network size={16} />;
  };

  const renderStatusIcon = () => {
    if (isLoading) {
      return <CubeLoading size="small" />;
    }
    if (status === 'completed') {
      return <CheckCircle className="icon-completed" size={14} />;
    }
    return null;
  };

  const renderHeader = () => (
    <ToolCardHeader
      icon={renderToolIcon()}
      iconClassName="mermaid-icon"
      action={t('toolCards.diagram.interactive')}
      content={
        <span className="mermaid-title-content">{title}</span>
      }
      extra={
        <>
          {status === 'completed' && (
            <div className="mermaid-view-icon">
              <Eye size={14} />
            </div>
          )}
          {isLoading && (
            <span className="mermaid-status-text">
              {(status === 'running' || status === 'streaming') && t('toolCards.diagram.creating')}
              {status === 'pending' && t('toolCards.diagram.preparing')}
            </span>
          )}
        </>
      }
      statusIcon={renderStatusIcon()}
    />
  );

  return (
    <BaseToolCard
      status={status}
      isExpanded={false}
      onClick={isClickable ? handleOpenMermaid : undefined}
      className={`mermaid-interactive-card ${isClickable ? 'clickable' : ''}`}
      header={renderHeader()}
    />
  );
};
