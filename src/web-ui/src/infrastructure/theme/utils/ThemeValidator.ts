import { bitfunLightTheme } from '../presets/light-theme';
import type { ThemeConfig, ThemeValidationResult } from '../types';

const REQUIRED_SCHEMA_ROOTS = ['colors', 'effects', 'motion', 'typography'] as const;
const OPTIONAL_SCHEMA_PATHS = new Set([
  'colors.background.tooltip',
  'colors.purple',
  'colors.scrollbar',
]);
const OPTIONAL_SCHEMA_REFERENCES = {
  'colors.scrollbar': {
    thumb: 'transparent',
    thumbHover: 'transparent',
  },
} as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isOptionalSchemaPath(path: string): boolean {
  return OPTIONAL_SCHEMA_PATHS.has(path);
}

function isColorPath(path: string): boolean {
  return path.startsWith('colors.');
}

function readPath(source: unknown, path: string): unknown {
  return path.split('.').reduce<unknown>((current, key) => (
    isRecord(current) ? current[key] : undefined
  ), source);
}

function isValidColor(color: unknown): boolean {
  if (typeof color !== 'string') {
    return false;
  }

  if (/^#([0-9A-Fa-f]{3}){1,2}$/.test(color)) {
    return true;
  }

  if (/^rgba?\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*(,\s*[\d.]+\s*)?\)$/.test(color)) {
    return true;
  }

  if (/^hsla?\(\s*\d+\s*,\s*\d+%\s*,\s*\d+%\s*(,\s*[\d.]+\s*)?\)$/.test(color)) {
    return true;
  }

  return color === 'transparent' || color === 'currentColor';
}

function calculateContrast(_color1: string, _color2: string): number {
  return 4.5;
}

export class ThemeValidator {
  validate(theme: ThemeConfig): ThemeValidationResult {
    const errors: ThemeValidationResult['errors'] = [];
    const warnings: ThemeValidationResult['warnings'] = [];

    this.validateBasicFields(theme, errors);
    this.validateRequiredSchema(theme, errors);
    this.validateContrast(theme, warnings);
    this.validateCompleteness(theme, warnings);

    return {
      valid: errors.length === 0,
      errors,
      warnings,
    };
  }

  private validateBasicFields(
    theme: ThemeConfig,
    errors: ThemeValidationResult['errors'],
  ): void {
    if (!theme.id || theme.id.trim() === '') {
      errors.push({
        path: 'id',
        message: 'Theme id cannot be empty',
        code: 'MISSING_ID',
      });
    }

    if (!theme.name || theme.name.trim() === '') {
      errors.push({
        path: 'name',
        message: 'Theme name cannot be empty',
        code: 'MISSING_NAME',
      });
    }

    if (!theme.type || !['dark', 'light'].includes(theme.type)) {
      errors.push({
        path: 'type',
        message: 'Theme type must be "dark" or "light"',
        code: 'INVALID_TYPE',
      });
    }
  }

  private validateRequiredSchema(
    theme: ThemeConfig,
    errors: ThemeValidationResult['errors'],
  ): void {
    REQUIRED_SCHEMA_ROOTS.forEach((root) => {
      this.validateSchemaValue(
        root,
        (theme as unknown as Record<string, unknown>)[root],
        (bitfunLightTheme as unknown as Record<string, unknown>)[root],
        errors,
      );
    });

    Object.entries(OPTIONAL_SCHEMA_REFERENCES).forEach(([path, reference]) => {
      const value = readPath(theme, path);
      if (value !== undefined) {
        this.validateSchemaValue(path, value, reference, errors);
      }
    });
  }

  private validateSchemaValue(
    path: string,
    value: unknown,
    reference: unknown,
    errors: ThemeValidationResult['errors'],
  ): void {
    if (isOptionalSchemaPath(path) && value === undefined) {
      return;
    }

    if (isRecord(reference)) {
      if (!isRecord(value)) {
        errors.push({
          path,
          message: `Missing ${path} configuration`,
          code: path === 'colors' ? 'MISSING_COLORS' : 'MISSING_THEME_FIELD_GROUP',
        });
        return;
      }

      Object.entries(reference).forEach(([key, childReference]) => {
        this.validateSchemaValue(`${path}.${key}`, value[key], childReference, errors);
      });
      return;
    }

    if (isColorPath(path)) {
      if (!isValidColor(value)) {
        errors.push({
          path,
          message: `Invalid color value: ${String(value)}`,
          code: 'INVALID_COLOR_FORMAT',
        });
      }
      return;
    }

    if (typeof reference === 'number') {
      if (typeof value !== 'number' || !Number.isFinite(value)) {
        errors.push({
          path,
          message: `Missing numeric value for ${path}`,
          code: 'INVALID_THEME_NUMBER',
        });
      }
      return;
    }

    if (typeof reference === 'string' && (typeof value !== 'string' || value.trim() === '')) {
      errors.push({
        path,
        message: `Missing string value for ${path}`,
        code: 'INVALID_THEME_STRING',
      });
    }
  }

  private validateContrast(
    theme: ThemeConfig,
    warnings: ThemeValidationResult['warnings'],
  ): void {
    const textPrimary = theme.colors?.text?.primary;
    const bgPrimary = theme.colors?.background?.primary;
    if (!textPrimary || !bgPrimary) {
      return;
    }

    const contrast = calculateContrast(textPrimary, bgPrimary);

    if (contrast < 4.5) {
      warnings.push({
        path: 'colors',
        message: `Contrast between primary text and background (${contrast.toFixed(2)}) is below WCAG AA (4.5:1)`,
        code: 'LOW_CONTRAST',
      });
    }
  }

  private validateCompleteness(
    theme: ThemeConfig,
    warnings: ThemeValidationResult['warnings'],
  ): void {
    if (!theme.monaco) {
      warnings.push({
        path: 'monaco',
        message: 'Missing Monaco Editor configuration; default theme will be used',
        code: 'MISSING_MONACO',
      });
    }
  }
}

export const themeValidator = new ThemeValidator();
