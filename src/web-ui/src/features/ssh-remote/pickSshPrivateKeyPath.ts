/**
 * Native file picker for SSH private keys; default folder is ~/.ssh (via Tauri homeDir + join).
 */

import {workspaceAPI} from "@/infrastructure";
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('pickSshPrivateKeyPath');

export async function pickSshPrivateKeyPath(_options: { title?: string } = {}): Promise<string | null> {
  try {
    const selected = await workspaceAPI.open_oh_file_dialog();
    return selected ?? null;
  } catch (e) {
    log.error('SSH private key file picker failed', e);
    return null;
  }
}
