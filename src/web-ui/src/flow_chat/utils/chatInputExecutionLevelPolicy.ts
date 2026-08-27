export type SelectableComposerExecutionLevel = 'minimal' | 'balanced' | 'ultimate' | 'creative';
export type ComposerExecutionLevel = SelectableComposerExecutionLevel | 'other';

export interface ComposerExecutionLevelSelection {
  modeId: string;
}

export function isUltraAgentType(agentType: string | null | undefined): boolean {
  return agentType?.trim().toLowerCase() === 'ultra';
}

/**
 * Composer execution levels are a presentation projection over real Agents.
 */
export function resolveComposerExecutionLevelSelection(
  level: SelectableComposerExecutionLevel,
): ComposerExecutionLevelSelection {
  if (level === 'minimal') return { modeId: 'minimal' };
  if (level === 'ultimate') return { modeId: 'Ultra' };
  if (level === 'creative') return { modeId: 'Creative' };
  return { modeId: 'agentic' };
}

export function resolveSelectedComposerExecutionLevel(params: {
  currentMode: string;
}): ComposerExecutionLevel {
  if (params.currentMode.trim().toLowerCase() === 'minimal') return 'minimal';
  if (isUltraAgentType(params.currentMode)) return 'ultimate';
  if (params.currentMode.trim().toLowerCase() === 'creative') return 'creative';
  return params.currentMode.trim().toLowerCase() === 'agentic' ? 'balanced' : 'other';
}

export type ChatInputExecutionLevelOwner =
  | 'composer'
  | 'assistant-runtime-default'
  | 'acp-host'
  | 'parent-session';

export type ChatInputExecutionLevelPolicy =
  | { owner: 'composer'; userConfigurable: true }
  | {
      owner: Exclude<ChatInputExecutionLevelOwner, 'composer'>;
      userConfigurable: false;
    };

/**
 * Resolves whether the active composer target can choose its execution level.
 *
 * Root project Sessions may expose the choice in the composer; fixed Assistant,
 * ACP, and subagent targets keep that decision with their runtime owner.
 */
export function resolveChatInputExecutionLevelPolicy(params: {
  isAssistantWorkspace: boolean;
  sessionMode?: string | null;
  isAcpTargetSession: boolean;
  isSubagentInputTarget: boolean;
}): ChatInputExecutionLevelPolicy {
  if (params.isAcpTargetSession) {
    return { owner: 'acp-host', userConfigurable: false };
  }

  if (params.isSubagentInputTarget) {
    return { owner: 'parent-session', userConfigurable: false };
  }

  const isAssistantSession = params.sessionMode?.trim().toLowerCase() === 'claw';
  if (params.isAssistantWorkspace || isAssistantSession) {
    return { owner: 'assistant-runtime-default', userConfigurable: false };
  }

  return { owner: 'composer', userConfigurable: true };
}

/**
 * Prevents a selection drafted for a project Session from leaking into a
 * target whose execution level is not controlled by this composer.
 */
