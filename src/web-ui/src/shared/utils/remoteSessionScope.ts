export interface RemoteSessionScope {
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

function normalizedOptionalString(value: unknown): string | undefined {
  if (typeof value !== 'string') {
    return undefined;
  }
  return value.trim() || undefined;
}

export function isNonLocalRemoteHost(value: unknown): boolean {
  const host = normalizedOptionalString(value)?.toLowerCase();
  if (!host) {
    return false;
  }
  return !(
    host === 'localhost' ||
    host.startsWith('localhost:') ||
    host === '127.0.0.1' ||
    host.startsWith('127.0.0.1:') ||
    host === '::1' ||
    host === '[::1]' ||
    host.startsWith('[::1]:')
  );
}

export function normalizeRemoteSessionScope(
  remoteConnectionId?: unknown,
  remoteSshHost?: unknown,
): RemoteSessionScope {
  const connectionId = normalizedOptionalString(remoteConnectionId);
  const sshHost = normalizedOptionalString(remoteSshHost);

  return {
    ...(connectionId ? { remoteConnectionId: connectionId } : {}),
    ...(sshHost && (connectionId || isNonLocalRemoteHost(sshHost))
      ? { remoteSshHost: sshHost }
      : {}),
  };
}

export function isRemoteSessionScope(
  remoteConnectionId?: unknown,
  remoteSshHost?: unknown,
): boolean {
  const scope = normalizeRemoteSessionScope(remoteConnectionId, remoteSshHost);
  return Boolean(scope.remoteConnectionId || scope.remoteSshHost);
}
