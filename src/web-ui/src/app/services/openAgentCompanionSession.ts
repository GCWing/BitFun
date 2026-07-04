import { FlowChatStore } from '@/flow_chat/store/FlowChatStore';
import { openBtwSessionInAuxPane } from '@/flow_chat/services/btwSessionPane';
import { openMainSession } from '@/flow_chat/services/sessionActivation';
import { resolveSessionRelationship } from '@/flow_chat/utils/sessionMetadata';
import { sessionBelongsToWorkspaceNavRow } from '@/flow_chat/utils/sessionOrdering';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import type { Session } from '@/flow_chat/types/flow-chat';
import type { WorkspaceInfo } from '@/shared/types/global-state';

/**
 * Resolve the opened workspace that owns this session so the pet-bubble jump
 * can activate it — matching the sidebar's workspace-switch behaviour.
 */
function findWorkspaceForSession(session: Session): WorkspaceInfo | null {
  if (!session.workspacePath) {
    return null;
  }
  const { openedWorkspaces } = workspaceManager.getState();

  // Fast path: session carries an explicit workspaceId that is still opened.
  if (session.workspaceId && openedWorkspaces.has(session.workspaceId)) {
    return openedWorkspaces.get(session.workspaceId) ?? null;
  }

  // Fallback: match by path + remote identity (same logic the sidebar uses).
  for (const workspace of openedWorkspaces.values()) {
    if (
      sessionBelongsToWorkspaceNavRow(
        session,
        workspace.rootPath,
        workspace.connectionId ?? null,
        workspace.sshHost ?? null,
      )
    ) {
      return workspace;
    }
  }
  return null;
}

export async function openAgentCompanionSession(sessionId: string): Promise<boolean> {
  const flowChatStore = FlowChatStore.getInstance();
  const session = flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    return false;
  }

  const relationship = resolveSessionRelationship(session);
  const parentSessionId = relationship.parentSessionId;

  // Activate the session's workspace when it differs from the current one,
  // mirroring the sidebar handleSwitch path so the chat-input workspace
  // folder stays consistent after opening from the pet bubble.
  const workspace = findWorkspaceForSession(session);
  const currentWorkspaceId = workspaceManager.getState().activeWorkspaceId;
  const workspaceId = workspace?.id;
  const activateWorkspace =
    workspaceId && workspaceId !== currentWorkspaceId
      ? async (targetWorkspaceId: string) => {
          await workspaceManager.setActiveWorkspace(targetWorkspaceId);
        }
      : undefined;

  if (relationship.canOpenInAuxPane && parentSessionId) {
    await openMainSession(parentSessionId, {
      workspaceId,
      activateWorkspace,
    });
    openBtwSessionInAuxPane({
      childSessionId: sessionId,
      parentSessionId,
      workspacePath: session.workspacePath,
    });
    return true;
  }

  await openMainSession(sessionId, {
    workspaceId,
    activateWorkspace,
  });

  // When the session was already active before the pet bubble was clicked,
  // activateMainSession takes an early-return path that does not call
  // switchChatSession, so `bitfun:session-switched` is never dispatched and
  // SessionsSection's listener never clears the unread marks. Clear them
  // explicitly here so the bubble and workspace dot dismiss reliably.
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      flowChatStore.clearSessionUnreadCompletion(sessionId);
      flowChatStore.clearSessionNeedsAttention(sessionId);
    });
  });

  return true;
}
