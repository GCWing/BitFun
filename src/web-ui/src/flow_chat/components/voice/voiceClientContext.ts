import { useSceneStore } from '@/app/stores/sceneStore';
import { FlowChatManager } from '@/flow_chat/services/FlowChatManager';
import { stateMachineManager } from '@/flow_chat/state-machine';
import type { Session } from '@/flow_chat/types/flow-chat';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { WorkspaceKind, isRemoteWorkspace, type WorkspaceInfo } from '@/shared/types';

const MAX_CONTEXT_WORKSPACES = 24;
const MAX_CONTEXT_SESSIONS = 12;

interface VoiceOwnedTaskBaseContext {
  sessionId: string | null;
  state: 'starting' | 'running' | 'stopping';
}

export type VoiceOwnedTaskContext = VoiceOwnedTaskBaseContext & (
  | {
      kind: 'workspace';
      workspaceId: string;
      workspaceName: string;
    }
  | {
      kind: 'miniapp';
      appId: string;
      appName: string;
    }
);

/** Immutable routing target captured when Voice starts inside a MiniApp bubble. */
export interface VoiceMiniAppCallTarget {
  kind: 'miniapp';
  appId: string;
  appName: string;
  claimToken: string;
  sessionId: string;
  workspacePath?: string;
}

function latestTurnStatus(session: Session): string {
  return session.dialogTurns[session.dialogTurns.length - 1]?.status ?? 'empty';
}

function workspaceForSession(
  session: Session,
  workspaces: WorkspaceInfo[],
): WorkspaceInfo | undefined {
  const workspacePath = session.config.projectWorkspacePath ?? session.config.workspacePath;
  if (!workspacePath) return undefined;
  return workspaces.find(workspace =>
    workspace.rootPath === workspacePath
    && (!session.config.remoteConnectionId
      || workspace.connectionId === session.config.remoteConnectionId),
  );
}

/**
 * Build a compact, public snapshot for the realtime model. This contains only
 * navigation/session facts already visible in the controller UI; it excludes
 * message contents, tool payloads, credentials, and private Agent reasoning.
 * This is context data for the Voice control plane, not a workspace Agent tool
 * registry. Do not add Agent tool schemas or execution capabilities here.
 */
export function buildVoiceClientContext(
  voiceTask: VoiceOwnedTaskContext | null = null,
  callTarget: VoiceMiniAppCallTarget | null = null,
) {
  const workspaceState = workspaceManager.getState();
  const allWorkspaces = Array.from(workspaceState.openedWorkspaces.values());
  const workspaces = allWorkspaces.slice(0, MAX_CONTEXT_WORKSPACES);
  const flowState = FlowChatManager.getInstance().getFlowChatState();
  const allSessions = Array.from(flowState.sessions.values()) as Session[];
  const sessions = allSessions
    .sort((left, right) => right.lastActiveAt - left.lastActiveAt)
    .slice(0, MAX_CONTEXT_SESSIONS)
    .map(session => {
      const workspace = workspaceForSession(session, allWorkspaces);
      return {
        id: session.sessionId,
        title: session.title || null,
        workspace_id: workspace?.id ?? null,
        workspace_name: workspace?.name ?? null,
        workspace_path: session.config.projectWorkspacePath ?? session.config.workspacePath ?? null,
        turn_status: latestTurnStatus(session),
        execution_state: stateMachineManager.getCurrentState(session.sessionId),
        active: session.sessionId === flowState.activeSessionId,
      };
    });

  const sceneState = useSceneStore.getState();
  const activeWorkspace = workspaceState.currentWorkspace;
  return {
    scope: 'openbitfun_client',
    captured_at: new Date().toISOString(),
    // Keep immutable call routing near the front of the bounded snapshot so it
    // remains prominent even when the client has many open workspaces/sessions.
    voice_call_target: callTarget ? {
      kind: 'miniapp',
      app_id: callTarget.appId,
      app_name: callTarget.appName,
      session_id: callTarget.sessionId,
      workspace_path: callTarget.workspacePath ?? null,
      task_routing: 'miniapp_conversation',
    } : null,
    active_scene: sceneState.activeTabId || null,
    open_scenes: sceneState.openTabs.map(tab => tab.id),
    active_workspace_id: activeWorkspace?.id ?? null,
    active_workspace: activeWorkspace ? {
      id: activeWorkspace.id,
      name: activeWorkspace.name,
      path: activeWorkspace.rootPath,
      kind: activeWorkspace.workspaceKind,
      remote: isRemoteWorkspace(activeWorkspace),
      connection_name: activeWorkspace.connectionName ?? null,
    } : null,
    opened_workspace_count: allWorkspaces.length,
    opened_workspaces_truncated: allWorkspaces.length > workspaces.length,
    opened_workspaces: workspaces.map(workspace => ({
      id: workspace.id,
      name: workspace.name,
      path: workspace.rootPath,
      kind: workspace.workspaceKind,
      active: workspace.id === workspaceState.activeWorkspaceId,
      remote: isRemoteWorkspace(workspace),
      connection_name: workspace.connectionName ?? null,
      supports_project_tasks: workspace.workspaceKind !== WorkspaceKind.Assistant,
    })),
    visible_session_count: allSessions.length,
    visible_sessions_truncated: allSessions.length > sessions.length,
    active_session_id: flowState.activeSessionId ?? null,
    visible_sessions: sessions,
    voice_owned_task: voiceTask ? {
      session_id: voiceTask.sessionId,
      state: voiceTask.state,
      target_kind: voiceTask.kind,
      workspace_id: voiceTask.kind === 'workspace' ? voiceTask.workspaceId : null,
      workspace_name: voiceTask.kind === 'workspace' ? voiceTask.workspaceName : null,
      miniapp_id: voiceTask.kind === 'miniapp' ? voiceTask.appId : null,
      miniapp_name: voiceTask.kind === 'miniapp' ? voiceTask.appName : null,
    } : null,
  };
}

