import type { ITheme } from '@xterm/xterm';
import type {
  AppearanceRendererAdapter,
  AppearanceRendererContext,
  XtermAppearanceSettings,
  XtermAppearanceSurface,
} from '../types';

type XtermAppearanceListener = () => void;

const COLOR_KEYS: ReadonlySet<keyof ITheme> = new Set([
  'background', 'foreground', 'cursor', 'cursorAccent', 'selectionBackground',
  'selectionForeground', 'selectionInactiveBackground', 'black', 'red', 'green',
  'yellow', 'blue', 'magenta', 'cyan', 'white', 'brightBlack', 'brightRed',
  'brightGreen', 'brightYellow', 'brightBlue', 'brightMagenta', 'brightCyan',
  'brightWhite',
]);
const COLOR_PATTERN = /^(?:transparent|#[0-9a-f]{3,8}|rgba?\(\s*\d+(?:\.\d+)?\s*,\s*\d+(?:\.\d+)?\s*,\s*\d+(?:\.\d+)?(?:\s*,\s*(?:0|1|0?\.\d+))?\s*\))$/i;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function validateColors(value: unknown, path: string): string[] {
  if (!isRecord(value)) return [`${path} must be an object`];
  const errors: string[] = [];
  Object.entries(value).forEach(([key, color]) => {
    if (!COLOR_KEYS.has(key as keyof ITheme)) errors.push(`${path}.${key} is not supported`);
    if (typeof color !== 'string' || !COLOR_PATTERN.test(color)) errors.push(`${path}.${key} is not a supported color`);
  });
  for (const key of ['background', 'foreground', 'cursor'] as const) {
    if (typeof value[key] !== 'string') errors.push(`${path}.${key} is required`);
  }
  return errors;
}

export class XtermAppearanceAdapter implements AppearanceRendererAdapter<'xterm'> {
  readonly id = 'xterm';
  private settings: XtermAppearanceSettings | null = null;
  private listeners = new Set<XtermAppearanceListener>();

  validate(settings: unknown): string[] {
    const errors: string[] = [];
    if (!isRecord(settings)) return ['Settings must be an object'];
    Object.keys(settings).forEach(key => {
      if (key !== 'surfaces') errors.push(`Unknown setting: ${key}`);
    });
    if (!isRecord(settings.surfaces)) {
      errors.push('surfaces must be an object');
    } else {
      Object.keys(settings.surfaces).forEach(key => {
        if (!['terminal', 'output'].includes(key)) errors.push(`Unknown xterm surface: ${key}`);
      });
      errors.push(...validateColors(settings.surfaces.terminal, 'surfaces.terminal'));
      errors.push(...validateColors(settings.surfaces.output, 'surfaces.output'));
    }
    return errors;
  }

  apply(
    next: Readonly<XtermAppearanceSettings> | undefined,
    _previous: Readonly<XtermAppearanceSettings> | undefined,
    _context: AppearanceRendererContext,
  ): void {
    if (next) {
      const errors = this.validate(next);
      if (errors.length > 0) throw new Error(`Invalid xterm appearance: ${errors.join('; ')}`);
      this.settings = next as XtermAppearanceSettings;
    } else this.settings = null;
    this.listeners.forEach(listener => listener());
  }

  getColors(surface: XtermAppearanceSurface): ITheme {
    return { ...this.requireSettings().surfaces[surface] };
  }

  subscribe(listener: XtermAppearanceListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private requireSettings(): XtermAppearanceSettings {
    if (!this.settings) throw new Error('xterm appearance has not been applied');
    return this.settings;
  }
}

export const xtermAppearanceAdapter = new XtermAppearanceAdapter();
