import { RemoteSession } from '../model/RemoteModels';

/** Stable key for a session that may exist on more than one desktop. */
export function remoteSessionIdentity(session: RemoteSession): string {
  return `${session.deviceId || ''}\u0001${session.id}`;
}

export function stampRemoteSessionDevice(session: RemoteSession, deviceId: string): RemoteSession {
  if (deviceId.length === 0 || session.deviceId === deviceId) {
    return session;
  }
  return {
    id: session.id,
    title: session.title,
    agentType: session.agentType,
    status: session.status,
    updatedAt: session.updatedAt,
    createdAt: session.createdAt,
    messageCount: session.messageCount,
    workspacePath: session.workspacePath,
    workspaceName: session.workspaceName,
    pinned: session.pinned,
    titleGenerated: session.titleGenerated,
    deviceId
  };
}

export function stampRemoteSessionsDevice(sessions: RemoteSession[], deviceId: string): RemoteSession[] {
  if (deviceId.length === 0) {
    return sessions;
  }
  return sessions.map((item: RemoteSession): RemoteSession => stampRemoteSessionDevice(item, deviceId));
}

/** Fills the scope omitted by older list_sessions payloads without mutating it. */
export function stampRemoteSessionWorkspace(session: RemoteSession, workspacePath: string): RemoteSession {
  if ((session.workspacePath || '').length > 0 || workspacePath.length === 0) {
    return session;
  }
  return {
    id: session.id,
    title: session.title,
    agentType: session.agentType,
    status: session.status,
    updatedAt: session.updatedAt,
    createdAt: session.createdAt,
    messageCount: session.messageCount,
    workspacePath,
    workspaceName: session.workspaceName,
    pinned: session.pinned,
    titleGenerated: session.titleGenerated,
    deviceId: session.deviceId
  };
}

export function stampRemoteSessionsWorkspace(
  sessions: RemoteSession[],
  workspacePath: string
): RemoteSession[] {
  return sessions.map((item: RemoteSession): RemoteSession =>
    stampRemoteSessionWorkspace(item, workspacePath));
}
