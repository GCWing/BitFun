import { canonicalHarnessId, type HarnessId } from '@/shared/agents/identity';

export type SelectableComposerExecutionLevel = HarnessId;
/** 'other' is menu navigation state, never an Agent or Harness identity. */
export type ComposerExecutionLevel = HarnessId | 'other';
export interface ComposerExecutionLevelSelection { modeId: HarnessId }

export function resolveComposerExecutionLevelSelection(level: HarnessId): ComposerExecutionLevelSelection {
  return { modeId: level };
}

export function resolveSelectedComposerExecutionLevel(params: { currentMode: string }): ComposerExecutionLevel {
  return canonicalHarnessId(params.currentMode) ?? 'other';
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
