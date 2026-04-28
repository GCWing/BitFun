

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
import { configAPI } from '@/infrastructure/api';
import { monacoThemeSync } from '../integrations/MonacoThemeSync';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ThemeService');

/** Space-separated R G B for `rgba(var(--color-primary-rgb) / α)` in component styles. */
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
      const saved = await this.loadThemeSelection();

      if (saved === SYSTEM_THEME_ID) {
        await this.applyTheme(SYSTEM_THEME_ID);
      } else if (saved && this.themes.has(saved)) {
        await this.applyTheme(saved);
      } else {
        const preInjectedThemeId = document.documentElement.getAttribute('data-theme');
        if (preInjectedThemeId && this.themes.has(preInjectedThemeId as ThemeId)) {
          await this.applyTheme(preInjectedThemeId as ThemeId);
        } else {
          await this.applyTheme(SYSTEM_THEME_ID);
        }
      }

      this.loadUserThemes().catch(() => {

      });
    } catch (error) {
      log.error('Theme system initialization failed', error);

      await this.applyTheme(SYSTEM_THEME_ID);
    }
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
        themes.forEach(theme => {
          this.themes.set(theme.id, theme);
        });
        log.info('Loaded user themes', { count: themes.length });
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




  registerTheme(theme: ThemeConfig): void {
    if (theme.id === SYSTEM_THEME_ID) {
      log.error('Reserved theme id', { id: theme.id });
      throw new Error(`Theme id "${SYSTEM_THEME_ID}" is reserved`);
    }
    if (this.themes.has(theme.id)) {
      log.warn('Theme already exists, will override', { id: theme.id });
    }

    this.themes.set(theme.id, theme);
    this.emitEvent('theme:register', theme.id, theme);
    log.info('Theme registered', { id: theme.id, name: theme.name });
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

  async applyTheme(themeId: ThemeId | typeof SYSTEM_THEME_ID): Promise<void> {
    if (themeId !== SYSTEM_THEME_ID && !this.themes.has(themeId)) {
      log.error('Theme not found', { id: themeId });
      throw new Error(`Theme ${themeId} not found`);
    }

    this.detachSystemThemeListener();

    if (themeId === SYSTEM_THEME_ID) {
      this.themeSelection = SYSTEM_THEME_ID;
      await this.saveThemeSelection(SYSTEM_THEME_ID);
      this.attachSystemThemeListener();
      const resolved = getSystemPreferredDefaultThemeId();
      await this.applyResolvedTheme(resolved);
    } else {
      this.themeSelection = themeId;
      await this.saveThemeSelection(themeId);
      await this.applyResolvedTheme(themeId);
    }
  }


  private injectCSSVariables(theme: ThemeConfig): void {
    const root = document.documentElement;
    const { colors, effects, motion, typography } = theme;


    root.style.setProperty('--color-bg-primary', colors.background.primary);
    root.style.setProperty('--color-bg-secondary', colors.background.secondary);
    root.style.setProperty('--color-bg-tertiary', colors.background.tertiary);
    root.style.setProperty('--color-bg-quaternary', colors.background.quaternary);
    root.style.setProperty('--color-bg-elevated', colors.background.elevated);
    root.style.setProperty('--color-bg-workbench', colors.background.workbench);
    root.style.setProperty('--color-bg-scene', colors.background.scene);
    root.style.setProperty('--color-bg-flowchat', colors.background.scene);
    if (colors.background.tooltip) {
      root.style.setProperty('--color-bg-tooltip', colors.background.tooltip);
    }

    root.style.setProperty('--color-overlay', theme.type === 'dark' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(0, 0, 0, 0.3)');


    root.style.setProperty('--color-text-primary', colors.text.primary);
    root.style.setProperty('--color-text-secondary', colors.text.secondary);
    root.style.setProperty('--color-text-muted', colors.text.muted);
    root.style.setProperty('--color-text-disabled', colors.text.disabled);


    Object.entries(colors.accent).forEach(([key, value]) => {
      root.style.setProperty(`--color-accent-${key}`, value);
    });

    const primaryAccent = colors.accent[500];
    const primaryHover = colors.accent[600];
    root.style.setProperty('--color-primary', primaryAccent);
    root.style.setProperty('--color-primary-hover', primaryHover);
    root.style.setProperty('--color-accent', primaryAccent);
    root.style.setProperty('--color-accent-primary', primaryAccent);
    const primaryRgb = accentColorToRgbChannels(primaryAccent);
    if (primaryRgb) {
      root.style.setProperty('--color-primary-rgb', primaryRgb);
    }


    if (colors.purple) {
      Object.entries(colors.purple).forEach(([key, value]) => {
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
    root.style.setProperty('--color-highlight', colors.semantic.highlight);
    root.style.setProperty('--color-highlight-bg', colors.semantic.highlightBg);


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


    root.style.setProperty('--git-color-branch', colors.git.branch);
    root.style.setProperty('--git-color-branch-bg', colors.git.branchBg);
    root.style.setProperty('--git-color-changes', colors.git.changes);
    root.style.setProperty('--git-color-changes-bg', colors.git.changesBg);
    root.style.setProperty('--git-color-added', colors.git.added);
    root.style.setProperty('--git-color-added-bg', colors.git.addedBg);
    root.style.setProperty('--git-color-deleted', colors.git.deleted);
    root.style.setProperty('--git-color-deleted-bg', colors.git.deletedBg);
    root.style.setProperty('--git-color-staged', colors.git.staged);
    root.style.setProperty('--git-color-staged-bg', colors.git.stagedBg);




    const scrollbarThumb = colors.scrollbar?.thumb ?? (
        theme.type === 'dark'
            ? 'rgba(255, 255, 255, 0.12)'
            : 'rgba(0, 0, 0, 0.15)'
    );
    const scrollbarThumbHover = colors.scrollbar?.thumbHover ?? (
        theme.type === 'dark'
            ? 'rgba(255, 255, 255, 0.22)'
            : 'rgba(0, 0, 0, 0.28)'
    );
    root.style.setProperty('--scrollbar-thumb', scrollbarThumb);
    root.style.setProperty('--scrollbar-thumb-hover', scrollbarThumbHover);


    if (effects?.shadow) {
      Object.entries(effects.shadow).forEach(([key, value]) => {
        root.style.setProperty(`--shadow-${key}`, value);
      });
    }


    if (effects?.glow) {
      root.style.setProperty('--glow-blue', effects.glow.blue);
      root.style.setProperty('--glow-purple', effects.glow.purple);
      root.style.setProperty('--glow-mixed', effects.glow.mixed);
    }


    if (effects?.blur) {
      Object.entries(effects.blur).forEach(([key, value]) => {
        root.style.setProperty(`--blur-${key}`, value);
      });
    }


    if (effects?.radius) {
      Object.entries(effects.radius).forEach(([key, value]) => {
        root.style.setProperty(`--radius-${key}`, value);
      });
    }


    if (effects?.spacing) {
      Object.entries(effects.spacing).forEach(([key, value]) => {
        root.style.setProperty(`--spacing-${key}`, value);
      });
    }


    if (effects?.opacity) {
      root.style.setProperty('--opacity-disabled', String(effects.opacity.disabled));
      root.style.setProperty('--opacity-hover', String(effects.opacity.hover));
      root.style.setProperty('--opacity-focus', String(effects.opacity.focus));
      root.style.setProperty('--opacity-overlay', String(effects.opacity.overlay));
    }


    if (motion?.duration) {
      Object.entries(motion.duration).forEach(([key, value]) => {
        root.style.setProperty(`--motion-${key}`, value);
      });
    }


    if (motion?.easing) {
      Object.entries(motion.easing).forEach(([key, value]) => {
        root.style.setProperty(`--easing-${key}`, value);
      });
    }


    if (typography?.font) {
      root.style.setProperty('--font-sans', typography.font.sans);
      root.style.setProperty('--font-mono', typography.font.mono);
    }


    if (typography?.weight) {
      Object.entries(typography.weight).forEach(([key, value]) => {
        root.style.setProperty(`--font-weight-${key}`, String(value));
      });
    }


    if (typography?.size) {
      Object.entries(typography.size).forEach(([key, value]) => {
        root.style.setProperty(`--font-size-${key}`, value);
      });
    }


    if (typography?.lineHeight) {
      Object.entries(typography.lineHeight).forEach(([key, value]) => {
        root.style.setProperty(`--line-height-${key}`, String(value));
      });
    }





    const buttonConfig = theme.components?.button;
    if (buttonConfig) {

      root.style.setProperty('--btn-default-bg', buttonConfig.default.background);
      root.style.setProperty('--btn-default-color', buttonConfig.default.color);
      root.style.setProperty('--btn-default-border', buttonConfig.default.border);
      root.style.setProperty('--btn-default-shadow', buttonConfig.default.shadow || 'none');

      root.style.setProperty('--btn-default-hover-bg', buttonConfig.hover.background);
      root.style.setProperty('--btn-default-hover-color', buttonConfig.hover.color);
      root.style.setProperty('--btn-default-hover-border', buttonConfig.hover.border);
      root.style.setProperty('--btn-default-hover-shadow', buttonConfig.hover.shadow || 'none');
      root.style.setProperty('--btn-default-hover-transform', buttonConfig.hover.transform || 'none');

      root.style.setProperty('--btn-default-active-bg', buttonConfig.active.background);
      root.style.setProperty('--btn-default-active-color', buttonConfig.active.color);
      root.style.setProperty('--btn-default-active-border', buttonConfig.active.border);
      root.style.setProperty('--btn-default-active-shadow', buttonConfig.active.shadow || 'none');
      root.style.setProperty('--btn-default-active-transform', buttonConfig.active.transform || 'none');


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


      root.style.setProperty('--btn-ghost-bg', buttonConfig.ghost.default.background);
      root.style.setProperty('--btn-ghost-color', buttonConfig.ghost.default.color);
      root.style.setProperty('--btn-ghost-border', buttonConfig.ghost.default.border);
      root.style.setProperty('--btn-ghost-shadow', buttonConfig.ghost.default.shadow || 'none');

      root.style.setProperty('--btn-ghost-hover-bg', buttonConfig.ghost.hover.background);
      root.style.setProperty('--btn-ghost-hover-color', buttonConfig.ghost.hover.color);
      root.style.setProperty('--btn-ghost-hover-border', buttonConfig.ghost.hover.border);
      root.style.setProperty('--btn-ghost-hover-shadow', buttonConfig.ghost.hover.shadow || 'none');
      root.style.setProperty('--btn-ghost-hover-transform', buttonConfig.ghost.hover.transform || 'none');

      root.style.setProperty('--btn-ghost-active-bg', buttonConfig.ghost.active.background);
      root.style.setProperty('--btn-ghost-active-color', buttonConfig.ghost.active.color);
      root.style.setProperty('--btn-ghost-active-border', buttonConfig.ghost.active.border);
      root.style.setProperty('--btn-ghost-active-shadow', buttonConfig.ghost.active.shadow || 'none');
      root.style.setProperty('--btn-ghost-active-transform', buttonConfig.ghost.active.transform || 'none');
    } else {

      root.style.setProperty('--btn-default-bg', colors.element.base);
      root.style.setProperty('--btn-default-color', colors.text.secondary);
      root.style.setProperty('--btn-default-border', colors.border.base);
      root.style.setProperty('--btn-default-shadow', 'none');
      root.style.setProperty('--btn-default-hover-bg', colors.element.medium);
      root.style.setProperty('--btn-default-hover-color', colors.text.primary);
      root.style.setProperty('--btn-default-hover-border', colors.border.medium);
      root.style.setProperty('--btn-default-hover-shadow', 'none');
      root.style.setProperty('--btn-default-hover-transform', 'none');

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
    }


    const windowControlsConfig = theme.components?.windowControls;
    if (windowControlsConfig) {

      root.style.setProperty('--window-control-minimize-dot', windowControlsConfig.minimize.dot);
      root.style.setProperty('--window-control-minimize-dot-shadow', windowControlsConfig.minimize.dotShadow || 'none');
      root.style.setProperty('--window-control-minimize-hover-bg', windowControlsConfig.minimize.hoverBg);
      root.style.setProperty('--window-control-minimize-hover-color', windowControlsConfig.minimize.hoverColor);
      root.style.setProperty('--window-control-minimize-hover-border', windowControlsConfig.minimize.hoverBorder);
      root.style.setProperty('--window-control-minimize-hover-shadow', windowControlsConfig.minimize.hoverShadow || 'none');


      root.style.setProperty('--window-control-maximize-dot', windowControlsConfig.maximize.dot);
      root.style.setProperty('--window-control-maximize-dot-shadow', windowControlsConfig.maximize.dotShadow || 'none');
      root.style.setProperty('--window-control-maximize-hover-bg', windowControlsConfig.maximize.hoverBg);
      root.style.setProperty('--window-control-maximize-hover-color', windowControlsConfig.maximize.hoverColor);
      root.style.setProperty('--window-control-maximize-hover-border', windowControlsConfig.maximize.hoverBorder);
      root.style.setProperty('--window-control-maximize-hover-shadow', windowControlsConfig.maximize.hoverShadow || 'none');


      root.style.setProperty('--window-control-close-dot', windowControlsConfig.close.dot);
      root.style.setProperty('--window-control-close-dot-shadow', windowControlsConfig.close.dotShadow || 'none');
      root.style.setProperty('--window-control-close-hover-bg', windowControlsConfig.close.hoverBg);
      root.style.setProperty('--window-control-close-hover-color', windowControlsConfig.close.hoverColor);
      root.style.setProperty('--window-control-close-hover-border', windowControlsConfig.close.hoverBorder);
      root.style.setProperty('--window-control-close-hover-shadow', windowControlsConfig.close.hoverShadow || 'none');


      root.style.setProperty('--window-control-default-color', windowControlsConfig.common.defaultColor);
      root.style.setProperty('--window-control-default-dot', windowControlsConfig.common.defaultDot);
      root.style.setProperty('--window-control-disabled-dot', windowControlsConfig.common.disabledDot);
      root.style.setProperty('--window-control-flow-gradient', windowControlsConfig.common.flowGradient || 'none');
    } else {

      root.style.setProperty('--window-control-minimize-dot', colors.accent[400]);
      root.style.setProperty('--window-control-minimize-dot-shadow', 'none');
      root.style.setProperty('--window-control-minimize-hover-bg', colors.accent[100]);
      root.style.setProperty('--window-control-minimize-hover-color', colors.accent[500]);
      root.style.setProperty('--window-control-minimize-hover-border', colors.accent[200]);
      root.style.setProperty('--window-control-minimize-hover-shadow', 'none');

      root.style.setProperty('--window-control-maximize-dot', colors.accent[400]);
      root.style.setProperty('--window-control-maximize-dot-shadow', 'none');
      root.style.setProperty('--window-control-maximize-hover-bg', colors.accent[100]);
      root.style.setProperty('--window-control-maximize-hover-color', colors.accent[500]);
      root.style.setProperty('--window-control-maximize-hover-border', colors.accent[200]);
      root.style.setProperty('--window-control-maximize-hover-shadow', 'none');

      root.style.setProperty('--window-control-close-dot', colors.semantic.error);
      root.style.setProperty('--window-control-close-dot-shadow', 'none');
      root.style.setProperty('--window-control-close-hover-bg', colors.semantic.errorBg);
      root.style.setProperty('--window-control-close-hover-color', colors.semantic.error);
      root.style.setProperty('--window-control-close-hover-border', colors.semantic.errorBorder);
      root.style.setProperty('--window-control-close-hover-shadow', 'none');

      root.style.setProperty('--window-control-default-color', colors.text.primary);
      root.style.setProperty('--window-control-default-dot', colors.text.muted);
      root.style.setProperty('--window-control-disabled-dot', colors.text.disabled);
      root.style.setProperty('--window-control-flow-gradient', 'none');
    }


    root.style.setProperty('--input-bg', colors.element.base);
    root.style.setProperty('--input-bg-hover', colors.element.medium);
    root.style.setProperty('--input-bg-focus', colors.element.soft);
    root.style.setProperty('--input-bg-disabled', colors.element.subtle);
    root.style.setProperty('--input-border', colors.border.base);
    root.style.setProperty('--input-border-hover', colors.border.medium);
    root.style.setProperty('--input-border-focus', colors.accent[400]);
    root.style.setProperty('--input-border-error', colors.semantic.error);
    root.style.setProperty('--input-text', colors.text.primary);
    root.style.setProperty(
        '--input-placeholder',
        'color-mix(in srgb, var(--color-text-muted) 40%, var(--color-bg-primary))'
    );


    root.style.setProperty('--card-bg', colors.element.base);
    root.style.setProperty('--card-bg-hover', colors.element.medium);
    root.style.setProperty('--card-bg-active', colors.element.elevated);
    root.style.setProperty('--card-border', colors.border.base);
    root.style.setProperty('--card-border-hover', colors.border.medium);
    root.style.setProperty('--card-border-active', colors.accent[300]);


    if (theme.type === 'dark') {

      root.style.setProperty('--card-bg-default', 'rgba(255, 255, 255, 0.025)');
      root.style.setProperty('--card-bg-elevated', 'rgba(255, 255, 255, 0.035)');
      root.style.setProperty('--card-bg-subtle', 'rgba(255, 255, 255, 0.015)');
      root.style.setProperty('--card-bg-hover', 'rgba(255, 255, 255, 0.04)');
      root.style.setProperty('--card-bg-active', 'rgba(255, 255, 255, 0.05)');
      root.style.setProperty('--card-bg-accent', 'rgba(255, 255, 255, 0.09)');
      root.style.setProperty('--card-bg-accent-hover', 'rgba(255, 255, 255, 0.13)');
      root.style.setProperty('--card-bg-purple', 'rgba(139, 92, 246, 0.08)');
      root.style.setProperty('--card-bg-purple-hover', 'rgba(139, 92, 246, 0.12)');
    } else {

      root.style.setProperty('--card-bg-default', 'rgba(0, 0, 0, 0.06)');
      root.style.setProperty('--card-bg-elevated', 'rgba(0, 0, 0, 0.08)');
      root.style.setProperty('--card-bg-subtle', 'rgba(0, 0, 0, 0.04)');
      root.style.setProperty('--card-bg-hover', 'rgba(0, 0, 0, 0.065)');
      root.style.setProperty('--card-bg-active', 'rgba(0, 0, 0, 0.09)');
      root.style.setProperty('--card-bg-accent', 'rgba(15, 23, 42, 0.08)');
      root.style.setProperty('--card-bg-accent-hover', 'rgba(15, 23, 42, 0.12)');
      root.style.setProperty('--card-bg-purple', 'rgba(124, 58, 237, 0.12)');
      root.style.setProperty('--card-bg-purple-hover', 'rgba(124, 58, 237, 0.18)');
    }


    root.style.setProperty('--modal-bg', colors.background.elevated);
    root.style.setProperty('--modal-border', colors.border.base);
    root.style.setProperty('--modal-overlay', theme.type === 'dark' ? 'rgba(0, 0, 0, 0.7)' : 'rgba(0, 0, 0, 0.5)');


    root.style.setProperty('--nav-bg', colors.background.secondary);
    root.style.setProperty('--nav-item-bg-hover', colors.element.base);
    root.style.setProperty('--nav-item-bg-active', colors.element.medium);
    root.style.setProperty('--nav-item-text', colors.text.secondary);
    root.style.setProperty('--nav-item-text-active', colors.text.primary);


    root.style.setProperty('--panel-bg', colors.background.primary);
    root.style.setProperty('--panel-header-bg', colors.background.secondary);
    root.style.setProperty('--panel-border', colors.border.base);


    root.style.setProperty('--tooltip-bg', colors.background.elevated);
    root.style.setProperty('--tooltip-border', colors.border.medium);
    root.style.setProperty('--tooltip-text', colors.text.primary);


    root.style.setProperty('--tool-card-bg-primary', colors.element.base);
    root.style.setProperty('--tool-card-bg-secondary', colors.element.soft);
    root.style.setProperty('--tool-card-bg-hover', colors.element.medium);
    root.style.setProperty('--tool-card-bg-elevated', colors.element.elevated);
    root.style.setProperty('--tool-card-border', colors.border.base);
    root.style.setProperty('--tool-card-border-subtle', colors.border.subtle);
    root.style.setProperty('--tool-card-text-primary', colors.text.primary);
    root.style.setProperty('--tool-card-text-secondary', colors.text.secondary);
    root.style.setProperty('--tool-card-text-muted', colors.text.muted);


    root.setAttribute('data-theme', theme.id);
    root.setAttribute('data-theme-type', theme.type);

    const bgPrimary = colors.background.primary;
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
      theme,
      metadata,
      exportedAt: new Date().toISOString(),
    };
  }




  validateTheme(theme: ThemeConfig): ThemeValidationResult {
    const errors: ThemeValidationResult['errors'] = [];
    const warnings: ThemeValidationResult['warnings'] = [];


    if (!theme.id) {
      errors.push({ path: 'id', message: 'Missing theme id', code: 'MISSING_ID' });
    }
    if (!theme.name) {
      errors.push({ path: 'name', message: 'Missing theme name', code: 'MISSING_NAME' });
    }
    if (!theme.type || !['dark', 'light'].includes(theme.type)) {
      errors.push({ path: 'type', message: 'Invalid theme type', code: 'INVALID_TYPE' });
    }


    if (!theme.colors) {
      errors.push({ path: 'colors', message: 'Missing color configuration', code: 'MISSING_COLORS' });
    }

    return {
      valid: errors.length === 0,
      errors,
      warnings,
    };
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

