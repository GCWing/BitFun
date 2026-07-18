/**
 * Resident listener for account cloud-sync settings application.
 *
 * The desktop backend emits `account://settings-applied` after importing a
 * newer cloud settings blob (login auto-sync and the periodic pull reconcile).
 * Registered once at app startup so the frontend config cache and
 * config-driven UI refresh even while the account dialog is closed.
 */
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SettingsAppliedListener');

let settingsAppliedUnlisten: (() => void) | null = null;

async function applyCloudSyncedSettings(): Promise<void> {
  try {
    await configAPI.reloadConfig();
    await configManager.applyExternalReload();
  } catch (error) {
    log.warn('Failed to apply cloud-synced settings', error);
  }
}

/** Register once so config refresh works while the account dialog is closed. */
export function ensureSettingsAppliedListener(): void {
  if (settingsAppliedUnlisten) {
    return;
  }
  try {
    settingsAppliedUnlisten = api.listen('account://settings-applied', () => {
      void applyCloudSyncedSettings();
    });
  } catch (error) {
    log.warn('Failed to register settings-applied listener', error);
  }
}
