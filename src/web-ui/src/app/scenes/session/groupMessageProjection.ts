import type { DialogTurn } from '../../../flow_chat/types/flow-chat';
import type { DialogTurnKind } from '@/shared/types/session-history';

/**
 * Group message history -> DialogTurn (reuses the existing DialogTurn shape;
 * metadata carries the five sender fields). Mirrors the backend
 * group_room_tools.rs GroupMessage wire 1:1 (author.sessionId/role/depth/name).
 *
 * R-WF-14: shared by GroupChatView (interactive) and GroupLogView (read-only)
 * so both render the same bubble timeline through the same FlowChat pipeline
 * from a single projection (no drift between the two views).
 */
export function groupMessageToDialogTurn(
  message: {
    messageId?: string;
    content: string;
    timestamp?: number;
    author?: { sessionId?: string; role?: string | null; depth?: number | null; name?: string | null; agentType?: string | null };
    groupSessionId?: string;
    role?: string | null;
  },
  groupId: string,
): DialogTurn {
  const author = message.author ?? {};
  const now = Date.now();
  const timestamp = typeof message.timestamp === 'number' && message.timestamp > 0
    ? message.timestamp
    : now;
  const id = message.messageId || `${groupId}-msg-${timestamp}-${Math.random().toString(36).slice(2, 8)}`;
  const kind: DialogTurnKind = 'user_dialog';
  return {
    id,
    sessionId: groupId,
    kind,
    // Group sessions are agent_type="group" conversations (backend builds
    // them with default_group_agent_type, group_room_tools.rs; R-WF-02 makes
    // group a first-class agent type); keep the rendered turn agentType
    // aligned with the group session type.
    agentType: 'group',
    userMessage: {
      id,
      content: message.content,
      timestamp,
      metadata: {
        groupId,
        senderSessionId: author.sessionId || 'unknown',
        ...(author.role ? { senderRole: author.role } : {}),
        ...(typeof author.depth === 'number' ? { senderDepth: author.depth } : {}),
        ...(author.name ? { senderName: author.name } : {}),
        // R-WF-08: senderType = agent type slot of the sender identity badge,
        // passed through from backend author.agentType; the group-lead turn
        // (role=system) is marked turnRole=system so message rendering can
        // distinguish the group mode prompt (acceptance: group-lead turn=system).
        ...(author.agentType ? { senderType: author.agentType } : {}),
        ...(message.role === 'system' ? { turnRole: 'system' } : {}),
      },
    },
    modelRounds: [],
    status: 'completed',
    startTime: timestamp,
    endTime: timestamp,
    success: true,
    finishReason: 'completed',
    hasFinalResponse: false,
  };
}
