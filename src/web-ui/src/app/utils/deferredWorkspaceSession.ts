const DEFERRED_NEW_SESSION_WORKSPACE_KEY = 'bitfun:newSessionDialog:deferredWorkspacePath';

function normalizeWorkspacePath(path: string): string {
  return path.trim().replace(/\\/g, '/');
}

export function markDeferredNewSessionWorkspace(path: string): void {
  try {
    sessionStorage.setItem(
      DEFERRED_NEW_SESSION_WORKSPACE_KEY,
      normalizeWorkspacePath(path)
    );
  } catch {
    /* ignore */
  }
}

export function consumeDeferredNewSessionWorkspace(path: string): boolean {
  try {
    const storedPath = sessionStorage.getItem(DEFERRED_NEW_SESSION_WORKSPACE_KEY);
    if (!storedPath || storedPath !== normalizeWorkspacePath(path)) {
      return false;
    }
    sessionStorage.removeItem(DEFERRED_NEW_SESSION_WORKSPACE_KEY);
    return true;
  } catch {
    return false;
  }
}

export function clearDeferredNewSessionWorkspace(path?: string): void {
  try {
    if (!path) {
      sessionStorage.removeItem(DEFERRED_NEW_SESSION_WORKSPACE_KEY);
      return;
    }

    const storedPath = sessionStorage.getItem(DEFERRED_NEW_SESSION_WORKSPACE_KEY);
    if (storedPath === normalizeWorkspacePath(path)) {
      sessionStorage.removeItem(DEFERRED_NEW_SESSION_WORKSPACE_KEY);
    }
  } catch {
    /* ignore */
  }
}
