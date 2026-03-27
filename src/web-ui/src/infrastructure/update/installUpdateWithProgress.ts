import { systemAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('installUpdateWithProgress');

export const UPDATE_PROGRESS_EVENT = 'bitfun-update-progress';

export interface UpdateDownloadProgressPayload {
  downloaded: number;
  total: number | null;
}

/**
 * Subscribes to Rust-emitted download progress, then runs `install_update`.
 * Unsubscribes when the install promise settles.
 */
export async function installUpdateWithProgress(
  onProgress: (p: UpdateDownloadProgressPayload) => void
): Promise<void> {
  const { listen } = await import('@tauri-apps/api/event');
  const unlisten = await listen<UpdateDownloadProgressPayload>(
    UPDATE_PROGRESS_EVENT,
    event => {
      const payload = event.payload;
      onProgress({
        downloaded: payload.downloaded,
        total: payload.total ?? null
      });
    }
  );
  try {
    await systemAPI.installUpdate();
  } catch (error) {
    log.error('install_update failed', error);
    throw error;
  } finally {
    unlisten();
  }
}
