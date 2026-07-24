/**
 * Native file pickers for SSH authentication files; default folder is ~/.ssh
 * (via Tauri homeDir + join).
 */

import { open } from '@tauri-apps/plugin-dialog';
import { homeDir, join } from '@tauri-apps/api/path';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('pickSshAuthFilePath');

async function pickSshAuthFilePath(
  kind: 'private key' | 'certificate',
  options: { title?: string } = {},
): Promise<string | null> {
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
    log.error(`SSH ${kind} file picker failed`, e);
    return null;
  }
}

export function pickSshPrivateKeyPath(options: { title?: string } = {}): Promise<string | null> {
  return pickSshAuthFilePath('private key', options);
}

export function pickSshCertificatePath(options: { title?: string } = {}): Promise<string | null> {
  return pickSshAuthFilePath('certificate', options);
}
