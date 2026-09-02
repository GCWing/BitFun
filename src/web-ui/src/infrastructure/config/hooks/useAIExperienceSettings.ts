import { useCallback, useEffect, useRef, useState } from 'react';
import {
  aiExperienceConfigService,
  type AIExperienceSettings,
} from '../services/AIExperienceConfigService';

export interface UseAIExperienceSettingsResult {
  settings: AIExperienceSettings | null;
  isLoading: boolean;
  error: Error | null;
  reload: () => Promise<void>;
}

export function useAIExperienceSettings(): UseAIExperienceSettingsResult {
  const [settings, setSettings] = useState<AIExperienceSettings | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const activeRef = useRef(true);
  const requestIdRef = useRef(0);

  const load = useCallback(async (forceRefresh: boolean) => {
    const requestId = ++requestIdRef.current;
    if (activeRef.current) {
      setIsLoading(true);
      setError(null);
    }
    try {
      const next = await aiExperienceConfigService.getSettingsAsync({ forceRefresh });
      if (activeRef.current && requestId === requestIdRef.current) {
        setSettings(next);
      }
    } catch (reason) {
      if (activeRef.current && requestId === requestIdRef.current) {
        setSettings(null);
        setError(reason instanceof Error ? reason : new Error(String(reason)));
      }
    } finally {
      if (activeRef.current && requestId === requestIdRef.current) {
        setIsLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    activeRef.current = true;
    void load(false);
    const removeListener = aiExperienceConfigService.addChangeListener(next => {
      if (!activeRef.current) return;
      requestIdRef.current += 1;
      setSettings(next);
      setError(null);
      setIsLoading(false);
    });
    return () => {
      activeRef.current = false;
      removeListener();
    };
  }, [load]);

  const reload = useCallback(() => load(true), [load]);

  return { settings, isLoading, error, reload };
}
