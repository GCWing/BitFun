/**
 * Bridges ACP permission requests into FlowChat tool cards.
 */

import { FlowChatStore } from '../../store/FlowChatStore';
import type { FlowToolItem } from '../../types/flow-chat';
import type { AcpPermissionRequestEvent } from '@/infrastructure/api/service-api/ACPClientAPI';

const pendingAcpPermissionRequests = new Map<string, AcpPermissionRequestEvent>();

function acpPermissionToolId(event: AcpPermissionRequestEvent): string | null {
  const toolCallId = event.toolCall?.toolCallId;
  return typeof toolCallId === 'string' && toolCallId.trim().length > 0
    ? toolCallId
    : null;
}

function findToolContextById(
  store: FlowChatStore,
  toolId: string
): { sessionId: string; turnId: string; itemId: string } | null {
  const state = store.getState();
  for (const [sessionId, session] of state.sessions) {
    for (const turn of session.dialogTurns) {
      for (const round of turn.modelRounds) {
        const item = round.items.find(candidate => (
          candidate.type === 'tool' &&
          (candidate.id === toolId || (candidate as FlowToolItem).toolCall?.id === toolId)
        )) as FlowToolItem | undefined;

        if (item) {
          return { sessionId, turnId: turn.id, itemId: item.id };
        }
      }
    }
  }
  return null;
}

function applyAcpPermissionRequest(
  store: FlowChatStore,
  toolId: string,
  event: AcpPermissionRequestEvent
): boolean {
  const toolContext = findToolContextById(store, toolId);
  if (!toolContext) {
    return false;
  }

  store.updateModelRoundItem(toolContext.sessionId, toolContext.turnId, toolContext.itemId, {
    requiresConfirmation: true,
    userConfirmed: false,
    status: 'pending_confirmation',
    acpPermission: {
      permissionId: event.permissionId,
      sessionId: event.sessionId,
      toolCallId: toolId,
      requestedAt: Date.now(),
      options: event.options,
      toolCall: event.toolCall,
    },
  } as any);

  const activeSessionId = store.getState().activeSessionId;
  if (toolContext.sessionId !== activeSessionId) {
    store.setSessionNeedsAttention(toolContext.sessionId, 'tool_confirm');
  }

  return true;
}

export function handleAcpPermissionRequestForToolCard(event: AcpPermissionRequestEvent): boolean {
  const toolId = acpPermissionToolId(event);
  if (!toolId) {
    return false;
  }

  const store = FlowChatStore.getInstance();
  if (!applyAcpPermissionRequest(store, toolId, event)) {
    pendingAcpPermissionRequests.set(toolId, event);
    return true;
  }

  pendingAcpPermissionRequests.delete(toolId);
  return true;
}

export function applyPendingAcpPermissionForTool(
  store: FlowChatStore,
  toolId: string
): void {
  const event = pendingAcpPermissionRequests.get(toolId);
  if (!event) {
    return;
  }

  if (applyAcpPermissionRequest(store, toolId, event)) {
    pendingAcpPermissionRequests.delete(toolId);
  }
}
