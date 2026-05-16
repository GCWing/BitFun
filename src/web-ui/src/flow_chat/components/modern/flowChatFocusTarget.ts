import type { FlowChatFocusItemRequest } from '../../events/flowchatNavigation';
import type { VirtualItem } from '../../store/modernFlowChatStore';
import type { Session } from '../../types/flow-chat';

export interface ResolvedFocusTarget {
  resolvedVirtualIndex?: number;
  resolvedTurnId?: string;
  resolvedTurnIndex?: number;
  focusItemId?: string;
  expandExploreGroupId?: string;
  preferPinnedTurnNavigation: boolean;
}

export function resolveFlowChatFocusTarget(
  request: FlowChatFocusItemRequest,
  currentVirtualItems: VirtualItem[],
  targetSession?: Session,
): ResolvedFocusTarget {
  const { turnIndex, itemId, source } = request;
  let resolvedVirtualIndex: number | undefined = undefined;
  let resolvedTurnIndex = turnIndex;
  let resolvedTurnId: string | undefined = undefined;
  let expandExploreGroupId: string | undefined = undefined;

  if (targetSession && turnIndex && turnIndex >= 1 && turnIndex <= targetSession.dialogTurns.length) {
    resolvedTurnId = targetSession.dialogTurns[turnIndex - 1]?.id;
  }

  if (itemId) {
    if (targetSession) {
      for (let i = 0; i < targetSession.dialogTurns.length; i += 1) {
        const turn = targetSession.dialogTurns[i];
        const found = turn.modelRounds?.some(round => round.items?.some(item => item.id === itemId));
        if (found) {
          resolvedTurnIndex = i + 1;
          resolvedTurnId = turn.id;
          break;
        }
      }
    }

    for (let i = 0; i < currentVirtualItems.length; i += 1) {
      const item = currentVirtualItems[i];
      if (item.type === 'model-round') {
        const hit = item.data?.items?.some(flowItem => flowItem?.id === itemId);
        if (hit) {
          resolvedVirtualIndex = i;
          break;
        }
      } else if (item.type === 'explore-group') {
        const hit = item.data?.allItems?.some(flowItem => flowItem?.id === itemId);
        if (hit) {
          resolvedVirtualIndex = i;
          resolvedTurnId = resolvedTurnId ?? item.turnId;
          expandExploreGroupId = item.data.groupId;
          break;
        }
      }
    }
  }

  return {
    resolvedVirtualIndex,
    resolvedTurnId,
    resolvedTurnIndex,
    focusItemId: itemId,
    expandExploreGroupId,
    preferPinnedTurnNavigation: source === 'btw-back',
  };
}
