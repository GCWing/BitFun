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
