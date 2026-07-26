import { useEffect, useState } from 'react';
import {
  aiExperienceConfigService,
  type AIExperienceSettings,
} from '../services/AIExperienceConfigService';

export interface UseAIExperienceSettingsResult {
  settings: AIExperienceSettings | null;
  isLoading: boolean;
  error: Error | null;
}

export function useAIExperienceSettings(): UseAIExperienceSettingsResult {
  const [settings, setSettings] = useState<AIExperienceSettings | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let active = true;
    void aiExperienceConfigService.getSettingsAsync()
      .then(next => { if (active) setSettings(next); })
      .catch(reason => {
        if (active) setError(reason instanceof Error ? reason : new Error(String(reason)));
      })
      .finally(() => { if (active) setIsLoading(false); });
    const removeListener = aiExperienceConfigService.addChangeListener(next => {
      if (active) setSettings(next);
    });
    return () => { active = false; removeListener(); };
  }, []);

  return { settings, isLoading, error };
}
