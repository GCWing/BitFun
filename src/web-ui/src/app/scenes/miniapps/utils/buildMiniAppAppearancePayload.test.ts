import { describe, expect, it } from 'vitest';

import type { AppearanceMode, ResolvedAppearance } from '@/infrastructure/appearance';
import { buildMiniAppAppearancePayload } from './buildMiniAppAppearancePayload';

function createAppearance(
  mode: AppearanceMode,
  scrollbar?: { thumb: string; thumbHover: string },
): ResolvedAppearance {
  return {
    revision: 1,
    id: `${mode}-fixture`,
    name: `${mode} fixture`,
    version: '1.0.0',
    mode,
    globals: {},
    materials: {},
    components: {},
    scenes: {},
    renderers: {
      'theme-tokens': {
        tokens: {
          '--bf-color-surface-canvas': '#111111',
          '--bf-color-content-primary': '#eeeeee',
          '--bf-color-scrollbar-thumb': scrollbar?.thumb
            ?? (mode === 'dark' ? 'rgba(255, 255, 255, 0.12)' : 'rgba(0, 0, 0, 0.15)'),
          '--bf-color-scrollbar-thumb-hover': scrollbar?.thumbHover
            ?? (mode === 'dark' ? 'rgba(255, 255, 255, 0.24)' : 'rgba(0, 0, 0, 0.3)'),
        },
      },
    },
    assets: {},
    diagnostics: [],
    cssText: '',
  };
}

describe('buildMiniAppAppearancePayload', () => {
  it('projects the active appearance tokens into the MiniApp contract', () => {
    expect(buildMiniAppAppearancePayload(createAppearance('dark'))).toMatchObject({
      mode: 'dark',
      id: 'dark-fixture',
      vars: {
        '--bitfun-bg': '#111111',
        '--bitfun-text': '#eeeeee',
        '--bitfun-scrollbar-thumb': 'rgba(255, 255, 255, 0.12)',
      },
    });
  });

  it('preserves appearance-provided scrollbar values', () => {
    expect(
      buildMiniAppAppearancePayload(createAppearance('light', {
        thumb: 'appearance-thumb',
        thumbHover: 'appearance-thumb-hover',
      }))?.vars,
    ).toMatchObject({
      '--bitfun-scrollbar-thumb': 'appearance-thumb',
      '--bitfun-scrollbar-thumb-hover': 'appearance-thumb-hover',
    });
  });
});
