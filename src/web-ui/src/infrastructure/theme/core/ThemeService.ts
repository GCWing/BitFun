

import {
  ThemeConfig,
  ThemeId,
  ThemeMetadata,
  ThemeExport,
  ThemeValidationResult,
  ThemeEventType,
  ThemeEvent,
  ThemeEventListener,
  ThemeHooks,
  SYSTEM_THEME_ID,
  ThemeSelectionId,
} from '../types';
import { builtinThemes, getSystemPreferredDefaultThemeId } from '../presets';
import { themeValidator } from '../utils/ThemeValidator';
import { configAPI } from '@/infrastructure/api';
import { monacoThemeSync } from '../integrations/MonacoThemeSync';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ThemeService');

const FLOW_CHAT_LINK_COLORS = {
  dark: {
    default: '#60a5fa',
    hover: '#93c5fd',
  },
  light: {
    default: '#0969da',
    hover: '#0550ae',
  },
} as const;

const THEME_STATIC_COLORS = {
  white: '#ffffff',
  black: '#000000',
} as const;

const ACCENT_STOPS = [50, 100, 200, 300, 400, 500, 600, 700, 800] as const;
const SECONDARY_ACCENT_STOPS = [50, 100, 200, 400, 500, 600, 800] as const;
const SHADOW_TOKENS = ['xs', 'sm', 'base', 'lg', 'xl'] as const;
const BLUR_TOKENS = ['subtle', 'base'] as const;
const RADIUS_TOKENS = ['sm', 'base', 'lg', 'xl', '2xl', 'full'] as const;
const SPACING_TOKENS = [1, 2, 3, 4, 5, 6, 8, 10, 12, 16] as const;
const MOTION_DURATION_TOKENS = ['instant', 'fast', 'base', 'slow'] as const;
const EASING_TOKENS = ['standard', 'decelerate', 'smooth'] as const;
const FONT_WEIGHT_TOKENS = ['normal', 'medium', 'semibold', 'bold'] as const;
const FONT_SIZE_TOKENS = ['xs', 'sm', 'base', 'lg', 'xl', '2xl', '3xl', '4xl'] as const;
const LINE_HEIGHT_TOKENS = ['tight', 'base', 'relaxed'] as const;

const THEME_OVERLAYS = {
  white02: 'rgba(255, 255, 255, 0.02)',
  white04: 'rgba(255, 255, 255, 0.04)',
  white05: 'rgba(255, 255, 255, 0.05)',
  white06: 'rgba(255, 255, 255, 0.06)',
  white08: 'rgba(255, 255, 255, 0.08)',
  white10: 'rgba(255, 255, 255, 0.1)',
  white12: 'rgba(255, 255, 255, 0.12)',
  white15: 'rgba(255, 255, 255, 0.15)',
  white20: 'rgba(255, 255, 255, 0.2)',
  white24: 'rgba(255, 255, 255, 0.24)',
  white60: 'rgba(255, 255, 255, 0.6)',
  black06: 'rgba(0, 0, 0, 0.06)',
  black08: 'rgba(0, 0, 0, 0.08)',
  black10: 'rgba(0, 0, 0, 0.1)',
  black12: 'rgba(0, 0, 0, 0.12)',
  black15: 'rgba(0, 0, 0, 0.15)',
  black20: 'rgba(0, 0, 0, 0.2)',
  black25: 'rgba(0, 0, 0, 0.25)',
  black30: 'rgba(0, 0, 0, 0.3)',
  black40: 'rgba(0, 0, 0, 0.4)',
  black50: 'rgba(0, 0, 0, 0.5)',
  black70: 'rgba(0, 0, 0, 0.7)',
  black80: 'rgba(0, 0, 0, 0.8)',
} as const;

const THEME_OVERLAY_TOKEN_VALUES = [
  ['--color-overlay-white-02', THEME_OVERLAYS.white02],
  ['--color-overlay-white-04', THEME_OVERLAYS.white04],
  ['--color-overlay-white-05', THEME_OVERLAYS.white05],
  ['--color-overlay-white-06', THEME_OVERLAYS.white06],
  ['--color-overlay-white-08', THEME_OVERLAYS.white08],
  ['--color-overlay-white-10', THEME_OVERLAYS.white10],
  ['--color-overlay-white-12', THEME_OVERLAYS.white12],
  ['--color-overlay-white-15', THEME_OVERLAYS.white15],
  ['--color-overlay-white-20', THEME_OVERLAYS.white20],
  ['--color-overlay-white-60', THEME_OVERLAYS.white60],
  ['--color-overlay-black-06', THEME_OVERLAYS.black06],
  ['--color-overlay-black-08', THEME_OVERLAYS.black08],
  ['--color-overlay-black-10', THEME_OVERLAYS.black10],
  ['--color-overlay-black-12', THEME_OVERLAYS.black12],
  ['--color-overlay-black-15', THEME_OVERLAYS.black15],
  ['--color-overlay-black-20', THEME_OVERLAYS.black20],
  ['--color-overlay-black-25', THEME_OVERLAYS.black25],
  ['--color-overlay-black-30', THEME_OVERLAYS.black30],
  ['--color-overlay-black-40', THEME_OVERLAYS.black40],
  ['--color-overlay-black-50', THEME_OVERLAYS.black50],
  ['--color-overlay-black-80', THEME_OVERLAYS.black80],
] as const;

