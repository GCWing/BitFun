import { useEffect, useState, type Dispatch, type SetStateAction } from 'react';
import { configManager } from '../services/ConfigManager';

const COMPUTER_USE_ENABLED_CONFIG_PATH = 'ai.computer_use_enabled';

export interface UseComputerUseEnabledResult {
  computerUseEnabled: boolean;
  /**
   * Local override for optimistic UI (e.g. the settings toggle). Config
   * change events re-sync the value once the write lands or fails.
   */
  setComputerUseEnabled: Dispatch<SetStateAction<boolean>>;
}

/**
 * Tracks the `ai.computer_use_enabled` config value. Starts as `false` until
 * the config loads so ComputerUse affordances never flash as available on
 * first render.
 */
export function useComputerUseEnabled(): UseComputerUseEnabledResult {
  const [computerUseEnabled, setComputerUseEnabled] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const load = () => {
      void configManager.getConfig<boolean>(COMPUTER_USE_ENABLED_CONFIG_PATH).then((enabled) => {
        if (!cancelled) setComputerUseEnabled(enabled ?? false);
      });
    };
    load();
    const unsubscribe = configManager.onConfigChange((path) => {
      if (path === COMPUTER_USE_ENABLED_CONFIG_PATH || path === 'ai') load();
    });
    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, []);

  return { computerUseEnabled, setComputerUseEnabled };
}
