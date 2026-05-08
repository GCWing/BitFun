/**
 * Handles streamed text chunks and thinking content.
 */

import type { FlowChatContext, FlowTextItem } from './types';
import { clearRuntimeStatus } from './RuntimeStatusModule';
import { isAcpFlowSession } from '../../utils/acpSession';

function activeTextHasLaterRoundItem(
  context: FlowChatContext,
  sessionId: string,
  turnId: string,
  roundId: string,
  textItemId: string
): boolean {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session || !isAcpFlowSession(session)) {
    return false;
  }

  const turn = session?.dialogTurns.find(candidate => candidate.id === turnId);
  const round = turn?.modelRounds.find(candidate => candidate.id === roundId);
  if (!round) return false;

  const textItemIndex = round.items.findIndex(item => item.id === textItemId);
  if (textItemIndex === -1) return false;

  return round.items.slice(textItemIndex + 1).some(item => item.type !== 'text');
}

function closeActiveTextSegment(
  context: FlowChatContext,
  sessionId: string,
  turnId: string,
  roundId: string,
  textItemId: string
): void {
  context.flowChatStore.updateModelRoundItemSilent(sessionId, turnId, textItemId, {
    isStreaming: false,
    status: 'completed',
  } as any);

  context.contentBuffers.get(sessionId)?.delete(roundId);
  context.activeTextItems.get(sessionId)?.delete(roundId);
}

/**
 * Process a normal text chunk without notifying the store.
 */
export function processNormalTextChunkInternal(
  context: FlowChatContext,
  sessionId: string,
  turnId: string,
  roundId: string,
  text: string
): void {
  clearRuntimeStatus(context, sessionId, turnId, { roundId });

  if (!context.contentBuffers.has(sessionId)) {
    context.contentBuffers.set(sessionId, new Map());
  }
  if (!context.activeTextItems.has(sessionId)) {
    context.activeTextItems.set(sessionId, new Map());
  }
  
  const sessionContentBuffer = context.contentBuffers.get(sessionId)!;
  const sessionActiveTextItems = context.activeTextItems.get(sessionId)!;

  const activeTextItemId = sessionActiveTextItems.get(roundId);
  if (
    activeTextItemId &&
    activeTextHasLaterRoundItem(context, sessionId, turnId, roundId, activeTextItemId)
  ) {
    closeActiveTextSegment(context, sessionId, turnId, roundId, activeTextItemId);
  }

  // Coalesce excessive newlines while appending.
  const currentContent = sessionContentBuffer.get(roundId) || '';
  const cleanedContent = (currentContent + text).replace(/\n{3,}/g, '\n\n');
  sessionContentBuffer.set(roundId, cleanedContent);

  let textItemId = sessionActiveTextItems.get(roundId);
  
  if (!textItemId) {
    textItemId = `text_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    
    const textItem: FlowTextItem = {
      id: textItemId,
      type: 'text',
      content: cleanedContent,
      isStreaming: true,
      isMarkdown: true,
      timestamp: Date.now(),
      status: 'streaming'
    };
    
    context.flowChatStore.addModelRoundItemSilent(sessionId, turnId, textItem, roundId);
    sessionActiveTextItems.set(roundId, textItemId);
  } else {
    context.flowChatStore.updateModelRoundItemSilent(sessionId, turnId, textItemId, {
      content: cleanedContent,
      runtimeStatus: undefined,
      isMarkdown: true,
      timestamp: Date.now()
    } as any);
  }
}

/**
 * Process thinking chunks without notifying the store.
 */
export function processThinkingChunkInternal(
  context: FlowChatContext,
  sessionId: string,
  turnId: string,
  roundId: string,
  text: string,
  isThinkingEnd = false
): void {
  clearRuntimeStatus(context, sessionId, turnId, { roundId });

  if (!context.contentBuffers.has(sessionId)) {
    context.contentBuffers.set(sessionId, new Map());
  }
  if (!context.activeTextItems.has(sessionId)) {
    context.activeTextItems.set(sessionId, new Map());
  }
  
  const sessionContentBuffer = context.contentBuffers.get(sessionId)!;
  const sessionActiveTextItems = context.activeTextItems.get(sessionId)!;

  // Store thinking content under a separate key.
  const thinkingKey = `thinking_${roundId}`;

  const currentContent = sessionContentBuffer.get(thinkingKey) || '';
  const cleanedContent = (currentContent + text).replace(/\n{3,}/g, '\n\n');
  sessionContentBuffer.set(thinkingKey, cleanedContent);

  let thinkingItemId = sessionActiveTextItems.get(thinkingKey);
  
  if (!thinkingItemId) {
    thinkingItemId = `thinking_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    
    const thinkingItem: import('../../types/flow-chat').FlowThinkingItem = {
      id: thinkingItemId,
      type: 'thinking',
      content: cleanedContent,
      isStreaming: !isThinkingEnd,
      isCollapsed: isThinkingEnd,
      timestamp: Date.now(),
      status: isThinkingEnd ? 'completed' : 'streaming'
    };
    
    context.flowChatStore.addModelRoundItemSilent(sessionId, turnId, thinkingItem, roundId);
    sessionActiveTextItems.set(thinkingKey, thinkingItemId);
    
    if (isThinkingEnd) {
      sessionContentBuffer.delete(thinkingKey);
      sessionActiveTextItems.delete(thinkingKey);
    }
  } else {
    if (isThinkingEnd) {
      context.flowChatStore.updateModelRoundItemSilent(sessionId, turnId, thinkingItemId, {
        content: cleanedContent,
        isStreaming: false,
        isCollapsed: true,
        status: 'completed',
        timestamp: Date.now()
      } as any);
      
      sessionContentBuffer.delete(thinkingKey);
      sessionActiveTextItems.delete(thinkingKey);
    } else {
      context.flowChatStore.updateModelRoundItemSilent(sessionId, turnId, thinkingItemId, {
        content: cleanedContent,
        timestamp: Date.now()
      } as any);
    }
  }
}