declare global {
  // Injected by the desktop webview initialization script. These values let the
  // first renderer pass apply the persisted built-in theme without waiting on a
  // Tauri config round trip. They are absent on plain web/F5 fallback paths.
  var __BITFUN_BOOTSTRAP_THEME_ID__: string | undefined;
  var __BITFUN_BOOTSTRAP_THEME_SELECTION__: string | undefined;
}

/** Space-separated R G B channels for accent alpha composition in component styles. */
function accentColorToRgbChannels(accent: string): string | null {
  const trimmed = accent.trim();
  const hex6 = /^#([0-9a-f]{6})$/i.exec(trimmed);
  if (hex6) {
    const n = parseInt(hex6[1], 16);
    return `${(n >> 16) & 255} ${(n >> 8) & 255} ${n & 255}`;
  }
  const rgb = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/i.exec(trimmed);
  if (rgb) {
    return `${rgb[1]} ${rgb[2]} ${rgb[3]}`;
  }
  return null;
}

function cloneThemeConfig(theme: ThemeConfig): ThemeConfig {
  return JSON.parse(JSON.stringify(theme)) as ThemeConfig;
}

function mergeThemeConfig(base: ThemeConfig, override: Partial<ThemeConfig>): ThemeConfig {
  const mergeValue = (baseValue: unknown, overrideValue: unknown): unknown => {
    if (overrideValue === undefined || overrideValue === null) {
      return baseValue;
    }
    if (Array.isArray(baseValue) || Array.isArray(overrideValue)) {
      return overrideValue;
    }
    if (
      typeof baseValue === 'object' && baseValue !== null &&
      typeof overrideValue === 'object' && overrideValue !== null
    ) {
      const merged: Record<string, unknown> = { ...(baseValue as Record<string, unknown>) };
      Object.entries(overrideValue as Record<string, unknown>).forEach(([key, value]) => {
        merged[key] = mergeValue(merged[key], value);
      });
      return merged;
    }
    return overrideValue;
  };

  return mergeValue(cloneThemeConfig(base), override) as ThemeConfig;
}


export class ThemeService {
  private themes: Map<ThemeId, ThemeConfig> = new Map();
  /** User choice from settings (including follow-system). */
  private themeSelection: ThemeSelectionId = SYSTEM_THEME_ID;
  /** Last value successfully persisted to backend, used to skip redundant writes. */
  private lastSavedSelection: ThemeSelectionId | undefined = undefined;
  /** Currently applied built-in or custom theme (never `system`). */
  private resolvedThemeId: ThemeId = getSystemPreferredDefaultThemeId();
  private systemThemeCleanup: (() => void) | null = null;
  private listeners: Map<ThemeEventType, Set<ThemeEventListener>> = new Map();
  private hooks: ThemeHooks = {};
  private initialized = false;
  private userThemesLoaded = false;
  private userThemesLoadPromise: Promise<void> | null = null;
  private pendingUserThemeSelection: ThemeId | null = null;

  constructor() {
    this.initializeBuiltinThemes();
  }




  private initializeBuiltinThemes(): void {
    builtinThemes.forEach(theme => {
      this.themes.set(theme.id, theme);
    });
    log.info('Loaded builtin themes', { count: builtinThemes.length });
  }


  async initialize(): Promise<void> {
    if (this.initialized) return;
    this.initialized = true;
    try {
      const bootstrapSelection = this.getBootstrapThemeSelection();
      if (bootstrapSelection) {
        await this.applyThemeSelection(bootstrapSelection, { persist: false });
        return;
      }

      const saved = await this.loadThemeSelection();

      if (saved === SYSTEM_THEME_ID) {
        await this.applyThemeSelection(SYSTEM_THEME_ID, { persist: false });
      } else if (saved && this.themes.has(saved)) {
        await this.applyThemeSelection(saved, { persist: false });
      } else if (saved) {
        this.pendingUserThemeSelection = saved;
        await this.ensureUserThemesLoaded();
        if (this.themeSelection === saved) {
          return;
        }
        await this.applyStartupFallbackTheme();
      } else {
        await this.applyStartupFallbackTheme();
      }

    } catch (error) {
      log.error('Theme system initialization failed', error);

      await this.applyThemeSelection(SYSTEM_THEME_ID, { persist: false });
    }
  }


