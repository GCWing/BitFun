import { describe, expect, it } from 'vitest';

import type { ThemeConfig, ThemeType } from '@/infrastructure/theme/types';
import { buildMiniAppThemeVars } from './buildMiniAppThemeVars';

function createTheme(type: ThemeType, scrollbar?: { thumb: string; thumbHover: string }): ThemeConfig {
  return {
    id: `${type}-fixture`,
    name: `${type} fixture`,
    type,
    colors: {
      background: {
        primary: '#111111',
        secondary: '#222222',
        tertiary: '#333333',
        elevated: '#444444',
      },
      text: {
        primary: '#eeeeee',
        secondary: '#dddddd',
        muted: '#cccccc',
      },
      accent: {
        500: '#60a5fa',
        600: '#3b82f6',
      },
      semantic: {
        success: '#22c55e',
        warning: '#f59e0b',
        error: '#ef4444',
        info: '#38bdf8',
      },
      border: {
        base: '#555555',
        subtle: '#666666',
      },
      element: {
        base: '#777777',
        medium: '#888888',
      },
      scrollbar,
    },
  } as unknown as ThemeConfig;
}

describe('buildMiniAppThemeVars', () => {
  it('keeps dark and light scrollbar fallbacks output-equivalent', () => {
    expect(buildMiniAppThemeVars(createTheme('dark'))?.vars).toMatchObject({
      '--bitfun-scrollbar-thumb': 'rgba(255, 255, 255, 0.12)',
      '--bitfun-scrollbar-thumb-hover': 'rgba(255, 255, 255, 0.24)',
    });

    expect(buildMiniAppThemeVars(createTheme('light'))?.vars).toMatchObject({
      '--bitfun-scrollbar-thumb': 'rgba(0, 0, 0, 0.15)',
      '--bitfun-scrollbar-thumb-hover': 'rgba(0, 0, 0, 0.3)',
    });
  });

  it('preserves theme-provided scrollbar values over fallback values', () => {
    expect(
      buildMiniAppThemeVars(createTheme('dark', {
        thumb: 'theme-thumb',
        thumbHover: 'theme-thumb-hover',
      }))?.vars,
    ).toMatchObject({
      '--bitfun-scrollbar-thumb': 'theme-thumb',
      '--bitfun-scrollbar-thumb-hover': 'theme-thumb-hover',
    });
  });
});
