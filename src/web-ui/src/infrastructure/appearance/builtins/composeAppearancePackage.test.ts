import { describe, expect, it } from 'vitest';
import type { AppearancePackage } from '../types';
import { APPEARANCE_THEME_TOKEN_NAMES } from './catalog';
import { composeAppearancePackage } from './composeAppearancePackage';

describe('composeAppearancePackage', () => {
  it('resolves a partial imported package into a complete host appearance', () => {
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'example.partial',
      name: 'Partial',
      version: '1.0.0',
      mode: 'dark',
      globals: {
        colors: {
          accent: { kind: 'hex', value: '#ff3366' },
        },
      },
      components: {
        button: {
          parts: {
            root: {
              cascade: 'override',
              base: { borderRadius: { kind: 'px', value: 2 } },
            },
          },
        },
      },
    };

    const resolved = composeAppearancePackage(pkg);

    expect(resolved.id).toBe(pkg.id);
    expect(resolved.globals?.colors?.accent).toEqual({ kind: 'hex', value: '#ff3366' });
    expect(resolved.globals?.colors?.['bg-primary']).toBeDefined();
    expect(resolved.components?.button?.parts.root.base).toMatchObject({
      borderRadius: { kind: 'px', value: 2 },
    });
    expect(resolved.components?.button?.parts.root.cascade).toBe('override');
    expect(resolved.renderers?.monaco).toBeDefined();
    expect(resolved.renderers?.xterm).toBeDefined();
    expect(resolved.renderers?.mermaid).toBeDefined();
    expect(resolved.renderers?.['generative-widget']).toBeDefined();
    expect(resolved.renderers?.['bitfun-canvas']).toBeDefined();
    expect(Object.keys(resolved.renderers?.['theme-tokens']?.settings.tokens ?? {})).toEqual(
      expect.arrayContaining(APPEARANCE_THEME_TOKEN_NAMES),
    );
  });
});