  private async applyStartupFallbackTheme(): Promise<void> {
    const preInjectedThemeId = document.documentElement.getAttribute('data-theme');
    if (preInjectedThemeId && this.themes.has(preInjectedThemeId as ThemeId)) {
      await this.applyThemeSelection(preInjectedThemeId as ThemeId, { persist: false });
    } else {
      await this.applyThemeSelection(SYSTEM_THEME_ID, { persist: false });
    }
  }


  private getBootstrapThemeSelection(): ThemeSelectionId | null {
    const selection = globalThis.__BITFUN_BOOTSTRAP_THEME_SELECTION__;
    if (selection === SYSTEM_THEME_ID) {
      return SYSTEM_THEME_ID;
    }
    if (typeof selection === 'string' && this.themes.has(selection as ThemeId)) {
      return selection as ThemeId;
    }

    return null;
  }


  async ensureUserThemesLoaded(): Promise<void> {
    if (this.userThemesLoaded) {
      await this.applyPendingUserThemeSelection();
      return;
    }
    if (!this.userThemesLoadPromise) {
      this.userThemesLoadPromise = this.loadUserThemes()
        .finally(() => {
          this.userThemesLoaded = true;
          this.userThemesLoadPromise = null;
        });
    }
    await this.userThemesLoadPromise;
    await this.applyPendingUserThemeSelection();
  }


  private async applyPendingUserThemeSelection(): Promise<void> {
    const pending = this.pendingUserThemeSelection;
    if (!pending) {
      return;
    }

    this.pendingUserThemeSelection = null;
    if (!this.themes.has(pending)) {
      log.warn('Saved theme selection was not found after loading user themes', { id: pending });
      return;
    }

    await this.applyThemeSelection(pending, { persist: false });
  }


  private async loadUserThemes(): Promise<void> {
    try {
      // Read the whole themes section so missing optional `custom` does not surface
      // as an expected backend error during startup.
      const themesConfig = await configAPI.getConfig('themes', {
        skipRetryOnNotFound: true,
      }) as { custom?: ThemeConfig[] } | undefined;
      const themes = themesConfig?.custom;

      if (Array.isArray(themes) && themes.length > 0) {
        let loadedCount = 0;
        themes.forEach(theme => {
          try {
            const normalizedTheme = this.normalizeCustomTheme(theme);
            this.themes.set(normalizedTheme.id, normalizedTheme);
            loadedCount += 1;
          } catch (error) {
            log.warn('Skipped invalid user theme', {
              id: theme?.id,
              error: error instanceof Error ? error.message : String(error),
            });
          }
        });
        log.info('Loaded user themes', { count: loadedCount, skipped: themes.length - loadedCount });
      }
    } catch (_error) {

    }
  }


  private async loadThemeSelection(): Promise<ThemeSelectionId | null> {
    try {

      const raw = await configAPI.getConfig('themes.current', {
        skipRetryOnNotFound: true
      }) as string | undefined;

      if (raw === SYSTEM_THEME_ID) {
        return SYSTEM_THEME_ID;
      }
      return raw || null;
    } catch (_error) {
      return null;
    }
  }




  private normalizeCustomTheme(theme: ThemeConfig): ThemeConfig {
    if (!theme || typeof theme !== 'object') {
      throw new Error('Invalid theme: expected object');
    }
    if (!theme.id || theme.id.trim() === '') {
      throw new Error('Theme id cannot be empty');
    }
    if (!theme.name || theme.name.trim() === '') {
      throw new Error(`Invalid theme ${theme.id}: theme name cannot be empty`);
    }
    if (theme.id === SYSTEM_THEME_ID) {
      log.error('Reserved theme id', { id: theme.id });
      throw new Error(`Theme id "${SYSTEM_THEME_ID}" is reserved`);
    }
    if (builtinThemes.some(item => item.id === theme.id)) {
      log.error('Reserved builtin theme id', { id: theme.id });
      throw new Error(`Theme id "${theme.id}" is reserved for a built-in theme`);
    }
    if (!theme.type || !['dark', 'light'].includes(theme.type)) {
      throw new Error(`Invalid theme ${theme.id || '<missing>'}: theme type must be "dark" or "light"`);
    }

    const baseTheme = theme.type === 'light'
      ? builtinThemes.find(item => item.type === 'light') || builtinThemes[0]
      : builtinThemes.find(item => item.id === 'bitfun-dark') || builtinThemes.find(item => item.type === 'dark') || builtinThemes[0];
    const normalized = mergeThemeConfig(baseTheme, theme);
    const validation = this.validateTheme(normalized);

    if (!validation.valid) {
      const detail = validation.errors
        .slice(0, 3)
        .map(error => `${error.path}: ${error.message}`)
        .join('; ');
      throw new Error(`Invalid theme ${theme.id || '<missing>'}: ${detail}`);
    }

    return normalized;
  }


