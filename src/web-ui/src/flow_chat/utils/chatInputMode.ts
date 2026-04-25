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
