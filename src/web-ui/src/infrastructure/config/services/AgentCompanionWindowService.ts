import { isTauriRuntime } from '@/infrastructure/runtime';
import { createLogger } from '@/shared/utils/logger';
import type { AIExperienceSettings } from './AIExperienceConfigService';

const log = createLogger('AgentCompanionWindowService');

/**
 * Serialized `invoke`/`emit` so rapid settings toggles cannot interleave show/hide on the backend.
 */
let companionDesktopWindowSyncChain: Promise<void> = Promise.resolve();
let companionDesktopWindowSyncRequestId = 0;

export async function syncAgentCompanionDesktopWindow(
  settings: AIExperienceSettings,
): Promise<void> {
  if (!isTauriRuntime()) return;
  const requestId = companionDesktopWindowSyncRequestId += 1;

  const run = async (): Promise<void> => {
    const startedAt = performance.now();
    const command = settings.enable_agent_companion
      && settings.agent_companion_display_mode === 'desktop'
      ? 'show_agent_companion_desktop_pet'
      : 'hide_agent_companion_desktop_pet';

    if (requestId !== companionDesktopWindowSyncRequestId) {
      log.debug('Skipped stale Agent companion desktop window sync', {
        command,
        displayMode: settings.agent_companion_display_mode,
      });
      return;
    }

    try {
      log.debug('Agent companion desktop window sync started', {
        command,
        displayMode: settings.agent_companion_display_mode,
      });
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke(command);
      if (requestId !== companionDesktopWindowSyncRequestId) {
        return;
      }
      if (command === 'show_agent_companion_desktop_pet') {
        const { emit } = await import('@tauri-apps/api/event');
        await emit('agent-companion://settings-updated', settings);
      }
      log.debug('Agent companion desktop window sync completed', {
        command,
        durationMs: Number((performance.now() - startedAt).toFixed(1)),
      });
    } catch (error) {
      log.error('Failed to sync Agent companion desktop window', {
        command,
        durationMs: Number((performance.now() - startedAt).toFixed(1)),
        error,
      });
    }
  };

  companionDesktopWindowSyncChain = companionDesktopWindowSyncChain.then(run, run);
  await companionDesktopWindowSyncChain;
}
