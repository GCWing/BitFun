
import { configAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import {
  FontPreference,
  FontPreferenceEvent,
  FontPreferenceEventListener,
  FontPreferenceEventType,
  FlowChatFontMode,
  FontSizeLevel,
  FontSizeTokens,
  UiFontSizePreference,
  DEFAULT_FONT_PREFERENCE,
  resolveFontSizeTokens,
  resolveFlowChatFontSizeTokens,
} from '../types';

const log = createLogger('FontPreferenceService');

const CONFIG_KEY = 'font';

export class FontPreferenceService {
  private preference: FontPreference = { ...DEFAULT_FONT_PREFERENCE };
  private listeners: Map<FontPreferenceEventType, Set<FontPreferenceEventListener>> = new Map();
  /** Only register theme hook once (initialize may run from main + settings). */
  private themeSyncRegistered = false;

  // ---- Lifecycle ----

  async initialize(): Promise<void> {
    try {
      const saved = await configAPI.getConfig(CONFIG_KEY, { skipRetryOnNotFound: true }) as FontPreference | undefined;
      if (saved) {
        this.preference = this.mergeWithDefaults(saved);
      }
    } catch {
      // Config not found — use defaults
    }
    this.applyPreference(this.preference);

    if (!this.themeSyncRegistered) {
      this.themeSyncRegistered = true;
      const { themeService } = await import('@/infrastructure/theme');
      themeService.on('theme:after-change', () => {
        this.applyPreference(this.preference);
      });
    }

    log.info('Font preference initialized', {
      level: this.preference.uiSize.level,
      flowChat: this.preference.flowChat.mode,
    });
  }

  // ---- Read ----

  getPreference(): FontPreference {
    return { ...this.preference };
  }

  getDefaultPreference(): FontPreference {
    return { ...DEFAULT_FONT_PREFERENCE };
  }

  // ---- Write ----

  async setPreference(partial: Partial<FontPreference>): Promise<void> {
    const previous = { ...this.preference };
    const merged = this.mergeWithDefaults({ ...this.preference, ...partial });

    this.emit({ type: 'font:before-change', preference: merged, previousPreference: previous, timestamp: Date.now() });

    this.preference = merged;
    this.applyPreference(merged);

    this.emit({ type: 'font:after-change', preference: merged, previousPreference: previous, timestamp: Date.now() });

    try {
      await configAPI.setConfig(CONFIG_KEY, merged);
    } catch (error) {
      log.error('Failed to persist font preference', error);
    }
  }

  async setUiSize(level: FontSizeLevel, customPx?: number): Promise<void> {
    const uiSize: UiFontSizePreference = level === 'custom'
      ? { level, customPx: Math.max(12, Math.min(20, customPx ?? 14)) }
      : { level };
    await this.setPreference({ uiSize });
  }

  async setFlowChatFont(mode: FlowChatFontMode, basePx?: number): Promise<void> {
    if (mode === 'independent') {
      await this.setPreference({
        flowChat: { mode, basePx: Math.max(12, Math.min(20, Math.round(basePx ?? 14))) },
      });
      return;
    }
    await this.setPreference({ flowChat: { mode } });
  }

  async reset(): Promise<void> {
    await this.setPreference(DEFAULT_FONT_PREFERENCE);
  }

  // ---- CSS Application ----

  applyPreference(pref: FontPreference): void {
    const root = document.documentElement;
    const tokens = resolveFontSizeTokens(pref.uiSize);

    // Apply all UI font-size tokens — overrides tokens.scss :root defaults
    (Object.entries(tokens) as [string, string][]).forEach(([key, value]) => {
      root.style.setProperty(`--font-size-${key}`, value);
    });

    this.applyExtraFontSizeTokens(root, tokens);

    const flowTokens = resolveFlowChatFontSizeTokens(pref);
    (Object.entries(flowTokens) as [string, string][]).forEach(([key, value]) => {
      root.style.setProperty(`--flowchat-font-size-${key}`, value);
    });
    this.applyFlowChatExtraFontSizeTokens(root, flowTokens);

    // Drive body font-size so elements using `inherit` cascade to the new base size.
    // This is the broadest single-point fix for SCSS components that compiled their
    // font-size to literal px at build time (e.g. font-size: 14px).
    document.body.style.fontSize = tokens.base;

    log.debug('Font preference applied', { level: pref.uiSize.level });
  }

  // ---- Events ----

  on(type: FontPreferenceEventType, listener: FontPreferenceEventListener): () => void {
    if (!this.listeners.has(type)) {
      this.listeners.set(type, new Set());
    }
    this.listeners.get(type)!.add(listener);
    return () => {
      this.listeners.get(type)?.delete(listener);
    };
  }

  off(type: FontPreferenceEventType, listener: FontPreferenceEventListener): void {
    this.listeners.get(type)?.delete(listener);
  }

  /** Smaller steps used by some SCSS (xxs / 2xs); badge uses xs scale. */
  private applyExtraFontSizeTokens(root: HTMLElement, tokens: FontSizeTokens): void {
    const xsPx = parseInt(tokens.xs, 10);
    if (!Number.isNaN(xsPx)) {
      const twoXs = Math.max(8, xsPx - 1);
      const xxs = Math.max(7, xsPx - 2);
      root.style.setProperty('--font-size-2xs', `${twoXs}px`);
      root.style.setProperty('--font-size-xxs', `${xxs}px`);
    }
    root.style.setProperty('--badge-font-size', tokens.xs);
  }

  private applyFlowChatExtraFontSizeTokens(root: HTMLElement, tokens: FontSizeTokens): void {
    const xsPx = parseInt(tokens.xs, 10);
    if (!Number.isNaN(xsPx)) {
      const twoXs = Math.max(8, xsPx - 1);
      const xxs = Math.max(7, xsPx - 2);
      root.style.setProperty('--flowchat-font-size-2xs', `${twoXs}px`);
      root.style.setProperty('--flowchat-font-size-xxs', `${xxs}px`);
    }
  }

  private emit(event: FontPreferenceEvent): void {
    const listeners = this.listeners.get(event.type);
    if (!listeners) return;
    listeners.forEach(listener => {
      try {
        void listener(event);
      } catch (error) {
        log.error('Font preference event listener error', { type: event.type, error });
      }
    });
  }

  // ---- Helpers ----

  private mergeWithDefaults(raw: Partial<FontPreference>): FontPreference {
    const def = DEFAULT_FONT_PREFERENCE;
    return {
      uiSize: {
        level: raw.uiSize?.level ?? def.uiSize.level,
        customPx: raw.uiSize?.customPx,
      },
      flowChat: this.mergeFlowChatPreference(raw.flowChat),
    };
  }

  private mergeFlowChatPreference(
    raw: Partial<FontPreference['flowChat']> | undefined,
  ): FontPreference['flowChat'] {
    const def = DEFAULT_FONT_PREFERENCE.flowChat;
    if (!raw || raw.mode === undefined) {
      return { ...def };
    }
    if (raw.mode === 'sync' || raw.mode === 'lift') {
      return { mode: raw.mode };
    }
    if (raw.mode === 'independent') {
      const basePx = typeof raw.basePx === 'number'
        ? Math.max(12, Math.min(20, Math.round(raw.basePx)))
        : 14;
      return { mode: 'independent', basePx };
    }
    return { ...def };
  }
}

export const fontPreferenceService = new FontPreferenceService();
