import { FlowChatStore } from '@/flow_chat/store/FlowChatStore';
import { openBtwSessionInAuxPane, openMainSession } from '@/flow_chat/services/openBtwSession';
import { resolveSessionRelationship } from '@/flow_chat/utils/sessionMetadata';

export async function openAgentCompanionSession(sessionId: string): Promise<boolean> {
  const flowChatStore = FlowChatStore.getInstance();
  const session = flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    return false;
  }

  const relationship = resolveSessionRelationship(session);
  const parentSessionId = relationship.parentSessionId;

  if (relationship.canOpenInAuxPane && parentSessionId) {
    await openMainSession(parentSessionId);
    openBtwSessionInAuxPane({
      childSessionId: sessionId,
      parentSessionId,
      workspacePath: session.workspacePath,
    });
    return true;
  }

  flowChatStore.switchSession(sessionId);
  return true;
}
