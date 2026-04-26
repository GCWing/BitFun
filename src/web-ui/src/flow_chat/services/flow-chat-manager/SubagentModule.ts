/**
 * Routes subagent events to their parent tool cards.
 */

import { FlowChatStore } from '../../store/FlowChatStore';
import { createLogger } from '@/shared/utils/logger';
import type { FlowChatContext, FlowTextItem, SubagentTextChunkData, SubagentToolEventData } from './types';
import type { FlowThinkingItem } from '../../types/flow-chat';
import { processToolEvent } from './ToolEventModule';
import type { ToolEventData } from '../EventBatcher';

const log = createLogger('SubagentModule');

function getSubagentTextItemId(parentToolId: string, sessionId: string, roundId: string): string {
  return `subagent-text-${parentToolId}-${sessionId}-${roundId}`;
}

function findParentTurnId(parentSession: { dialogTurns: Array<{ id: string; modelRounds: Array<{ items: Array<{ id: string }> }> }> }, parentToolId: string): string | null {
  for (const turn of parentSession.dialogTurns) {
    const hasParentTool = turn.modelRounds.some(round =>
      round.items.some(item => item.id === parentToolId)
    );
    if (hasParentTool) {
      return turn.id;
    }
  }

  return null;
}

/**
 * Create a placeholder text item when a subagent model round starts.
 * This gives users immediate visual feedback that the subagent is running,
 * rather than waiting for the first text chunk.
 */
export function routeModelRoundStartedToToolCard(
  _context: FlowChatContext,
  parentSessionId: string,
  parentToolId: string,
  data: {
    sessionId: string;
    turnId: string;
    roundId: string;
  }
): void {
  const store = FlowChatStore.getInstance();
  const parentSession = store.getState().sessions.get(parentSessionId);

  if (!parentSession) {
    log.debug('Parent session not found (Subagent ModelRoundStarted)', { parentSessionId });
    return;
  }

  const parentTurnId = findParentTurnId(parentSession, parentToolId);
  if (!parentTurnId) {
    log.debug('Parent tool DialogTurn not found (ModelRoundStarted)', { parentSessionId, parentToolId });
    return;
  }

  const itemId = getSubagentTextItemId(parentToolId, data.sessionId, data.roundId);

  // Check if placeholder already exists (e.g., from a previous call)
  const parentTurn = parentSession.dialogTurns.find(turn => turn.id === parentTurnId);
  if (parentTurn) {
    for (const round of parentTurn.modelRounds) {
      if (round.items.some(item => item.id === itemId)) {
        return;
      }
    }
  }

  const parentTool = store.findToolItem(parentSessionId, parentTurnId, parentToolId);
  const parentTimestamp = parentTool?.timestamp || Date.now();

  const placeholderItem: FlowTextItem = {
    id: itemId,
    type: 'text',
    content: '\u200B',
    timestamp: parentTimestamp + 1,
    isStreaming: true,
    status: 'streaming',
    isMarkdown: true,
    isSubagentItem: true,
    parentTaskToolId: parentToolId,
    subagentSessionId: data.sessionId,
  };

  store.insertModelRoundItemAfterTool(parentSessionId, parentTurnId, parentToolId, placeholderItem);
}

/**
 * Route subagent text chunks to the parent tool card.
 * Supports "text" and "thinking" content types.
 */
