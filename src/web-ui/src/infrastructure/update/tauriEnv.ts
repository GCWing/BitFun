/** True when running inside the Tauri desktop shell (not pure browser dev). */
export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}
