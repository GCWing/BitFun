const STARTUP_MODULE_RELOAD_KEY = 'bitfun:startup-module-reload-attempted';

const DYNAMIC_MODULE_LOAD_ERROR =
  /(?:importing a module script failed|failed to fetch dynamically imported module|error loading dynamically imported module|load failed)/i;

type StartupModuleRecoveryStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

interface StartupModuleRecoveryRuntime {
  isDevelopment: boolean;
  storage: StartupModuleRecoveryStorage;
  reload: () => void;
}

function browserRuntime(): StartupModuleRecoveryRuntime | null {
  try {
    return {
      isDevelopment: import.meta.env.DEV,
      storage: window.sessionStorage,
      reload: () => window.location.reload(),
    };
  } catch {
    return null;
  }
}

export function isRecoverableStartupModuleLoadError(error: unknown): boolean {
  if (!(error instanceof Error) || error.name !== 'TypeError') {
    return false;
  }
  return DYNAMIC_MODULE_LOAD_ERROR.test(error.message);
}

/**
 * Reload once when a development WebView rejects a lazy module request.
 * A single retry recovers stale/transient Vite module responses without
 * turning a persistent code or server error into a reload loop.
 */
export function retryStartupAfterModuleLoadFailure(
  error: unknown,
  runtime: StartupModuleRecoveryRuntime | null = browserRuntime(),
): boolean {
  if (!runtime?.isDevelopment || !isRecoverableStartupModuleLoadError(error)) {
    return false;
  }

  try {
    if (runtime.storage.getItem(STARTUP_MODULE_RELOAD_KEY) === '1') {
      return false;
    }
    runtime.storage.setItem(STARTUP_MODULE_RELOAD_KEY, '1');
    runtime.reload();
    return true;
  } catch {
    try {
      runtime.storage.removeItem(STARTUP_MODULE_RELOAD_KEY);
    } catch {
      // Storage may be unavailable in a restricted WebView. The caller will
      // reveal the normal application error boundary instead.
    }
    return false;
  }
}

export function clearStartupModuleReloadAttempt(
  storage?: StartupModuleRecoveryStorage,
): void {
  try {
    (storage ?? window.sessionStorage).removeItem(STARTUP_MODULE_RELOAD_KEY);
  } catch {
    // A successful module import must not fail because storage is unavailable.
  }
}
