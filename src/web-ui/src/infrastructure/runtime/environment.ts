type TauriInternals = {
  invoke?: unknown;
  metadata?: {
    currentWindow?: {
      label?: string;
    };
  };
};

const getTauriInternals = (): TauriInternals | undefined => {
  if (typeof window === 'undefined') return undefined;
  return (window as unknown as { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__;
};

export const isTauriRuntime = (): boolean => {
  const internals = getTauriInternals();
  return typeof internals?.invoke === 'function';
};

export const supportsNativeWindowControls = (): boolean => {
  // Tauri window APIs read metadata.currentWindow; browser builds must not call them without it.
  const currentWindow = getTauriInternals()?.metadata?.currentWindow;
  return isTauriRuntime() && typeof currentWindow?.label === 'string';
};

export const supportsNativeWindowDragging = supportsNativeWindowControls;

export const isMacOSDesktopRuntime = (): boolean =>
  supportsNativeWindowControls() &&
  typeof navigator !== 'undefined' &&
  typeof navigator.platform === 'string' &&
  navigator.platform.toUpperCase().includes('MAC');
