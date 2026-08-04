import type { AppearanceRendererAdapter, AppearanceRendererContext, MermaidAppearanceSettings } from '../types';

const COLOR_PATTERN = /^(?:transparent|#[0-9a-f]{3,8}|rgba?\(\s*\d+(?:\.\d+)?\s*,\s*\d+(?:\.\d+)?\s*,\s*\d+(?:\.\d+)?(?:\s*,\s*(?:0|1|0?\.\d+))?\s*\))$/i;
const PALETTE_KEYS = [
  'nodeFill', 'nodeFillHover', 'nodeText', 'nodeStroke', 'nodeStrokeHover',
  'clusterFill', 'clusterText', 'clusterStroke', 'edgeStroke', 'edgeLabelBackground',
  'edgeLabelText', 'noteFill', 'noteText', 'noteStroke', 'activationFill',
  'activationStroke', 'success', 'warning', 'error', 'errorBackground', 'info',
  'highlight', 'pieColors',
] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export class MermaidAppearanceAdapter implements AppearanceRendererAdapter<'mermaid'> {
  readonly id = 'mermaid';
  private settings: MermaidAppearanceSettings | null = null;
  private revision = 0;
  private listeners = new Set<() => void>();

  validate(settings: unknown): string[] {
    const errors: string[] = [];
    if (!isRecord(settings)) return ['Settings must be an object'];
    Object.keys(settings).forEach(key => {
      if (!['mode', 'palette'].includes(key)) errors.push(`Unknown setting: ${key}`);
    });
    if (!['light', 'dark'].includes(String(settings.mode))) errors.push('mode must be light or dark');
    if (!isRecord(settings.palette)) {
      errors.push('palette must be an object');
      return errors;
    }
    Object.keys(settings.palette).forEach(key => {
      if (!PALETTE_KEYS.includes(key as typeof PALETTE_KEYS[number])) errors.push(`palette.${key} is not supported`);
    });
    const palette = settings.palette as Record<string, unknown>;
    PALETTE_KEYS.forEach(key => {
      const value = palette[key];
      if (key === 'pieColors') {
        if (!Array.isArray(value) || value.length !== 8 || value.some(item => typeof item !== 'string' || !COLOR_PATTERN.test(item))) {
          errors.push('palette.pieColors must contain exactly eight supported colors');
        }
      } else if (typeof value !== 'string' || !COLOR_PATTERN.test(value)) {
        errors.push(`palette.${key} is not a supported color`);
      }
    });
    return errors;
  }

  apply(
    next: Readonly<MermaidAppearanceSettings> | undefined,
    _previous: Readonly<MermaidAppearanceSettings> | undefined,
    context: AppearanceRendererContext,
  ): void {
    if (next) {
      const errors = this.validate(next);
      if (errors.length > 0) throw new Error(`Invalid Mermaid appearance: ${errors.join('; ')}`);
      this.settings = next as MermaidAppearanceSettings;
    } else this.settings = null;
    this.revision = context.revision;
    this.listeners.forEach(listener => listener());
  }

  getSettings(): MermaidAppearanceSettings {
    if (!this.settings) throw new Error('Mermaid appearance has not been applied');
    return this.settings;
  }

  getRevision(): number {
    return this.revision;
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}

export const mermaidAppearanceAdapter = new MermaidAppearanceAdapter();
