import { useState, useEffect } from 'react';
import { ModelConfig } from '../shared/types';
import { modelConfigManager } from '../infrastructure/config/services/modelConfigs';
export const useModelConfigs = () => {
  const [configs, setConfigs] = useState<ModelConfig[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const initialConfigs = modelConfigManager.getAllConfigs();
    setConfigs(initialConfigs);
    setLoading(false);
    
    const unsubscribe = modelConfigManager.addListener((updatedConfigs) => {
      setConfigs(updatedConfigs);
      setLoading(false);
    });
    
    return unsubscribe;
  }, []);

  return {
    configs,
    loading,
    refresh: () => {
      const refreshedConfigs = modelConfigManager.getAllConfigs();
      setConfigs(refreshedConfigs);
    }
  };
};

export const useCurrentModelConfig = (initialConfigId?: string) => {
  const { configs } = useModelConfigs();
  const [currentConfig, setCurrentConfig] = useState<ModelConfig | null>(null);
  
  const CURRENT_CONFIG_KEY = 'wing_coder_current_model_config';

  const setCurrentConfigWithPersistence = (config: ModelConfig | null) => {
    setCurrentConfig(config);
    if (config) {
      localStorage.setItem(CURRENT_CONFIG_KEY, config.id);
    } else {
      localStorage.removeItem(CURRENT_CONFIG_KEY);
    }
    
    // Notify other components via storage event
    window.dispatchEvent(new StorageEvent('storage', {
      key: CURRENT_CONFIG_KEY,
      newValue: config?.id || null,
      storageArea: localStorage
    }));
  };

  useEffect(() => {
    const handleStorageChange = (e: StorageEvent) => {
      if (e.key === CURRENT_CONFIG_KEY && e.storageArea === localStorage) {
        if (e.newValue) {
          const targetConfig = configs.find(c => c.id === e.newValue);
          if (targetConfig && targetConfig.id !== currentConfig?.id) {
            setCurrentConfig(targetConfig);
          }
        } else if (currentConfig) {
          setCurrentConfig(null);
        }
      }
    };

    window.addEventListener('storage', handleStorageChange);
    return () => window.removeEventListener('storage', handleStorageChange);
  }, [configs, currentConfig]);

  useEffect(() => {
    if (configs.length === 0) {
      setCurrentConfig(null);
      return;
    }

    if (!currentConfig) {
      const savedConfigId = localStorage.getItem(CURRENT_CONFIG_KEY);
      const targetConfigId = initialConfigId || savedConfigId;
      
      if (targetConfigId) {
        const foundConfig = configs.find(c => c.id === targetConfigId);
        if (foundConfig) {
          setCurrentConfig(foundConfig);
          localStorage.setItem(CURRENT_CONFIG_KEY, foundConfig.id);
          return;
        }
      }
      
      // Fallback to first available config
      const firstConfig = configs[0];
      if (firstConfig) {
        setCurrentConfig(firstConfig);
        localStorage.setItem(CURRENT_CONFIG_KEY, firstConfig.id);
      }
      return;
    }

    // Handle case when current config no longer exists
    const currentConfigExists = configs.find(c => c.id === currentConfig.id);
    if (!currentConfigExists) {
      const firstConfig = configs[0];
      if (firstConfig) {
        setCurrentConfig(firstConfig);
        localStorage.setItem(CURRENT_CONFIG_KEY, firstConfig.id);
      } else {
        setCurrentConfig(null);
        localStorage.removeItem(CURRENT_CONFIG_KEY);
      }
    } else {
      // Sync with latest version (config may have been edited)
      const updatedConfig = configs.find(c => c.id === currentConfig.id);
      if (updatedConfig && JSON.stringify(updatedConfig) !== JSON.stringify(currentConfig)) {
        setCurrentConfig(updatedConfig);
      }
    }
  }, [configs, currentConfig, initialConfigId]);

  return {
    currentConfig,
    setCurrentConfig: setCurrentConfigWithPersistence,
    availableConfigs: configs
  };
};