/**
 * Finalize streaming state for active text items.
 */
export function completeActiveTextItems(
  context: FlowChatContext,
  sessionId: string,
  turnId: string
): void {
  const sessionActiveTextItems = context.activeTextItems.get(sessionId);
  if (sessionActiveTextItems && sessionActiveTextItems.size > 0) {
    const itemsToComplete = Array.from(sessionActiveTextItems.entries());
    const batchUpdates = itemsToComplete
      .map(([_roundId, itemId]) => ({
        itemId,
        changes: {
          isStreaming: false,
          status: 'completed' as const
        }
      }));
    
    if (batchUpdates.length > 0) {
      context.flowChatStore.batchUpdateModelRoundItems(sessionId, turnId, batchUpdates);
    }
    
    sessionActiveTextItems.clear();
  }
}

/**
 * Clean up session buffers.
 */
export function cleanupSessionBuffers(context: FlowChatContext, sessionId: string): void {
  const batcherSize = context.eventBatcher.getBufferSize();
  if (batcherSize > 0) {
    context.eventBatcher.clear();
  }

  const pendingCompletion = context.pendingTurnCompletions.get(sessionId);
  if (pendingCompletion) {
    if (pendingCompletion.timer) {
      clearTimeout(pendingCompletion.timer);
    }
    context.pendingTurnCompletions.delete(sessionId);
  }
  
  const contentBuffer = context.contentBuffers.get(sessionId);
  if (contentBuffer) {
    context.contentBuffers.delete(sessionId);
  }
  
  const activeItems = context.activeTextItems.get(sessionId);
  if (activeItems) {
    context.activeTextItems.delete(sessionId);
  }

  for (const [key, timer] of context.runtimeStatusTimers.entries()) {
    if (key.startsWith(`${sessionId}:`)) {
      clearTimeout(timer);
      context.runtimeStatusTimers.delete(key);
    }
  }
}

/**
 * Clear all buffers and transient state.
 */
export function clearAllBuffers(context: FlowChatContext): void {
  for (const pendingCompletion of context.pendingTurnCompletions.values()) {
    if (pendingCompletion.timer) {
      clearTimeout(pendingCompletion.timer);
    }
  }
  context.pendingTurnCompletions.clear();

  context.contentBuffers.clear();
  context.activeTextItems.clear();

  for (const timer of context.runtimeStatusTimers.values()) {
    clearTimeout(timer);
  }
  context.runtimeStatusTimers.clear();
  
  for (const timer of context.saveDebouncers.values()) {
    clearTimeout(timer);
  }
  context.saveDebouncers.clear();
  context.lastSaveTimestamps.clear();
  context.lastSaveHashes.clear();
  context.turnSavePending.clear();
  context.turnSaveInFlight.clear();
}
