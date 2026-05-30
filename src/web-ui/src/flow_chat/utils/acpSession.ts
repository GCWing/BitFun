import type { Session } from '../types/flow-chat';

const ACP_AGENT_TYPE_PREFIX = 'acp:';

export function acpClientIdFromAgentType(agentType: string | null | undefined): string | null {
  const value = agentType?.trim();
  if (!value?.startsWith(ACP_AGENT_TYPE_PREFIX)) return null;

  const clientId = value.slice(ACP_AGENT_TYPE_PREFIX.length).trim();
  return clientId || null;
}

export function isAcpAgentType(agentType: string | null | undefined): boolean {
  return acpClientIdFromAgentType(agentType) !== null;
}

export function isAcpFlowSession(
  session: Pick<Session, 'config' | 'mode'> | null | undefined,
): boolean {
  return Boolean(
    isAcpAgentType(session?.config?.agentType) ||
    isAcpAgentType(session?.mode),
  );
}

/** The identifying fields needed to query/observe an ACP session's backend state. */
export interface AcpSessionRef {
  sessionId: string;
  clientId: string;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

/**
 * Derive an {@link AcpSessionRef} from a flow-chat session, or `null` if the
 * session is not ACP-backed. Mirrors the resolution used by ModelSelector.
 */
export function acpSessionRef(
  session: Pick<Session, 'sessionId' | 'config' | 'mode' | 'workspacePath' | 'remoteConnectionId' | 'remoteSshHost'> | null | undefined,
): AcpSessionRef | null {
  if (!session?.sessionId) return null;
  const clientId =
    acpClientIdFromAgentType(session.config?.agentType) ??
    acpClientIdFromAgentType(session.mode);
  if (!clientId) return null;
  return {
    sessionId: session.sessionId,
    clientId,
    workspacePath: session.workspacePath ?? session.config?.workspacePath,
    remoteConnectionId: session.remoteConnectionId ?? session.config?.remoteConnectionId,
    remoteSshHost: session.remoteSshHost,
  };
}

/**
 * The text that invokes an ACP slash command. ACP has no dedicated command RPC:
 * a command is sent as a normal prompt whose text begins with `/<name>`.
 */
export function acpSlashCommandText(name: string): string {
  return `/${name} `;
}
