export const DEFAULT_CHAT_INPUT_MODE_CONFIG_PATH = 'app.flow_chat.default_mode_id';

export function normalizeUserDefaultChatInputModeId(value: unknown): string | null {
  if (typeof value !== 'string') {
    return null;
  }

  const trimmed = value.trim();
  return trimmed ? trimmed : null;
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
    return null;
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
