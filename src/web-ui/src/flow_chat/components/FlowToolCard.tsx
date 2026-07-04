/**
 * Streaming tool card component.
 * Renders a dedicated card based on tool type.
 */

import React from 'react';
import { getToolCardComponent } from '../tool-cards';
import { getToolCardConfig } from '../tool-cards/toolCardMetadata';
import type { FlowToolItem, ToolCardDisplayContext, ToolRejectOptions } from '../types/flow-chat';
import { createLogger } from '@/shared/utils/logger';
import { FlowToolCardErrorBoundary } from './FlowToolCardErrorBoundary';
import { useTranslation } from 'react-i18next';
import { getToolInterruptionNote } from '../utils/toolInterruption';
import { ToolApprovalBar } from './ToolApprovalBar';

const log = createLogger('FlowToolCard');

interface FlowToolCardProps {
  toolItem: FlowToolItem;
  onConfirm?: (toolId: string, updatedInput?: any, permissionOptionId?: string, approve?: boolean) => void;
  onReject?: (toolId: string, options?: ToolRejectOptions) => void;
  onOpenInEditor?: (filePath: string) => void;
  onOpenInPanel?: (panelType: string, data: any) => void;
  onExpand?: (toolId: string) => void;
  sessionId?: string;
  turnId?: string;
  className?: string;
  displayContext?: ToolCardDisplayContext;
}

export const FlowToolCard: React.FC<FlowToolCardProps> = React.memo(({
  toolItem,
  onConfirm,
  onReject,
  onOpenInEditor,
  onOpenInPanel,
  onExpand,
  sessionId,
  className = '',
  displayContext = 'default',
}) => {
  const { t } = useTranslation('flow-chat');
  const config = getToolCardConfig(toolItem.toolName);
  const CardComponent = getToolCardComponent(toolItem.toolName);
  const interruptionNote = getToolInterruptionNote(toolItem, t);
  const cardHandlesInterruptionNote = toolItem.toolName === 'Task';
  const toolCardTestId =
    toolItem.toolName === 'Bash'
      ? 'chat-shell-tool-card'
      : toolItem.toolName === 'WebFetch'
        ? 'chat-browser-tool-card'
        : undefined;

  const handleConfirm = React.useCallback((updatedInput?: any, permissionOptionId?: string, approve?: boolean) => {
    log.debug('handleConfirm called', {
      toolId: toolItem.id,
      toolName: toolItem.toolName,
      hasUpdatedInput: updatedInput !== undefined,
      updatedInputKeys: updatedInput ? Object.keys(updatedInput) : [],
      hasPermissionOption: Boolean(permissionOptionId),
      approve
    });
    onConfirm?.(toolItem.id, updatedInput, permissionOptionId, approve);
  }, [toolItem.id, toolItem.toolName, onConfirm]);

  const handleReject = React.useCallback((options?: ToolRejectOptions) => {
    onReject?.(toolItem.id, options);
  }, [toolItem.id, onReject]);

  const handleExpand = React.useCallback(() => {
    onExpand?.(toolItem.id);
  }, [toolItem.id, onExpand]);

  return (
    <div
      className={`flow-tool-card-wrapper ${className}`}
      data-testid={toolCardTestId}
      data-tool-name={toolItem.toolName}
      data-tool-card-id={toolItem.id}
    >
      <FlowToolCardErrorBoundary
        toolItem={toolItem}
        displayName={config.displayName}
        sessionId={sessionId}
      >
        <CardComponent
          toolItem={toolItem}
          config={config}
          interruptionNote={interruptionNote}
          onOpenInEditor={onOpenInEditor}
          onOpenInPanel={onOpenInPanel}
          onExpand={handleExpand}
          sessionId={sessionId}
          displayContext={displayContext}
        />
      </FlowToolCardErrorBoundary>
      <ToolApprovalBar
        toolItem={toolItem}
        onConfirm={handleConfirm}
        onReject={handleReject}
      />
      {interruptionNote && !cardHandlesInterruptionNote && (
        <div className="flow-tool-card-note" role="note">
          {interruptionNote}
        </div>
      )}
    </div>
  );
}, (prevProps, nextProps) => {
  // Compare streaming parameters and progress messages to avoid stale renders.
  const prevProgress = (prevProps.toolItem as any)._progressMessage;
  const nextProgress = (nextProps.toolItem as any)._progressMessage;
  const prevProgressLogs = (prevProps.toolItem as any)._progressLogs;
  const nextProgressLogs = (nextProps.toolItem as any)._progressLogs;
  
  return (
    prevProps.toolItem.id === nextProps.toolItem.id &&
    prevProps.sessionId === nextProps.sessionId &&
    prevProps.toolItem.status === nextProps.toolItem.status &&
    prevProps.toolItem.interruptionReason === nextProps.toolItem.interruptionReason &&
    prevProps.toolItem.terminalSessionId === nextProps.toolItem.terminalSessionId &&
    prevProps.toolItem.userConfirmed === nextProps.toolItem.userConfirmed &&
    prevProps.toolItem.acpPermission === nextProps.toolItem.acpPermission &&
    prevProps.toolItem.isParamsStreaming === nextProps.toolItem.isParamsStreaming &&
    prevProps.toolItem.subagentSessionId === nextProps.toolItem.subagentSessionId &&
    prevProps.toolItem.subagentModelId === nextProps.toolItem.subagentModelId &&
    prevProps.toolItem.subagentModelAlias === nextProps.toolItem.subagentModelAlias &&
    prevProps.displayContext === nextProps.displayContext &&
    prevProgress === nextProgress &&
    prevProgressLogs === nextProgressLogs &&
    prevProps.toolItem.partialParams === nextProps.toolItem.partialParams &&
    prevProps.toolItem.toolResult === nextProps.toolItem.toolResult
  );
});