export function serializeVoiceClientContext(
  voiceTask: VoiceOwnedTaskContext | null = null,
  callTarget: VoiceMiniAppCallTarget | null = null,
): string {
  return JSON.stringify(buildVoiceClientContext(voiceTask, callTarget));
}

/** An explicit workspace in the provider tool call always overrides MiniApp routing. */
export function shouldRouteVoiceTaskToMiniApp(
  callTarget: VoiceMiniAppCallTarget | null,
  workspaceReference?: string,
): callTarget is VoiceMiniAppCallTarget {
  return Boolean(callTarget && !workspaceReference?.trim());
}

function matchingOpenedWorkspaces(reference: string): WorkspaceInfo[] {
  const normalized = reference.trim().toLocaleLowerCase();
  const workspaces = Array.from(workspaceManager.getState().openedWorkspaces.values());
  const exactId = workspaces.find(workspace => workspace.id === reference);
  if (exactId) return [exactId];
  return workspaces.filter(workspace => {
    const rootName = workspace.rootPath.split(/[\\/]/).filter(Boolean).at(-1) ?? '';
    return workspace.name.toLocaleLowerCase() === normalized
      || workspace.rootPath.toLocaleLowerCase() === normalized
      || rootName.toLocaleLowerCase() === normalized;
  });
}

export function resolveOpenedVoiceWorkspace(
  workspaceReference?: string | null,
): WorkspaceInfo {
  if (!workspaceReference?.trim()) {
    const activeWorkspace = workspaceManager.getState().currentWorkspace;
    if (!activeWorkspace) {
      throw new Error('No OpenBitFun workspace is currently open');
    }
    if (activeWorkspace.workspaceKind === WorkspaceKind.Assistant) {
      const projectWorkspace = Array.from(
        workspaceManager.getState().openedWorkspaces.values(),
      ).find(workspace => workspace.workspaceKind !== WorkspaceKind.Assistant);
      if (!projectWorkspace) {
        throw new Error('No opened project workspace is available for this Agent task');
      }
      return projectWorkspace;
    }
    return activeWorkspace;
  }

  const matches = matchingOpenedWorkspaces(workspaceReference);
  if (matches.length === 0) {
    throw new Error(`Opened workspace not found: ${workspaceReference}`);
  }
  if (matches.length > 1) {
    throw new Error(
      `Workspace reference is ambiguous: ${matches.map(workspace => `${workspace.name} (${workspace.id})`).join(', ')}`,
    );
  }
  const workspace = matches[0];
  if (workspace.workspaceKind === WorkspaceKind.Assistant) {
    throw new Error(`Workspace ${workspace.name} is an assistant workspace, not a project workspace`);
  }
  return workspace;
}

export async function switchOpenedVoiceWorkspace(
  workspaceReference: string,
): Promise<WorkspaceInfo> {
  const matches = matchingOpenedWorkspaces(workspaceReference);
  if (matches.length === 0) {
    throw new Error(`Opened workspace not found: ${workspaceReference}`);
  }
  if (matches.length > 1) {
    throw new Error(
      `Workspace reference is ambiguous: ${matches.map(workspace => `${workspace.name} (${workspace.id})`).join(', ')}`,
    );
  }
  return workspaceManager.setActiveWorkspace(matches[0].id);
}
