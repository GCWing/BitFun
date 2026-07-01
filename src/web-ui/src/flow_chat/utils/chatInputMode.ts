import { WorkspaceKind, type WorkspaceInfo } from '@/shared/types';

export const DEFAULT_CHAT_INPUT_MODE_CONFIG_PATH = 'app.flow_chat.default_mode_id';

const FIXED_CHAT_INPUT_MODE_IDS = new Set(['cowork', 'claw']);
const SUBAGENT_HIDDEN_CHAT_INPUT_ACTION_IDS = new Set(['goal', 'deepreview', 'init']);

type WorkspaceResolutionInfo = Pick<
  WorkspaceInfo,
  'id' | 'rootPath' | 'workspaceKind' | 'connectionId'
>;

export type ChatInputFixedModeReason =
  | 'assistant-workspace'
  | 'acp-session'
  | 'current-mode'
  | 'session-mode';

export interface ChatInputModePolicy {
  canSwitchModes: boolean;
  fixedModeId: string | null;
  fixedReason: ChatInputFixedModeReason | null;
}

function normalizeOptionalString(value: string | null | undefined): string | null {
  if (typeof value !== 'string') {
    return null;
  }

  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function normalizeAgentTypeString(value: string | null | undefined): string | null {
  const normalized = normalizeOptionalString(value);
  if (!normalized || normalized.toLowerCase() === 'not provided') {
    return null;
  }

  return normalized;
}

function normalizeWorkspacePath(value: string | null | undefined): string | null {
  const trimmed = normalizeOptionalString(value);
  if (!trimmed) {
    return null;
  }

  return trimmed.replace(/[\\/]+$/, '');
}

function isWorkspaceConnectionCompatible(
  workspaceConnectionId: string | null | undefined,
  sessionRemoteConnectionId: string | null | undefined,
): boolean {
  const normalizedWorkspaceConnectionId = normalizeOptionalString(workspaceConnectionId);
  const normalizedSessionRemoteConnectionId = normalizeOptionalString(sessionRemoteConnectionId);

  if (normalizedSessionRemoteConnectionId && normalizedWorkspaceConnectionId) {
    return normalizedWorkspaceConnectionId === normalizedSessionRemoteConnectionId;
  }

  if (normalizedSessionRemoteConnectionId && !normalizedWorkspaceConnectionId) {
    return false;
  }

  return true;
}

function resolveSessionWorkspaceMatch(params: {
  currentWorkspace?: WorkspaceResolutionInfo | null;
  sessionWorkspaceId?: string | null;
  sessionWorkspacePath?: string | null;
  sessionRemoteConnectionId?: string | null;
  openedWorkspaces?: Iterable<WorkspaceResolutionInfo>;
}): WorkspaceResolutionInfo | null {
  const normalizedSessionWorkspaceId = normalizeOptionalString(params.sessionWorkspaceId);
  const normalizedSessionWorkspacePath = normalizeWorkspacePath(params.sessionWorkspacePath);
  const normalizedSessionRemoteConnectionId = normalizeOptionalString(params.sessionRemoteConnectionId);
  const currentWorkspace = params.currentWorkspace ?? null;
  const openedWorkspaces = params.openedWorkspaces ?? [];

  if (normalizedSessionWorkspaceId) {
    if (currentWorkspace?.id === normalizedSessionWorkspaceId) {
      return currentWorkspace;
    }

    for (const workspace of openedWorkspaces) {
      if (workspace.id === normalizedSessionWorkspaceId) {
        return workspace;
      }
    }
  }

  if (!normalizedSessionWorkspacePath) {
    return null;
  }

  const matchingWorkspaces: WorkspaceResolutionInfo[] = [];
  const pushIfMatching = (workspace: WorkspaceResolutionInfo | null | undefined) => {
    if (!workspace) {
      return;
    }

    if (normalizeWorkspacePath(workspace.rootPath) !== normalizedSessionWorkspacePath) {
      return;
    }

    if (!isWorkspaceConnectionCompatible(workspace.connectionId, normalizedSessionRemoteConnectionId)) {
      return;
    }

    if (!matchingWorkspaces.some(candidate => candidate.id === workspace.id)) {
      matchingWorkspaces.push(workspace);
    }
  };

  pushIfMatching(currentWorkspace);
  for (const workspace of openedWorkspaces) {
    pushIfMatching(workspace);
  }

  if (normalizedSessionRemoteConnectionId) {
    const exactConnectionMatch = matchingWorkspaces.find(
      (workspace) => normalizeOptionalString(workspace.connectionId) === normalizedSessionRemoteConnectionId,
    );
    if (exactConnectionMatch) {
      return exactConnectionMatch;
    }
  }

  return matchingWorkspaces[0] ?? null;
}

export function normalizeUserDefaultChatInputModeId(value: unknown): string | null {
  if (typeof value !== 'string') {
    return null;
  }

  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

export function resolveSessionAssistantWorkspace(params: {
  currentWorkspace?: WorkspaceResolutionInfo | null;
  sessionWorkspaceId?: string | null;
  sessionWorkspacePath?: string | null;
  sessionRemoteConnectionId?: string | null;
  openedWorkspaces?: Iterable<WorkspaceResolutionInfo>;
}): boolean {
  const matchedWorkspace = resolveSessionWorkspaceMatch(params);
  if (matchedWorkspace) {
    return matchedWorkspace.workspaceKind === WorkspaceKind.Assistant;
  }

  const hasExplicitSessionWorkspace =
    normalizeOptionalString(params.sessionWorkspaceId) !== null
    || normalizeWorkspacePath(params.sessionWorkspacePath) !== null;
  if (hasExplicitSessionWorkspace) {
    return false;
  }

  return params.currentWorkspace?.workspaceKind === WorkspaceKind.Assistant;
}

function normalizeModeLookupId(value: string | null | undefined): string | null {
  return normalizeOptionalString(value)?.toLowerCase() ?? null;
}

function canonicalFixedModeId(value: string | null | undefined): string | null {
  switch (normalizeModeLookupId(value)) {
    case 'cowork':
      return 'Cowork';
    case 'claw':
      return 'Claw';
    default:
      return null;
  }
}

export function resolveChatInputModePolicy(params: {
  currentMode: string;
  isAssistantWorkspace: boolean;
  sessionMode?: string | null;
  isAcpTargetSession?: boolean;
}): ChatInputModePolicy {
  if (params.isAcpTargetSession) {
    return {
      canSwitchModes: false,
      fixedModeId: null,
      fixedReason: 'acp-session',
    };
  }

  if (params.isAssistantWorkspace) {
    return {
      canSwitchModes: false,
      fixedModeId: 'Claw',
      fixedReason: 'assistant-workspace',
    };
  }

  const fixedSessionModeId = canonicalFixedModeId(params.sessionMode);
  if (fixedSessionModeId) {
    return {
      canSwitchModes: false,
      fixedModeId: fixedSessionModeId,
      fixedReason: 'session-mode',
    };
  }

  const fixedCurrentModeId = canonicalFixedModeId(params.currentMode);
  if (fixedCurrentModeId) {
    return {
      canSwitchModes: false,
      fixedModeId: fixedCurrentModeId,
      fixedReason: 'current-mode',
    };
  }

  return {
    canSwitchModes: true,
    fixedModeId: null,
    fixedReason: null,
  };
}

export function resolveSwitchableChatInputModes<TMode extends { id: string }>(
  availableModes: Iterable<TMode>,
): TMode[] {
  return Array.from(availableModes).filter(
    mode => !FIXED_CHAT_INPUT_MODE_IDS.has(normalizeModeLookupId(mode.id) ?? ''),
  );
}

export function resolveChatInputSendAgentType(params: {
  isSubagentTarget: boolean;
  subagentType?: string | null;
  sessionMode?: string | null;
  acpTargetAgentType?: string | null;
  composerMode: string;
}): string {
  const composerMode = normalizeAgentTypeString(params.composerMode) ?? 'agentic';
  if (!params.isSubagentTarget) {
    return normalizeAgentTypeString(params.acpTargetAgentType) ?? composerMode;
  }

  return (
    normalizeAgentTypeString(params.sessionMode) ??
    normalizeAgentTypeString(params.subagentType) ??
    composerMode
  );
}

export function resolveChatInputCanUseSkills(params: {
  isSubagentTarget: boolean;
  targetAgentType: string;
  availableAgents: Iterable<{ id: string; defaultTools?: string[] }>;
}): boolean {
  const targetAgentType = normalizeModeLookupId(params.targetAgentType);
  if (!targetAgentType) {
    return !params.isSubagentTarget;
  }

  for (const agent of params.availableAgents) {
    if (normalizeModeLookupId(agent.id) !== targetAgentType) {
      continue;
    }

    if (!Array.isArray(agent.defaultTools)) {
      return !params.isSubagentTarget;
    }

    return agent.defaultTools.some(tool => normalizeModeLookupId(tool) === 'skill');
  }

  return !params.isSubagentTarget;
}

export function isChatInputActionVisibleForTarget(params: {
  actionId: string;
  isSubagentTarget: boolean;
}): boolean {
  if (!params.isSubagentTarget) {
    return true;
  }

  const actionId = normalizeModeLookupId(params.actionId);
  return !actionId || !SUBAGENT_HIDDEN_CHAT_INPUT_ACTION_IDS.has(actionId);
}

export function resolveWorkspaceChatInputMode(params: {
  currentMode: string;
  isAssistantWorkspace: boolean;
  sessionMode?: string | null;
}): string | null {
  const normalizedSessionMode = params.sessionMode?.trim();

  if (params.isAssistantWorkspace) {
    return params.currentMode === 'Claw' ? null : 'Claw';
  }

  if (normalizedSessionMode?.toLowerCase() === 'claw') {
    return params.currentMode === 'Claw' ? null : 'Claw';
  }

  if (normalizedSessionMode && normalizedSessionMode !== params.currentMode) {
    return normalizedSessionMode;
  }

  if (!normalizedSessionMode && params.currentMode === 'Claw') {
    return 'agentic';
  }

  return null;
}

export function resolveAvailableChatInputMode(params: {
  currentMode: string;
  isAssistantWorkspace: boolean;
  sessionMode?: string | null;
  userDefaultModeId?: string | null;
  availableModeIds: Iterable<string>;
}): string | null {
  const availableModeIds = new Set(
    Array.from(params.availableModeIds, (modeId) => modeId.trim()).filter(Boolean),
  );
  if (availableModeIds.size === 0) {
    return null;
  }

  const synchronizedMode = resolveWorkspaceChatInputMode(params);
  if (synchronizedMode && availableModeIds.has(synchronizedMode)) {
    return synchronizedMode;
  }

  const normalizedCurrentMode = params.currentMode.trim();
  const normalizedSessionMode = params.sessionMode?.trim();
  const normalizedUserDefaultModeId = normalizeUserDefaultChatInputModeId(params.userDefaultModeId);
  const effectiveUserDefaultModeId =
    normalizedUserDefaultModeId && availableModeIds.has(normalizedUserDefaultModeId)
      ? normalizedUserDefaultModeId
      : null;
  const canUseUserDefaultMode =
    !params.isAssistantWorkspace &&
    !normalizedSessionMode &&
    Boolean(effectiveUserDefaultModeId);

  if (canUseUserDefaultMode && effectiveUserDefaultModeId && normalizedCurrentMode === 'agentic') {
    return effectiveUserDefaultModeId;
  }

  if (normalizedCurrentMode && availableModeIds.has(normalizedCurrentMode)) {
    return null;
  }

  if (canUseUserDefaultMode && effectiveUserDefaultModeId) {
    return effectiveUserDefaultModeId;
  }

  if (params.isAssistantWorkspace && availableModeIds.has('Claw')) {
    return 'Claw';
  }

  if (availableModeIds.has('agentic')) {
    return 'agentic';
  }

  return availableModeIds.values().next().value ?? null;
}
