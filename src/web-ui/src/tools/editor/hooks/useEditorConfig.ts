/**
 * Editor Configuration Hook
 * 
 * Responsibilities:
 * - Load user persisted configuration
 * - Merge preset configuration
 * - Merge runtime overrides
 * - Subscribe to configuration changes
 * @module useEditorConfig
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { createLogger } from '@/shared/utils/logger';
import type { EditorConfig, EditorConfigPartial, EditorPresetName } from '../config/types';
import { DEFAULT_EDITOR_CONFIG, mergeConfig } from '../config/defaults';
import { getPreset } from '../config/presets';

const log = createLogger('useEditorConfig');

interface UseEditorConfigOptions {
  /** Preset name */
  preset?: EditorPresetName;
  /** Runtime overrides */
  overrides?: EditorConfigPartial;
  /** Whether to auto-load persisted configuration */
  loadPersisted?: boolean;
}

interface UseEditorConfigResult {
  /** Final merged configuration */
  config: EditorConfig;
  /** Whether configuration is loading */
  isLoading: boolean;
  /** Update configuration */
  updateConfig: (partial: EditorConfigPartial) => void;
  /** Reset to default configuration */
  resetConfig: () => void;
}

/** Editor Configuration Hook */
export function useEditorConfig(options: UseEditorConfigOptions = {}): UseEditorConfigResult {
  const { preset, overrides, loadPersisted = true } = options;
  
  const [persistedConfig, setPersistedConfig] = useState<EditorConfigPartial | null>(null);
  const [isLoading, setIsLoading] = useState(loadPersisted);
  const [runtimeOverrides, setRuntimeOverrides] = useState<EditorConfigPartial>({});
  
  /** Load persisted configuration */
  useEffect(() => {
    if (!loadPersisted) {
      setIsLoading(false);
      return;
    }

    let unsubscribe: (() => void) | undefined;

    const syncConfig = async (): Promise<void> => {
      const { configManager } = await import('@/infrastructure/config/services/ConfigManager');
      const config = await configManager.getConfig<Record<string, unknown> | null>('editor');

      if (config) {
        setPersistedConfig(convertSnakeToCamel(config));
      }
    };

    void (async () => {
      try {
        const { configManager } = await import('@/infrastructure/config/services/ConfigManager');
        await syncConfig();
        unsubscribe = configManager.watch('editor', () => {
          void syncConfig();
        });
      } catch (error) {
        log.error('Failed to load config', error);
      } finally {
        setIsLoading(false);
      }
    })();

    return () => {
      unsubscribe?.();
    };
  }, [loadPersisted]);
  
  /** Compute final configuration: defaults -> preset -> persisted -> overrides -> runtime */
  const config = useMemo((): EditorConfig => {
    let result = { ...DEFAULT_EDITOR_CONFIG };
    
    if (preset) {
      const presetConfig = getPreset(preset);
      result = mergeConfig(result, presetConfig as EditorConfigPartial);
    }
    
    if (persistedConfig) {
      result = mergeConfig(result, persistedConfig);
    }
    
    if (overrides) {
      result = mergeConfig(result, overrides);
    }
    
    result = mergeConfig(result, runtimeOverrides);
    
    return result;
  }, [preset, persistedConfig, overrides, runtimeOverrides]);
  
  const updateConfig = useCallback((partial: EditorConfigPartial) => {
    setRuntimeOverrides(prev => mergeConfig(prev, partial));
  }, []);
  
  const resetConfig = useCallback(() => {
    setRuntimeOverrides({});
  }, []);
  
  return {
    config,
    isLoading,
    updateConfig,
    resetConfig,
  };
}

/** Convert snake_case keys to camelCase */
function convertSnakeToCamel(obj: Record<string, any>): EditorConfigPartial {
  const result: Record<string, any> = {};
  
  for (const [key, value] of Object.entries(obj)) {
    const camelKey = key.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
    
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      result[camelKey] = convertSnakeToCamel(value);
    } else {
      result[camelKey] = value;
    }
  }
  
  return result as EditorConfigPartial;
}

export default useEditorConfig;
