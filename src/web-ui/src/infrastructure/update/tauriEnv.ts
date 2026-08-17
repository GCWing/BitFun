/** True when running inside the Tauri desktop shell (not pure browser dev). */
export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}

/** App update checks are available only in packaged desktop builds. */
export function canCheckForAppUpdates(): boolean {
  return isTauriRuntime() && !import.meta.env.DEV;
}