  async registerTheme(theme: ThemeConfig): Promise<void> {
    const normalizedTheme = this.normalizeCustomTheme(theme);
    if (this.themes.has(theme.id)) {
      log.warn('Theme already exists, will override', { id: theme.id });
    }

    this.themes.set(normalizedTheme.id, normalizedTheme);
    this.emitEvent('theme:register', normalizedTheme.id, normalizedTheme);
    log.info('Theme registered', { id: normalizedTheme.id, name: normalizedTheme.name });
    await this.saveUserThemes();
  }


  unregisterTheme(themeId: ThemeId): boolean {
    const theme = this.themes.get(themeId);
    if (!theme) {
      log.warn('Theme not found', { id: themeId });
      return false;
    }


    const isBuiltin = builtinThemes.some(t => t.id === themeId);
    if (isBuiltin) {
      log.error('Cannot delete builtin theme', { id: themeId });
      return false;
    }


    if (this.themeSelection === themeId) {
      void this.applyTheme(SYSTEM_THEME_ID);
    }

    this.themes.delete(themeId);
    this.emitEvent('theme:unregister', themeId, theme);
    log.info('Theme unregistered', { id: themeId, name: theme.name });


    this.saveUserThemes();

    return true;
  }


  getTheme(themeId: ThemeId): ThemeConfig | undefined {
    return this.themes.get(themeId);
  }


  getCurrentTheme(): ThemeConfig {
    return this.themes.get(this.resolvedThemeId) || builtinThemes[0];
  }


  /** User selection for UI (may be `system`). */
  getCurrentThemeId(): ThemeSelectionId {
    return this.themeSelection;
  }

  /** Actually applied theme id (never `system`). */
  getResolvedThemeId(): ThemeId {
    return this.resolvedThemeId;
  }


  getThemeList(): ThemeMetadata[] {
    return Array.from(this.themes.values()).map(theme => ({
      id: theme.id,
      name: theme.name,
      type: theme.type,
      description: theme.description,
      author: theme.author,
      version: theme.version,
      builtin: builtinThemes.some(t => t.id === theme.id),
    }));
  }




  private detachSystemThemeListener(): void {
    if (this.systemThemeCleanup) {
      this.systemThemeCleanup();
      this.systemThemeCleanup = null;
    }
  }

