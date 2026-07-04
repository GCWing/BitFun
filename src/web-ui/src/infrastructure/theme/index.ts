/**
 * Theme system exports.
 */

// Types
export * from './types';

// Presets
export * from './presets';

// Core service
export { ThemeService, themeService } from './core/ThemeService';

// Extension boundary
export {
  PLUGIN_THEME_COLOR_KEYS,
  createPluginThemeColorProjection,
} from './pluginThemeProjection';
export type {
  PluginThemeColorKey,
  PluginThemeColorProjection,
} from './pluginThemeProjection';

// Integrations
export { monacoThemeSync } from './integrations/MonacoThemeSync';

// State
export { useThemeStore } from './store/themeStore';

// React hooks
export {
  useTheme,
  useThemeConfig,
  useThemeColors,
  useThemeEffects,
  useThemeManagement,
  useThemeToggle,
} from './hooks/useTheme';


