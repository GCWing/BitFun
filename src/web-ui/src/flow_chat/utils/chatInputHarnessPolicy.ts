import type { HarnessProfileId } from '@/infrastructure/api/service-api/AgentAPI';

export type ChatInputHarnessProfileOwner =
  | 'composer'
  | 'assistant-runtime-default'
  | 'acp-host'
  | 'parent-session';

export type ChatInputHarnessProfilePolicy =
  | { owner: 'composer'; userConfigurable: true }
  | {
      owner: Exclude<ChatInputHarnessProfileOwner, 'composer'>;
      userConfigurable: false;
    };

/**
 * Resolves who owns the Harness Profile decision for the active composer target.
 *
 * Harness is a Session execution fact, not an Agent mode. Root project Sessions
 * may expose the choice in the composer; fixed Assistant, ACP, and subagent
 * targets keep that decision with their runtime owner.
 */
export function resolveChatInputHarnessProfilePolicy(params: {
  isAssistantWorkspace: boolean;
  sessionMode?: string | null;
  isAcpTargetSession: boolean;
  isSubagentInputTarget: boolean;
}): ChatInputHarnessProfilePolicy {
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
 * target whose Harness is not controlled by this composer.
 */
export function resolvePendingHarnessProfileForCreation(
  policy: ChatInputHarnessProfilePolicy,
  pendingProfile: HarnessProfileId | null,
): HarnessProfileId | null {
  return policy.userConfigurable ? pendingProfile : null;
}