  private attachSystemThemeListener(): void {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return;
    }
    if (this.systemThemeCleanup) {
      return;
    }
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = () => {
      if (this.themeSelection !== SYSTEM_THEME_ID) {
        return;
      }
      const next = getSystemPreferredDefaultThemeId();
      if (next === this.resolvedThemeId) {
        return;
      }
      void this.applyResolvedTheme(next);
    };
    mq.addEventListener('change', handler);
    this.systemThemeCleanup = () => mq.removeEventListener('change', handler);
  }

  private async applyResolvedTheme(resolvedId: ThemeId): Promise<void> {
    const theme = this.themes.get(resolvedId);
    if (!theme) {
      log.error('Theme not found', { id: resolvedId });
      throw new Error(`Theme ${resolvedId} not found`);
    }

    const oldTheme = this.getCurrentTheme();

    try {
      if (this.hooks.beforeChange) {
        await this.hooks.beforeChange(theme, oldTheme);
      }
      this.emitEvent('theme:before-change', resolvedId, theme, oldTheme);

      this.resolvedThemeId = resolvedId;

      this.injectCSSVariables(theme);

      try {
        monacoThemeSync.syncTheme(theme);
      } catch (error) {
        log.warn('Monaco Editor theme sync failed', error);
      }

      if (this.hooks.afterChange) {
        await this.hooks.afterChange(theme, oldTheme);
      }
      this.emitEvent('theme:after-change', resolvedId, theme, oldTheme);

      log.info('Theme applied', { id: resolvedId, name: theme.name, selection: this.themeSelection });
    } catch (error) {
      log.error('Failed to apply theme', error);
      throw error;
    }
  }

  private async applyThemeSelection(
    themeId: ThemeId | typeof SYSTEM_THEME_ID,
    options: { persist: boolean },
  ): Promise<void> {
    if (themeId !== SYSTEM_THEME_ID && !this.themes.has(themeId)) {
      log.error('Theme not found', { id: themeId });
      throw new Error(`Theme ${themeId} not found`);
    }

    this.detachSystemThemeListener();

    if (themeId === SYSTEM_THEME_ID) {
      this.themeSelection = SYSTEM_THEME_ID;
      if (options.persist) {
        await this.saveThemeSelection(SYSTEM_THEME_ID);
      } else {
        this.lastSavedSelection = SYSTEM_THEME_ID;
      }
      this.attachSystemThemeListener();
      const resolved = getSystemPreferredDefaultThemeId();
      await this.applyResolvedTheme(resolved);
    } else {
      this.themeSelection = themeId;
      if (options.persist) {
        await this.saveThemeSelection(themeId);
      } else {
        this.lastSavedSelection = themeId;
      }
      await this.applyResolvedTheme(themeId);
    }
  }

  async applyTheme(themeId: ThemeId | typeof SYSTEM_THEME_ID): Promise<void> {
    await this.applyThemeSelection(themeId, { persist: true });
  }


  private injectCSSVariables(theme: ThemeConfig): void {
    const root = document.documentElement;
    const { colors, effects, motion, typography } = theme;


    root.style.setProperty('--color-bg-primary', colors.background.primary);
    root.style.setProperty('--color-static-white', THEME_STATIC_COLORS.white);
    root.style.setProperty('--color-static-black', THEME_STATIC_COLORS.black);
    root.style.setProperty('--color-static-white-rgb', '255, 255, 255');
    root.style.setProperty('--color-static-black-rgb', '0, 0, 0');
    THEME_OVERLAY_TOKEN_VALUES.forEach(([name, value]) => {
      root.style.setProperty(name, value);
    });
    root.style.setProperty('--color-bg-secondary', colors.background.secondary);
    root.style.setProperty('--color-bg-tertiary', colors.background.tertiary);
    root.style.setProperty('--color-bg-quaternary', colors.background.quaternary);
    root.style.setProperty('--color-bg-elevated', colors.background.elevated);
    root.style.setProperty('--color-bg-workbench', colors.background.workbench);
    root.style.setProperty('--color-bg-scene', colors.background.scene);
    if (colors.background.tooltip) {
      root.style.setProperty('--color-bg-tooltip', colors.background.tooltip);
    }

    root.style.setProperty('--color-text-primary', colors.text.primary);
    root.style.setProperty('--color-text-secondary', colors.text.secondary);
    root.style.setProperty('--color-text-muted', colors.text.muted);
    root.style.setProperty('--color-text-disabled', colors.text.disabled);


    ACCENT_STOPS.forEach((key) => {
      const value = colors.accent[key];
      root.style.setProperty(`--color-accent-${key}`, value);
    });

    const primaryAccent = colors.accent[500];
    const flowChatLinkColors = theme.type === 'light'
      ? FLOW_CHAT_LINK_COLORS.light
      : FLOW_CHAT_LINK_COLORS.dark;
    root.style.setProperty('--flowchat-link-color', flowChatLinkColors.default);
    root.style.setProperty('--flowchat-link-hover-color', flowChatLinkColors.hover);
    const accentRgb = accentColorToRgbChannels(primaryAccent);
    if (accentRgb) {
      root.style.setProperty('--color-accent-500-rgb', accentRgb);
    }


    const secondaryAccent = colors.purple;
    if (secondaryAccent) {
      SECONDARY_ACCENT_STOPS.forEach((key) => {
        const value = secondaryAccent[key];
        root.style.setProperty(`--color-purple-${key}`, value);
      });
    }


    root.style.setProperty('--color-success', colors.semantic.success);
    root.style.setProperty('--color-success-bg', colors.semantic.successBg);
    root.style.setProperty('--color-success-border', colors.semantic.successBorder);
    root.style.setProperty('--color-warning', colors.semantic.warning);
    root.style.setProperty('--color-warning-bg', colors.semantic.warningBg);
    root.style.setProperty('--color-warning-border', colors.semantic.warningBorder);
    root.style.setProperty('--color-error', colors.semantic.error);
    root.style.setProperty('--color-error-bg', colors.semantic.errorBg);
    root.style.setProperty('--color-error-border', colors.semantic.errorBorder);
    root.style.setProperty('--color-info', colors.semantic.info);
    root.style.setProperty('--color-info-bg', colors.semantic.infoBg);
    root.style.setProperty('--color-info-border', colors.semantic.infoBorder);


    root.style.setProperty('--border-subtle', colors.border.subtle);
    root.style.setProperty('--border-base', colors.border.base);
    root.style.setProperty('--border-medium', colors.border.medium);
    root.style.setProperty('--border-strong', colors.border.strong);
    root.style.setProperty('--border-prominent', colors.border.prominent);

    const sceneViewportBorder = theme.layout?.sceneViewportBorder ?? true;
    root.style.setProperty(
        '--scene-viewport-border-width',
        sceneViewportBorder ? '1px' : '0'
    );

    root.style.setProperty('--element-bg-subtle', colors.element.subtle);
    root.style.setProperty('--element-bg-soft', colors.element.soft);
    root.style.setProperty('--element-bg-base', colors.element.base);
    root.style.setProperty('--element-bg-medium', colors.element.medium);
    root.style.setProperty('--element-bg-strong', colors.element.strong);
    root.style.setProperty('--element-bg-elevated', colors.element.elevated);
    root.style.setProperty('--element-bg-hover', colors.element.medium);


    root.style.setProperty('--git-color-branch', colors.git.branch);
    root.style.setProperty('--git-color-branch-bg', colors.git.branchBg);
    root.style.setProperty('--git-color-changes', colors.git.changes);
    root.style.setProperty('--git-color-deleted', colors.git.deleted);
    root.style.setProperty('--git-color-deleted-bg', colors.git.deletedBg);
    root.style.setProperty('--git-color-staged', colors.git.staged);
    root.style.setProperty('--git-color-staged-bg', colors.git.stagedBg);




    const scrollbarThumb = colors.scrollbar?.thumb ?? (
        theme.type === 'dark'
            ? THEME_OVERLAYS.white12
            : THEME_OVERLAYS.black15
    );
    const scrollbarThumbHover = colors.scrollbar?.thumbHover ?? (
        theme.type === 'dark'
            ? THEME_OVERLAYS.white24
            : THEME_OVERLAYS.black30
    );
    root.style.setProperty('--scrollbar-thumb', scrollbarThumb);
    root.style.setProperty('--scrollbar-thumb-hover', scrollbarThumbHover);
    root.style.setProperty('--color-scrollbar', scrollbarThumb);


    const shadows = effects?.shadow;
    if (shadows) {
      SHADOW_TOKENS.forEach((key) => {
        const value = shadows[key];
        root.style.setProperty(`--shadow-${key}`, value);
      });
      root.style.setProperty('--glass-shadow-sm', shadows.sm);
      root.style.setProperty('--glass-shadow-base', shadows.base);
      root.style.setProperty('--glass-shadow-lg', shadows.lg);
      root.style.setProperty('--glass-shadow-xl', shadows.xl);
    }


    const blurs = effects?.blur;
    if (blurs) {
      BLUR_TOKENS.forEach((key) => {
        const value = blurs[key];
        root.style.setProperty(`--blur-${key}`, value);
      });
      root.style.setProperty('--glass-blur-sm', blurs.subtle);
      root.style.setProperty('--glass-blur-base', blurs.base);
    }


    const radii = effects?.radius;
    if (radii) {
      RADIUS_TOKENS.forEach((key) => {
        const value = radii[key];
        root.style.setProperty(`--size-radius-${key}`, value);
      });
      if (radii.base) {
        root.style.setProperty('--size-radius-md', radii.base);
      }
    }


    const spacing = effects?.spacing;
    if (spacing) {
      SPACING_TOKENS.forEach((key) => {
        const value = spacing[key];
        root.style.setProperty(`--size-gap-${key}`, value);
      });
    }


    if (effects?.opacity) {
      root.style.setProperty('--opacity-disabled', String(effects.opacity.disabled));
      root.style.setProperty('--opacity-hover', String(effects.opacity.hover));
      root.style.setProperty('--opacity-focus', String(effects.opacity.focus));
    }


    const motionDuration = motion?.duration;
    if (motionDuration) {
      MOTION_DURATION_TOKENS.forEach((key) => {
        const value = motionDuration[key];
        root.style.setProperty(`--motion-${key}`, value);
      });
    }


    const motionEasing = motion?.easing;
    if (motionEasing) {
      EASING_TOKENS.forEach((key) => {
        const value = motionEasing[key];
        root.style.setProperty(`--easing-${key}`, value);
      });
    }


    if (typography?.font) {
      root.style.setProperty('--font-family-sans', typography.font.sans);
      root.style.setProperty('--font-family-mono', typography.font.mono);
    }


    const fontWeights = typography?.weight;
    if (fontWeights) {
      FONT_WEIGHT_TOKENS.forEach((key) => {
        const value = fontWeights[key];
        root.style.setProperty(`--font-weight-${key}`, String(value));
      });
    }


    const typographySize = typography?.size;
    if (typographySize) {
      FONT_SIZE_TOKENS.forEach((key) => {
        const value = typographySize[key];
        root.style.setProperty(`--font-size-${key}`, value);
      });
    }


    const lineHeights = typography?.lineHeight;
    if (lineHeights) {
      LINE_HEIGHT_TOKENS.forEach((key) => {
        const value = lineHeights[key];
        root.style.setProperty(`--line-height-${key}`, String(value));
      });
    }





    const buttonConfig = theme.components?.button;
    if (buttonConfig) {

      root.style.setProperty('--btn-primary-bg', buttonConfig.primary.default.background);
      root.style.setProperty('--btn-primary-color', buttonConfig.primary.default.color);
      root.style.setProperty('--btn-primary-border', buttonConfig.primary.default.border);
      root.style.setProperty('--btn-primary-shadow', buttonConfig.primary.default.shadow || 'none');

      root.style.setProperty('--btn-primary-hover-bg', buttonConfig.primary.hover.background);
      root.style.setProperty('--btn-primary-hover-color', buttonConfig.primary.hover.color);
      root.style.setProperty('--btn-primary-hover-border', buttonConfig.primary.hover.border);
      root.style.setProperty('--btn-primary-hover-shadow', buttonConfig.primary.hover.shadow || 'none');
      root.style.setProperty('--btn-primary-hover-transform', buttonConfig.primary.hover.transform || 'none');

      root.style.setProperty('--btn-primary-active-bg', buttonConfig.primary.active.background);
      root.style.setProperty('--btn-primary-active-color', buttonConfig.primary.active.color);
      root.style.setProperty('--btn-primary-active-border', buttonConfig.primary.active.border);
      root.style.setProperty('--btn-primary-active-shadow', buttonConfig.primary.active.shadow || 'none');
      root.style.setProperty('--btn-primary-active-transform', buttonConfig.primary.active.transform || 'none');


      root.style.setProperty('--btn-ghost-color', buttonConfig.ghost.default.color);

      root.style.setProperty('--btn-ghost-hover-bg', buttonConfig.ghost.hover.background);
      root.style.setProperty('--btn-ghost-hover-color', buttonConfig.ghost.hover.color);
      root.style.setProperty('--btn-ghost-hover-border', buttonConfig.ghost.hover.border);
    } else {

      const a = colors.accent;
      root.style.setProperty('--btn-primary-bg', a[200]);
      root.style.setProperty('--btn-primary-color', a[600]);
      root.style.setProperty('--btn-primary-border', 'transparent');
      root.style.setProperty('--btn-primary-shadow', 'none');
      root.style.setProperty('--btn-primary-hover-bg', a[300]);
      root.style.setProperty('--btn-primary-hover-color', colors.text.primary);
      root.style.setProperty('--btn-primary-hover-border', 'transparent');
      root.style.setProperty('--btn-primary-hover-shadow', 'none');
      root.style.setProperty('--btn-primary-hover-transform', 'none');
      root.style.setProperty('--btn-primary-active-bg', a[200]);
      root.style.setProperty('--btn-primary-active-color', colors.text.primary);
      root.style.setProperty('--btn-primary-active-border', 'transparent');
      root.style.setProperty('--btn-primary-active-shadow', 'none');
      root.style.setProperty('--btn-primary-active-transform', 'none');
      root.style.setProperty('--btn-ghost-color', colors.text.muted);
      root.style.setProperty('--btn-ghost-hover-bg', colors.element.subtle);
      root.style.setProperty('--btn-ghost-hover-color', colors.text.primary);
      root.style.setProperty('--btn-ghost-hover-border', 'transparent');
    }


    root.style.setProperty(
      '--window-control-close-hover-color',
      theme.components?.windowControls?.close.hoverColor ?? colors.semantic.error,
    );


    root.style.setProperty('--input-bg', colors.element.base);
    root.style.setProperty('--input-bg-hover', colors.element.medium);
    root.style.setProperty('--input-border', colors.border.base);
    root.style.setProperty('--input-border-hover', colors.border.medium);
    root.style.setProperty('--input-border-focus', colors.accent[400]);
    root.style.setProperty('--input-text', colors.text.primary);


    if (theme.type === 'dark') {

      root.style.setProperty('--card-bg-default', THEME_OVERLAYS.white04);
      root.style.setProperty('--card-bg-elevated', THEME_OVERLAYS.white04);
      root.style.setProperty('--card-bg-subtle', THEME_OVERLAYS.white02);
      root.style.setProperty('--card-bg-hover', THEME_OVERLAYS.white04);
      root.style.setProperty('--card-bg-active', THEME_OVERLAYS.white05);
      root.style.setProperty('--card-bg-accent', THEME_OVERLAYS.white08);
      root.style.setProperty('--card-bg-accent-hover', THEME_OVERLAYS.white12);
      root.style.setProperty('--card-bg-purple', 'rgba(139, 92, 246, 0.08)');
      root.style.setProperty('--card-bg-purple-hover', 'rgba(139, 92, 246, 0.15)');
    } else {

      root.style.setProperty('--card-bg-default', THEME_OVERLAYS.black06);
      root.style.setProperty('--card-bg-elevated', THEME_OVERLAYS.black08);
      root.style.setProperty('--card-bg-subtle', 'transparent');
      root.style.setProperty('--card-bg-hover', THEME_OVERLAYS.black06);
      root.style.setProperty('--card-bg-active', THEME_OVERLAYS.black10);
      root.style.setProperty('--card-bg-accent', 'rgba(15, 23, 42, 0.08)');
      root.style.setProperty('--card-bg-accent-hover', 'rgba(15, 23, 42, 0.12)');
      root.style.setProperty('--card-bg-purple', 'rgba(124, 58, 237, 0.12)');
      root.style.setProperty('--card-bg-purple-hover', 'rgba(139, 92, 246, 0.15)');
    }


    root.style.setProperty('--panel-bg', colors.background.primary);


    root.setAttribute('data-theme', theme.id);
    root.setAttribute('data-theme-type', theme.type);

    const bgPrimary = colors.background.primary;
    root.style.setProperty('--bitfun-startup-bg', bgPrimary);
    root.style.backgroundColor = bgPrimary;
    if (document.body) {
      document.body.style.backgroundColor = bgPrimary;
    }
  }


  private async saveThemeSelection(selection: ThemeSelectionId): Promise<void> {
    if (this.lastSavedSelection === selection) {
      return;
    }
    this.lastSavedSelection = selection;
    try {
      await configAPI.setConfig('themes.current', selection);
    } catch (error) {
      this.lastSavedSelection = undefined;
      log.warn('Failed to save current theme ID', error);
    }
  }


  private async saveUserThemes(): Promise<void> {
    try {
      const userThemes = Array.from(this.themes.values()).filter(
          theme => !builtinThemes.some(t => t.id === theme.id)
      );
      await configAPI.setConfig('themes.custom', userThemes);
    } catch (error) {
      log.warn('Failed to save user themes', error);
    }
  }




  exportTheme(themeId: ThemeId): ThemeExport | null {
    const theme = this.themes.get(themeId);
    if (!theme) {
      log.error('Theme not found', { id: themeId });
      return null;
    }

    const validation = this.validateTheme(theme);
    if (!validation.valid) {
      log.error('Cannot export invalid theme', {
        id: themeId,
        errors: validation.errors.slice(0, 3),
      });
      return null;
    }

    const metadata: ThemeMetadata = {
      id: theme.id,
      name: theme.name,
      type: theme.type,
      description: theme.description,
      author: theme.author,
      version: theme.version,
      builtin: builtinThemes.some(t => t.id === theme.id),
    };

    return {
      schema: '2.0.0',
      theme: cloneThemeConfig(theme),
      metadata,
      exportedAt: new Date().toISOString(),
    };
  }




  validateTheme(theme: ThemeConfig): ThemeValidationResult {
    return themeValidator.validate(theme);
  }




  on(eventType: ThemeEventType, listener: ThemeEventListener): () => void {
    if (!this.listeners.has(eventType)) {
      this.listeners.set(eventType, new Set());
    }

    this.listeners.get(eventType)!.add(listener);


    return () => {
      this.listeners.get(eventType)?.delete(listener);
    };
  }


  private emitEvent(
      type: ThemeEventType,
      themeId: ThemeId,
      theme?: ThemeConfig,
      previousTheme?: ThemeConfig
  ): void {
    const event: ThemeEvent = {
      type,
      themeId,
      theme,
      previousTheme,
      timestamp: Date.now(),
    };

    const listeners = this.listeners.get(type);
    if (listeners) {
      listeners.forEach(listener => {
        try {
          listener(event);
        } catch (error) {
          log.error('Event listener execution failed', { type, error });
        }
      });
    }
  }




  registerHooks(hooks: ThemeHooks): void {
    this.hooks = { ...this.hooks, ...hooks };
  }
}


export const themeService = new ThemeService();

