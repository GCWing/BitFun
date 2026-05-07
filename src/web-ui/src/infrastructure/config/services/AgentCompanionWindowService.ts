import { isTauriRuntime } from '@/infrastructure/runtime';
import { createLogger } from '@/shared/utils/logger';
import type { AIExperienceSettings } from './AIExperienceConfigService';

const log = createLogger('AgentCompanionWindowService');

/**
 * Serialized `invoke`/`emit` so rapid settings toggles cannot interleave show/hide on the backend.
 */
let companionDesktopWindowSyncChain: Promise<void> = Promise.resolve();

export async function syncAgentCompanionDesktopWindow(
  settings: AIExperienceSettings,
): Promise<void> {
  if (!isTauriRuntime()) return;

  const run = async (): Promise<void> => {
    const command = settings.enable_agent_companion
      && settings.agent_companion_display_mode === 'desktop'
      ? 'show_agent_companion_desktop_pet'
      : 'hide_agent_companion_desktop_pet';

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke(command);
      if (command === 'show_agent_companion_desktop_pet') {
        const { emit } = await import('@tauri-apps/api/event');
        await emit('agent-companion://settings-updated', settings);
      }
    } catch (error) {
      log.error('Failed to sync Agent companion desktop window', { command, error });
    }
  };

  companionDesktopWindowSyncChain = companionDesktopWindowSyncChain.then(run, run);
  await companionDesktopWindowSyncChain;
}
