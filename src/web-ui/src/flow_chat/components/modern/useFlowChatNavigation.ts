/**
 * FlowChat navigation side effects.
 *
 * Handles cross-session focus requests for the modern virtualized list.
 */

import { useEffect, useLayoutEffect, useRef, type RefObject } from 'react';
import { globalEventBus } from '@/infrastructure/event-bus';
import { createLogger } from '@/shared/utils/logger';
import { flowChatStore } from '../../store/FlowChatStore';
import { useModernFlowChatStore, type VirtualItem } from '../../store/modernFlowChatStore';
import { flowChatManager } from '../../services/FlowChatManager';
import {
  FLOWCHAT_FOCUS_ITEM_EVENT,
  type FlowChatFocusItemRequest,
} from '../../events/flowchatNavigation';
import type { VirtualMessageListRef } from './VirtualMessageList';
import { resolveFlowChatFocusTarget, type ResolvedFocusTarget } from './flowChatFocusTarget';

const log = createLogger('useFlowChatNavigation');

interface UseFlowChatNavigationOptions {
  activeSessionId?: string;
  virtualItems: VirtualItem[];
  virtualListRef: RefObject<VirtualMessageListRef | null>;
  onExpandExploreGroup?: (groupId: string) => void;
  onNavigateToFocusTurn?: (request: FlowChatFocusItemRequest) => Promise<boolean> | boolean;
}

async function waitForCondition(predicate: () => boolean, timeoutMs: number): Promise<boolean> {
  const start = performance.now();
  while (performance.now() - start < timeoutMs) {
    if (predicate()) return true;
    await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
  }
  return predicate();
}

async function waitForAnimationFrames(frameCount: number): Promise<void> {
  let remaining = Math.max(0, frameCount);
  while (remaining > 0) {
    await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
    remaining -= 1;
  }
}

function navigateToResolvedTarget(
  virtualListRef: RefObject<VirtualMessageListRef | null>,
  target: ResolvedFocusTarget,
  options?: { allowLocalTurnIndex?: boolean },
): void {
  const list = virtualListRef.current;
  if (!list) return;

  if (target.preferTurnNavigation && target.resolvedTurnId) {
    list.navigateToTurn(target.resolvedTurnId, { behavior: 'auto' });
    return;
  }

  if (target.resolvedVirtualIndex != null) {
    list.scrollToIndex(target.resolvedVirtualIndex);
    return;
  }

  if (options?.allowLocalTurnIndex !== false && target.resolvedTurnIndex) {
    list.scrollToTurn(target.resolvedTurnIndex);
  }
}

export function useFlowChatNavigation({
  activeSessionId,
  virtualItems,
  virtualListRef,
  onExpandExploreGroup,
  onNavigateToFocusTurn,
}: UseFlowChatNavigationOptions): void {
  const virtualItemsRef = useRef(virtualItems);
  const onExpandExploreGroupRef = useRef(onExpandExploreGroup);
  const onNavigateToFocusTurnRef = useRef(onNavigateToFocusTurn);

  useLayoutEffect(() => {
    virtualItemsRef.current = virtualItems;
    onExpandExploreGroupRef.current = onExpandExploreGroup;
    onNavigateToFocusTurnRef.current = onNavigateToFocusTurn;
  }, [onExpandExploreGroup, onNavigateToFocusTurn, virtualItems]);

  useEffect(() => {
    const unsubscribe = globalEventBus.on<FlowChatFocusItemRequest>(FLOWCHAT_FOCUS_ITEM_EVENT, async (request) => {
      const { sessionId, itemId } = request;
      if (!sessionId) return;

      if (activeSessionId !== sessionId) {
        try {
          await flowChatManager.switchChatSession(sessionId);
        } catch (error) {
          log.warn('Failed to switch session for focus request', { sessionId, error });
          return;
        }
      }

      const ready = await waitForCondition(() => {
        const modernActiveSessionId = useModernFlowChatStore.getState().activeSession?.sessionId;
        return modernActiveSessionId === sessionId && !!virtualListRef.current;
      }, 1500);
      if (!ready) {
        log.warn('FlowChat focus target did not become active before timeout', { sessionId });
        return;
      }

      const delegatedTurnNavigationAttempted = Boolean(
        (request.turnId || request.turnIndex)
        && onNavigateToFocusTurnRef.current,
      );
      let delegatedTurnNavigation = false;
      if (delegatedTurnNavigationAttempted && onNavigateToFocusTurnRef.current) {
        try {
          delegatedTurnNavigation = await onNavigateToFocusTurnRef.current(request);
        } catch (error) {
          log.warn('Failed to navigate to the requested FlowChat Turn window', {
            sessionId,
            turnId: request.turnId,
            turnIndex: request.turnIndex,
            error,
          });
        }
      }

      const targetSession = flowChatStore.getState().sessions.get(sessionId);
      const resolvedTarget = resolveFlowChatFocusTarget(
        request,
        virtualItemsRef.current,
        targetSession,
      );

      if (!delegatedTurnNavigation) {
        if (resolvedTarget.expandExploreGroupId) {
          onExpandExploreGroupRef.current?.(resolvedTarget.expandExploreGroupId);
        }
        navigateToResolvedTarget(virtualListRef, resolvedTarget, {
          // A focus request carries an absolute Session Turn index. Once the
          // container-owned catalog/window transaction has handled that
          // coordinate, it must never be reused as an index into the current
          // partial presentation. Stable ids and already-rendered item indexes
          // remain safe fallbacks.
          allowLocalTurnIndex: !delegatedTurnNavigationAttempted,
        });
      }

      if (!itemId) return;

      await waitForAnimationFrames(2);

      const maxAttempts = 120;
      let attempts = 0;
      let expandedExploreGroupId: string | null = null;
      const tryFocus = () => {
        attempts += 1;
        const currentTarget = resolveFlowChatFocusTarget(
          request,
          virtualItemsRef.current,
          flowChatStore.getState().sessions.get(sessionId),
        );
        if (
          currentTarget.expandExploreGroupId
          && currentTarget.expandExploreGroupId !== expandedExploreGroupId
        ) {
          expandedExploreGroupId = currentTarget.expandExploreGroupId;
          onExpandExploreGroupRef.current?.(currentTarget.expandExploreGroupId);
        }
        const focusItemId = currentTarget.focusItemId ?? itemId;
        const element = document.querySelector(`[data-flow-item-id="${CSS.escape(focusItemId)}"]`) as HTMLElement | null;
        if (!element) {
          if (
            attempts % 12 === 0
            && !delegatedTurnNavigationAttempted
            && !currentTarget.preferTurnNavigation
          ) {
            navigateToResolvedTarget(virtualListRef, currentTarget);
          }
          if (attempts < maxAttempts) {
            requestAnimationFrame(tryFocus);
          }
          return;
        }

        element.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'auto' });
        element.classList.add('flowchat-flow-item--focused');
        window.setTimeout(() => element.classList.remove('flowchat-flow-item--focused'), 1600);
      };

      requestAnimationFrame(tryFocus);
    });

    return unsubscribe;
  }, [activeSessionId, virtualListRef]);
}
