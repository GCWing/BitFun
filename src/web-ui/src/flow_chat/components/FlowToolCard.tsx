/**
 * Streaming tool card component.
 * Renders a dedicated card based on tool type.
 */

import React from 'react';
import { getToolCardConfig, getToolCardComponent } from '../tool-cards';
import type { FlowToolItem } from '../types/flow-chat';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('FlowToolCard');

/**
 * When the primary model is multimodal, `view_image` returns `{ mode: "attached_to_primary_model" }`
 * instead of a textual analysis. In that case the tool card is pure noise and should be hidden.
 */
function isViewImageAttachedMode(toolItem: FlowToolItem): boolean {
  if (toolItem.toolName !== 'view_image') return false;
  const raw = toolItem.toolResult?.result as Record<string, unknown> | undefined;
  const mode =
    raw?.mode ??
    (raw?.result as Record<string, unknown> | undefined)?.mode ??
    (raw?.data as Record<string, unknown> | undefined)?.mode;
  return mode === 'attached_to_primary_model';
}

interface FlowToolCardProps {
  toolItem: FlowToolItem;
  onConfirm?: (toolId: string, updatedInput?: any) => void;
  onReject?: (toolId: string) => void;
  onOpenInEditor?: (filePath: string) => void;
  onOpenInPanel?: (panelType: string, data: any) => void;
  onExpand?: (toolId: string) => void;
  sessionId?: string;
  className?: string;
}

export const FlowToolCard: React.FC<FlowToolCardProps> = React.memo(({
  toolItem,
  onConfirm,
  onReject,
  onOpenInEditor,
  onOpenInPanel,
  onExpand,
  sessionId,
  className = ''
}) => {
  if (isViewImageAttachedMode(toolItem)) {
    return null;
  }

  const config = getToolCardConfig(toolItem.toolName);
  const CardComponent = getToolCardComponent(toolItem.toolName);

  const handleConfirm = React.useCallback((updatedInput?: any) => {
    log.debug('handleConfirm called', {
      toolId: toolItem.id,
      toolName: toolItem.toolName,
      hasUpdatedInput: updatedInput !== undefined,
      updatedInputKeys: updatedInput ? Object.keys(updatedInput) : []
    });
    onConfirm?.(toolItem.id, updatedInput);
  }, [toolItem.id, toolItem.toolName, onConfirm]);

  const handleReject = React.useCallback(() => {
    onReject?.(toolItem.id);
  }, [toolItem.id, onReject]);

  const handleExpand = React.useCallback(() => {
    onExpand?.(toolItem.id);
  }, [toolItem.id, onExpand]);

  return (
    <div className={`flow-tool-card-wrapper ${className}`}>
      <CardComponent
        toolItem={toolItem}
        config={config}
        onConfirm={handleConfirm}
        onReject={handleReject}
        onOpenInEditor={onOpenInEditor}
        onOpenInPanel={onOpenInPanel}
        onExpand={handleExpand}
        sessionId={sessionId}
      />
    </div>
  );
}, (prevProps, nextProps) => {
  // Compare streaming parameters and progress messages to avoid stale renders.
  const prevProgress = (prevProps.toolItem as any)._progressMessage;
  const nextProgress = (nextProps.toolItem as any)._progressMessage;
  
  return (
    prevProps.toolItem.id === nextProps.toolItem.id &&
    prevProps.toolItem.status === nextProps.toolItem.status &&
    prevProps.toolItem.userConfirmed === nextProps.toolItem.userConfirmed &&
    prevProps.toolItem.isParamsStreaming === nextProps.toolItem.isParamsStreaming &&
    prevProgress === nextProgress &&
    JSON.stringify(prevProps.toolItem.partialParams) === JSON.stringify(nextProps.toolItem.partialParams) &&
    JSON.stringify(prevProps.toolItem.toolResult) === JSON.stringify(nextProps.toolItem.toolResult)
  );
});
