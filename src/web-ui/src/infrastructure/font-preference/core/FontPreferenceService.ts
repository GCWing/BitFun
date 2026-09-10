
import { configAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import {
  FontPreference,
  FontPreferenceEvent,
  FontPreferenceEventListener,
  FontPreferenceEventType,
  FontSizeLevel,
  UiFontSizePreference,
  DEFAULT_FONT_PREFERENCE,
  resolveFontSizeTokens,
} from '../types';

const log = createLogger('FontPreferenceService');

const CONFIG_KEY = 'font';

export class FontPreferenceService {
  private preference: FontPreference = { ...DEFAULT_FONT_PREFERENCE };
  private listeners: Map<FontPreferenceEventType, Set<FontPreferenceEventListener>> = new Map();
  private changeVersion = 0;

  // ---- Lifecycle ----

  async initialize(): Promise<void> {
    try {
      await this.reloadFromConfig();
    } catch (error) {
      log.warn('Failed to load font preference', error);
    }
    this.applyPreference(this.preference);

    log.info('Font preference initialized', {
      level: this.preference.uiSize.level,
    });
  }

  /** Apply externally persisted preferences without writing them back to the host. */
  async reloadFromConfig(): Promise<void> {
    const version = ++this.changeVersion;
    const saved = await configAPI.getConfig(CONFIG_KEY, { skipRetryOnNotFound: true }) as FontPreference | undefined;
    if (version !== this.changeVersion) return;
    const previous = this.preference;
    const preference = this.mergeWithDefaults(saved ?? DEFAULT_FONT_PREFERENCE);
    this.preference = preference;
    this.applyPreference(preference);
    if (previous.uiSize.level !== preference.uiSize.level
      || previous.uiSize.customPx !== preference.uiSize.customPx) {
      this.emit({ type: 'font:after-change', preference, previousPreference: previous, timestamp: Date.now() });
    }
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
    this.changeVersion += 1;
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

  async reset(): Promise<void> {
    await this.setPreference(DEFAULT_FONT_PREFERENCE);
  }

  async adjustUiSize(delta: -1 | 1): Promise<void> {
    const currentPx = parseFloat(resolveFontSizeTokens(this.preference.uiSize).base);
    const nextPx = Math.max(12, Math.min(20, currentPx + delta));
    if (nextPx !== currentPx) {
      await this.setUiSize('custom', nextPx);
    }
  }

  // ---- CSS Application ----

  applyPreference(pref: FontPreference): void {
    const root = document.documentElement;
    const tokens = resolveFontSizeTokens(pref.uiSize);

    // Runtime preferences override only the canonical foundation. Semantic
    // typography roles keep following it through generated CSS references.
    (Object.entries(tokens) as [string, string][]).forEach(([key, value]) => {
      root.style.setProperty(`--openbitfun-font-size-${key}`, value);
    });

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
    };
  }
}

export const fontPreferenceService = new FontPreferenceService();
