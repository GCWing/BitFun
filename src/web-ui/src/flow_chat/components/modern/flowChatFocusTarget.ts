import type { FlowChatFocusItemRequest } from '../../events/flowchatNavigation';
import type { VirtualItem } from '../../store/modernFlowChatStore';
import type { Session } from '../../types/flow-chat';
import {
  absoluteSessionTurnIndexForId,
  absoluteSessionTurnIndexForLocalIndex,
  loadedSessionTurnIdForAbsoluteIndex,
} from '../../utils/flowChatTurnOrdinal';

export interface ResolvedFocusTarget {
  resolvedVirtualIndex?: number;
  resolvedTurnId?: string;
  resolvedTurnIndex?: number;
  focusItemId?: string;
  expandExploreGroupId?: string;
  preferTurnNavigation: boolean;
}

export function resolveFlowChatFocusTarget(
  request: FlowChatFocusItemRequest,
  currentVirtualItems: VirtualItem[],
  targetSession?: Session,
): ResolvedFocusTarget {
  const { turnIndex, itemId, source } = request;
  let resolvedVirtualIndex: number | undefined = undefined;
  let resolvedTurnIndex = turnIndex;
  let resolvedTurnId = request.turnId?.trim() || undefined;
  let expandExploreGroupId: string | undefined = undefined;

  if (targetSession && turnIndex && turnIndex >= 1 && !resolvedTurnId) {
    resolvedTurnId = loadedSessionTurnIdForAbsoluteIndex(targetSession, turnIndex);
  }
  if (targetSession && resolvedTurnId && resolvedTurnIndex === undefined) {
    resolvedTurnIndex = absoluteSessionTurnIndexForId(targetSession, resolvedTurnId);
  }

  if (itemId) {
    if (targetSession) {
      for (let i = 0; i < targetSession.dialogTurns.length; i += 1) {
        const turn = targetSession.dialogTurns[i];
        const found = turn.modelRounds?.some(round => round.items?.some(item => item.id === itemId));
        if (found) {
          resolvedTurnIndex = absoluteSessionTurnIndexForLocalIndex(targetSession, i);
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
          resolvedTurnId = resolvedTurnId ?? item.turnId;
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
    preferTurnNavigation: source === 'btw-back',
  };
}
