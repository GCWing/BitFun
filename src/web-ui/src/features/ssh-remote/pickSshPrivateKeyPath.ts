/**
 * Native file picker for SSH private keys; default folder is ~/.ssh (via Tauri homeDir + join).
 */

import { open } from '@tauri-apps/plugin-dialog';
import { homeDir, join } from '@tauri-apps/api/path';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('pickSshPrivateKeyPath');

export async function pickSshPrivateKeyPath(options: { title?: string } = {}): Promise<string | null> {
  try {
    const home = await homeDir();
    const defaultPath = await join(home, '.ssh');
    const selected = await open({
      multiple: false,
      directory: false,
      defaultPath,
      title: options.title,
    });
    return selected ?? null;
  } catch (e) {
    log.error('SSH private key file picker failed', e);
    return null;
  }
}
