import type {
  MonacoAppearanceSettings,
  MonacoAppearanceTokenRule,
} from '../types';

interface RgbaColor {
  red: number;
  green: number;
  blue: number;
  alpha: number;
}

function parseHexColor(value: string, allowBareHex: boolean): RgbaColor | null {
  const match = (allowBareHex
    ? /^#?([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i
    : /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i
  ).exec(value);
  if (!match) return null;

  const raw = match[1];
  const expanded = raw.length <= 4
    ? raw.split('').map(channel => channel + channel).join('')
    : raw;
  return {
    red: Number.parseInt(expanded.slice(0, 2), 16),
    green: Number.parseInt(expanded.slice(2, 4), 16),
    blue: Number.parseInt(expanded.slice(4, 6), 16),
    alpha: expanded.length === 8
      ? Number.parseInt(expanded.slice(6, 8), 16) / 255
      : 1,
  };
}

function parseRgbColor(value: string): RgbaColor | null {
  const match = /^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)(?:\s*,\s*([\d.]+))?\s*\)$/i.exec(value);
  if (!match) return null;

  const channels = match.slice(1, 4).map(Number);
  const alpha = match[4] === undefined ? 1 : Number(match[4]);
  if (channels.some(channel => !Number.isFinite(channel) || channel < 0 || channel > 255)
    || !Number.isFinite(alpha)
    || alpha < 0
    || alpha > 1) {
    return null;
  }

  return {
    red: channels[0],
    green: channels[1],
    blue: channels[2],
    alpha,
  };
}

function parseColor(value: string, allowBareHex: boolean): RgbaColor | null {
  return parseHexColor(value, allowBareHex) ?? parseRgbColor(value);
}

function hexChannel(value: number): string {
  return Math.round(value).toString(16).padStart(2, '0');
}

function formatHex(color: RgbaColor, preserveAlpha: boolean): string {
  const rgb = `${hexChannel(color.red)}${hexChannel(color.green)}${hexChannel(color.blue)}`;
  return preserveAlpha && color.alpha < 1
    ? `#${rgb}${hexChannel(color.alpha * 255)}`
    : `#${rgb}`;
}

function compositeOver(foreground: RgbaColor, background: RgbaColor): RgbaColor {
  const alpha = foreground.alpha + background.alpha * (1 - foreground.alpha);
  if (alpha === 0) return { red: 0, green: 0, blue: 0, alpha: 0 };

  const compositeChannel = (foregroundChannel: number, backgroundChannel: number): number => (
    foregroundChannel * foreground.alpha
    + backgroundChannel * background.alpha * (1 - foreground.alpha)
  ) / alpha;
  return {
    red: compositeChannel(foreground.red, background.red),
    green: compositeChannel(foreground.green, background.green),
    blue: compositeChannel(foreground.blue, background.blue),
    alpha,
  };
}

function parseRequiredColor(value: string, path: string, allowBareHex: boolean): RgbaColor {
  const parsed = parseColor(value, allowBareHex);
  if (!parsed) throw new Error(`${path} is not a supported color`);
  return parsed;
}

function projectOpaqueColor(
  color: RgbaColor,
  path: string,
  editorBackground: RgbaColor | null,
): string {
  if (color.alpha === 1) return formatHex(color, false);
  if (!editorBackground) {
    throw new Error(`${path} uses alpha but colors.editor.background is not an opaque color`);
  }
  return formatHex(compositeOver(color, editorBackground), false);
}

function projectTokenRule(
  rule: MonacoAppearanceTokenRule,
  index: number,
  editorBackground: RgbaColor | null,
): MonacoAppearanceTokenRule {
  const projectTokenColor = (value: string, field: 'foreground' | 'background'): string => {
    const parsed = parseRequiredColor(value, `rules.${index}.${field}`, true);
    return projectOpaqueColor(parsed, `rules.${index}.${field}`, editorBackground);
  };

  return {
    ...rule,
    ...(rule.foreground === undefined
      ? {}
      : { foreground: projectTokenColor(rule.foreground, 'foreground') }),
    ...(rule.background === undefined
      ? {}
      : { background: projectTokenColor(rule.background, 'background') }),
  };
}

export function isMonacoAppearanceColor(value: string, allowBareHex = false): boolean {
  return parseColor(value, allowBareHex) !== null;
}

export function validateMonacoAppearanceColorSemantics(
  settings: Readonly<MonacoAppearanceSettings>,
): string[] {
  const errors: string[] = [];
  const backgroundValue = settings.colors['editor.background'];
  const background = backgroundValue === undefined ? null : parseColor(backgroundValue, false);
  const opaqueBackground = background?.alpha === 1 ? background : null;
  if (background && background.alpha < 1) {
    errors.push('colors.editor.background must be opaque because Monaco also uses it as a token color');
  }

  const requireBackgroundForAlpha = (
    value: string | undefined,
    path: string,
    allowBareHex: boolean,
  ): void => {
    if (value === undefined) return;
    const parsed = parseColor(value, allowBareHex);
    if (parsed && parsed.alpha < 1 && !opaqueBackground) {
      errors.push(`${path} uses alpha and requires an opaque colors.editor.background`);
    }
  };
  requireBackgroundForAlpha(
    settings.colors['editor.foreground'],
    'colors.editor.foreground',
    false,
  );
  settings.rules.forEach((rule, index) => {
    requireBackgroundForAlpha(rule.foreground, `rules.${index}.foreground`, true);
    requireBackgroundForAlpha(rule.background, `rules.${index}.background`, true);
  });
  return errors;
}

/**
 * Converts OpenBitFun Appearance colors to the narrower formats accepted by Monaco.
 * Monaco workbench colors accept hex alpha, while token colors must be opaque.
 */
export function projectMonacoAppearanceSettings(
  settings: Readonly<MonacoAppearanceSettings>,
): MonacoAppearanceSettings {
  const configuredBackground = settings.colors['editor.background'];
  const editorBackground = configuredBackground === undefined
    ? null
    : parseRequiredColor(configuredBackground, 'colors.editor.background', false);
  if (editorBackground && editorBackground.alpha < 1) {
    throw new Error('colors.editor.background must be opaque because Monaco also uses it as a token color');
  }

  const colors = Object.fromEntries(Object.entries(settings.colors).map(([key, value]) => {
    const parsed = parseRequiredColor(value, `colors.${key}`, false);
    return [key, formatHex(parsed, true)];
  }));
  const configuredForeground = settings.colors['editor.foreground'];
  if (configuredForeground !== undefined) {
    const foreground = parseRequiredColor(configuredForeground, 'colors.editor.foreground', false);
    colors['editor.foreground'] = projectOpaqueColor(
      foreground,
      'colors.editor.foreground',
      editorBackground,
    );
  }

  return {
    ...settings,
    rules: settings.rules.map((rule, index) => projectTokenRule(rule, index, editorBackground)),
    colors,
  };
}