export function routeTextChunkToToolCard(
  _context: FlowChatContext,
  parentSessionId: string,
  parentToolId: string,
  data: SubagentTextChunkData
): void {
  const store = FlowChatStore.getInstance();
  const parentSession = store.getState().sessions.get(parentSessionId);
  
  if (!parentSession) {
    log.debug('Parent session not found (Subagent TextChunk)', { parentSessionId });
    return;
  }

  const parentTurnId = findParentTurnId(parentSession, parentToolId);
  if (!parentTurnId) {
    log.debug('Parent tool DialogTurn not found', { parentSessionId, parentToolId });
    return;
  }
  
  const isThinking = data.contentType === 'thinking';
  const itemPrefix = isThinking ? 'subagent-thinking' : 'subagent-text';
  // Format: subagent-{type}-{parentToolId}-{sessionId}-{roundId}
  const itemId = isThinking
    ? `${itemPrefix}-${parentToolId}-${data.sessionId}-${data.roundId}`
    : getSubagentTextItemId(parentToolId, data.sessionId, data.roundId);
  
  const isThinkingEnd = isThinking && !!data.isThinkingEnd;
  const textContent = data.text;
  
  const parentTurn = parentSession.dialogTurns.find(turn => turn.id === parentTurnId);
  let existingItem: FlowTextItem | FlowThinkingItem | null = null;
  
  if (parentTurn) {
    for (const round of parentTurn.modelRounds) {
      const found = round.items.find(item => item.id === itemId);
      if (found) {
        existingItem = found as FlowTextItem | FlowThinkingItem;
        break;
      }
    }
  }
  
  if (existingItem) {
    // Strip the zero-width-space placeholder when the first real text arrives.
    const baseContent = existingItem.content === '\u200B' ? '' : existingItem.content;
    const content = baseContent + textContent;

    if (isThinkingEnd) {
      store.updateModelRoundItem(parentSessionId, parentTurnId, itemId, {
        content,
        isStreaming: false,
        isCollapsed: true,
        status: 'completed',
        timestamp: Date.now(),
      } as any);
      
    } else {
      store.updateModelRoundItem(parentSessionId, parentTurnId, itemId, {
        content,
        isStreaming: true,
        isMarkdown: !isThinking,
        status: 'streaming',
        timestamp: Date.now(),
      } as any);
    }
  } else {
    // Keep subagent item timestamps right after the parent tool.
    const parentTool = store.findToolItem(parentSessionId, parentTurnId, parentToolId);
    const parentTimestamp = parentTool?.timestamp || Date.now();
    
    if (isThinking) {
      const newThinkingItem: import('../../types/flow-chat').FlowThinkingItem = {
        id: itemId,
        type: 'thinking',
        content: textContent,
        timestamp: parentTimestamp + 1,
        isStreaming: !isThinkingEnd,
        isCollapsed: isThinkingEnd,
        status: isThinkingEnd ? 'completed' : 'streaming',
        isSubagentItem: true,
        parentTaskToolId: parentToolId,
        subagentSessionId: data.sessionId
      } as any;
      
      store.insertModelRoundItemAfterTool(parentSessionId, parentTurnId, parentToolId, newThinkingItem);
    } else {
      const newTextItem: FlowTextItem = {
        id: itemId,
        type: 'text',
        content: textContent,
        timestamp: parentTimestamp + 1,
        isStreaming: true,
        status: 'streaming',
        isMarkdown: true,
        isSubagentItem: true,
        parentTaskToolId: parentToolId,
        subagentSessionId: data.sessionId
      };
      
      store.insertModelRoundItemAfterTool(parentSessionId, parentTurnId, parentToolId, newTextItem);
    }
  }
}

/**
 * Route subagent tool events to the parent tool card.
 */
export function routeToolEventToToolCard(
  context: FlowChatContext,
  parentSessionId: string,
  parentToolId: string,
  data: SubagentToolEventData,
  onTodoWriteResult?: (sessionId: string, turnId: string, result: any) => void
): void {
  const store = FlowChatStore.getInstance();
  const parentSession = store.getState().sessions.get(parentSessionId);
  
  if (!parentSession) {
    log.debug('Parent session not found (Subagent ToolEvent)', { parentSessionId });
    return;
  }

  let parentTurnId: string | null = null;
  for (const turn of parentSession.dialogTurns) {
    const hasParentTool = turn.modelRounds.some(round => 
      round.items.some(item => item.id === parentToolId)
    );
    if (hasParentTool) {
      parentTurnId = turn.id;
      break;
    }
  }
  
  if (!parentTurnId) {
    log.debug('Parent tool DialogTurn not found', { parentSessionId, parentToolId });
    return;
  }
  
  const { toolEvent } = data;
  
  // Keep subagent item timestamps right after the parent tool.
  const parentTool = store.findToolItem(parentSessionId, parentTurnId, parentToolId);
  const parentTimestamp = parentTool?.timestamp || Date.now();
  
  processToolEvent(context, parentSessionId, parentTurnId, toolEvent, {
    isSubagent: true,
    parentToolId: parentToolId,
    subagentSessionId: data.sessionId,
    parentTimestamp: parentTimestamp
  }, onTodoWriteResult);
}

/**
 * Internal TextChunk routing for batch processing.
 */
export function routeTextChunkToToolCardInternal(
  context: FlowChatContext,
  parentSessionId: string,
  parentToolId: string,
  chunkData: {
    sessionId: string;
    turnId: string;
    roundId: string;
    text: string;
    contentType: string;
    isThinkingEnd?: boolean;
  }
): void {
  routeTextChunkToToolCard(context, parentSessionId, parentToolId, chunkData);
}

/**
 * Internal ModelRoundStarted routing for batch/direct event processing.
 */
export function routeModelRoundStartedToToolCardInternal(
  context: FlowChatContext,
  parentSessionId: string,
  parentToolId: string,
  roundData: {
    sessionId: string;
    turnId: string;
    roundId: string;
  }
): void {
  routeModelRoundStartedToToolCard(context, parentSessionId, parentToolId, roundData);
}

/**
 * Internal ToolEvent routing for batch processing.
 */
export function routeToolEventToToolCardInternal(
  context: FlowChatContext,
  parentSessionId: string,
  parentToolId: string,
  eventData: ToolEventData,
  onTodoWriteResult?: (sessionId: string, turnId: string, result: any) => void
): void {
  routeToolEventToToolCard(context, parentSessionId, parentToolId, eventData, onTodoWriteResult);
}
