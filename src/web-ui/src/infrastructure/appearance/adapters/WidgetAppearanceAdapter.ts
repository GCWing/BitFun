import type { AppearanceRendererAdapter, AppearanceRendererContext, WidgetAppearanceSettings } from '../types';
import { WIDGET_APPEARANCE_VARIABLE_NAMES } from './widgetAppearanceVariables';

const SAFE_VALUE_PATTERN = /^(?!.*(?:https?:\/\/|javascript:|data:|url\s*\(|[{};@\\]))[\w\s#.,%()'"+\-/*]+$/i;
const ALLOWED_VARIABLE_NAMES: ReadonlySet<string> = new Set(WIDGET_APPEARANCE_VARIABLE_NAMES);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export class WidgetAppearanceAdapter implements AppearanceRendererAdapter<'generative-widget'> {
  readonly id = 'generative-widget';
  private settings: WidgetAppearanceSettings | null = null;
  private listeners = new Set<() => void>();

  validate(settings: unknown): string[] {
    const errors: string[] = [];
    if (!isRecord(settings)) return ['Settings must be an object'];
    Object.keys(settings).forEach(key => {
      if (!['id', 'mode', 'vars'].includes(key)) errors.push(`Unknown setting: ${key}`);
    });
    if (typeof settings.id !== 'string' || !/^[a-z][a-z0-9.-]{0,95}$/.test(settings.id)) errors.push('id is invalid');
    if (!['light', 'dark'].includes(String(settings.mode))) errors.push('mode must be light or dark');
    if (!isRecord(settings.vars) || Object.keys(settings.vars).length > 256) {
      errors.push('vars must be an object with at most 256 entries');
    } else {
      Object.entries(settings.vars).forEach(([key, value]) => {
        if (!ALLOWED_VARIABLE_NAMES.has(key)) errors.push(`vars.${key} is not registered by the widget host contract`);
        if (typeof value !== 'string' || value.length > 256 || !SAFE_VALUE_PATTERN.test(value)) {
          errors.push(`vars.${key} has an unsafe value`);
        }
      });
    }
    return errors;
  }

  apply(next: Readonly<WidgetAppearanceSettings> | undefined, _previous: Readonly<WidgetAppearanceSettings> | undefined, _context: AppearanceRendererContext): void {
    if (next) {
      const errors = this.validate(next);
      if (errors.length > 0) throw new Error(`Invalid widget appearance: ${errors.join('; ')}`);
      this.settings = next as WidgetAppearanceSettings;
    } else this.settings = null;
    this.listeners.forEach(listener => listener());
  }

  getSettings(): WidgetAppearanceSettings {
    if (!this.settings) throw new Error('Widget appearance has not been applied');
    return this.settings;
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}

export const widgetAppearanceAdapter = new WidgetAppearanceAdapter();
