import type { FlowItem } from '../../types/flow-chat';

type ProcessingPhaseLike = string | null | undefined;
type LastFlowItem = FlowItem & {
  content?: string;
  runtimeStatus?: unknown;
};

export interface ProcessingIndicatorVisibilityInput {
  isTurnProcessing: boolean;
  isSessionProcessing?: boolean;
  processingPhase?: ProcessingPhaseLike;
  lastItem?: LastFlowItem;
  isContentGrowing: boolean;
}

function hasProcessing(input: ProcessingIndicatorVisibilityInput): boolean {
  return input.isTurnProcessing || input.isSessionProcessing === true;
}

function isInlineRuntimeStatus(item: LastFlowItem | undefined): boolean {
  return item?.type === 'text' && Boolean(item.runtimeStatus);
}

export function shouldShowProcessingIndicator(input: ProcessingIndicatorVisibilityInput): boolean {
  if (!hasProcessing(input)) return false;
  if (input.processingPhase === 'tool_confirming') return false;
  if (isInlineRuntimeStatus(input.lastItem)) return false;
  if (!input.lastItem) return true;

  if (input.lastItem.type === 'text' || input.lastItem.type === 'thinking') {
    const hasContent = Boolean(input.lastItem.content);
    if (hasContent && input.isContentGrowing) return false;
  }

  if (input.lastItem.type === 'tool') {
    const toolStatus = input.lastItem.status;
    if (toolStatus === 'running' || toolStatus === 'streaming' || toolStatus === 'preparing') {
      return false;
    }
  }

  return hasProcessing(input);
}

export function shouldReserveProcessingIndicatorSpace(input: ProcessingIndicatorVisibilityInput): boolean {
  if (!hasProcessing(input)) return false;
  if (input.processingPhase === 'tool_confirming') return false;
  if (isInlineRuntimeStatus(input.lastItem)) return false;
  return true;
}
