import { supportsNativeWindowDragging } from './environment';

/** Window chrome always belongs to the controller, including Peer Device Mode. */
export async function startNativeWindowDragging(): Promise<void> {
  if (!supportsNativeWindowDragging()) return;

  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().startDragging();
}
