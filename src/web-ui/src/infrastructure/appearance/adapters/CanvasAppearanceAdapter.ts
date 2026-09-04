import type { AppearanceRendererAdapter, AppearanceRendererContext, CanvasAppearanceSettings } from '../types';

const COLOR_PATTERN = /^(?:transparent|#[0-9a-f]{3,8}|rgba?\(\s*\d+(?:\.\d+)?\s*,\s*\d+(?:\.\d+)?\s*,\s*\d+(?:\.\d+)?(?:\s*,\s*(?:0|1|0?\.\d+))?\s*\)|hsla?\([^)]+\))$/i;
const COLOR_KEYS = ['bg', 'panel', 'fg', 'muted', 'border', 'accent', 'success', 'warning', 'danger', 'info'] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export class CanvasAppearanceAdapter implements AppearanceRendererAdapter<'openbitfun-canvas'> {
  readonly id = 'openbitfun-canvas';
  private settings: CanvasAppearanceSettings | null = null;
  private listeners = new Set<() => void>();

  validate(settings: unknown): string[] {
    const errors: string[] = [];
    if (!isRecord(settings)) return ['Settings must be an object'];
    const known = new Set(['id', 'mode', ...COLOR_KEYS]);
    Object.keys(settings).forEach(key => {
      if (!known.has(key)) errors.push(`Unknown setting: ${key}`);
    });
    if (typeof settings.id !== 'string' || !/^[a-z][a-z0-9.-]{0,95}$/.test(settings.id)) errors.push('id is invalid');
    if (!['light', 'dark'].includes(String(settings.mode))) errors.push('mode must be light or dark');
    COLOR_KEYS.forEach(key => {
      const value = settings[key];
      if (typeof value !== 'string' || !COLOR_PATTERN.test(value)) errors.push(`${key} is not a supported color`);
    });
    return errors;
  }

  apply(next: Readonly<CanvasAppearanceSettings> | undefined, _previous: Readonly<CanvasAppearanceSettings> | undefined, _context: AppearanceRendererContext): void {
    if (next) {
      const errors = this.validate(next);
      if (errors.length > 0) throw new Error(`Invalid Canvas appearance: ${errors.join('; ')}`);
      this.settings = next as CanvasAppearanceSettings;
    } else this.settings = null;
    this.listeners.forEach(listener => listener());
  }

  getSettings(): CanvasAppearanceSettings {
    if (!this.settings) throw new Error('Canvas appearance has not been applied');
    return this.settings;
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}

export const canvasAppearanceAdapter = new CanvasAppearanceAdapter();
