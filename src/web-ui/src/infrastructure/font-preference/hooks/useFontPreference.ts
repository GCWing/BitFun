
import { useEffect } from 'react';
import { useFontPreferenceStore } from '../store/fontPreferenceStore';
import { FontSizeLevel } from '../types';

export function useFontPreference() {
  const {
    preference,
    initialized,
    loading,
    error,
    initialize,
    setUiSize,
    setFlowChatFont,
    reset,
  } = useFontPreferenceStore();

  useEffect(() => {
    if (!initialized && !loading) {
      initialize();
    }
  }, [initialized, loading, initialize]);

  return {
    preference,
    loading,
    error,
    setUiSize,
    setFlowChatFont,
    reset,
  };
}

export function useUiFontSize() {
  return useFontPreferenceStore(s => ({
    uiSize: s.preference.uiSize,
    setUiSize: s.setUiSize,
  }));
}

/** Convenience: return the current UI base font size level label (for display). */
export function useFontSizeLevelLabel(): string {
  const level = useFontPreferenceStore(s => s.preference.uiSize.level);
  const labels: Record<FontSizeLevel, string> = {
    compact: 'Compact',
    small:   'Small',
    default: 'Default',
    medium:  'Medium',
    large:   'Large',
    custom:  'Custom',
  };
  return labels[level] ?? labels.default;
}
