/**
 * Peer-aware workspace directory picker.
 * - Local: native `@tauri-apps/plugin-dialog`
 * - Peer Device Mode: in-app browser listing the peer filesystem via HostInvoke
 */

import { isPeerDeviceModeActive } from './peerModeFlag';
import { usePeerDirectoryPickerStore } from './peerDirectoryPickerStore';

export interface PickWorkspaceDirectoryOptions {
  title: string;
  defaultPath?: string;
}

export async function pickWorkspaceDirectory(
  options: PickWorkspaceDirectoryOptions,
): Promise<string | null> {
  if (!isPeerDeviceModeActive()) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const selected = await open({
      directory: true,
      multiple: false,
      title: options.title,
      defaultPath: options.defaultPath,
    });
    return typeof selected === 'string' && selected.length > 0 ? selected : null;
  }

  return usePeerDirectoryPickerStore.getState().show(options);
}
