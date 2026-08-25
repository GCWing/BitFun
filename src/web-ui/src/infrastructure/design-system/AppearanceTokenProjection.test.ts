// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from 'vitest';
import { cssVariables as systemCssVariables } from '@bitfun/design-tokens';
import { themeCssVariables } from '@bitfun/theme-bitfun';
import { cssTokenAppearanceAdapter } from '@/infrastructure/appearance/adapters/CssTokenAppearanceAdapter';
import { builtinAppearancePackages } from '@/infrastructure/appearance/builtins/catalog';
import {
  DESIGN_SYSTEM_APPEARANCE_TOKEN_PROJECTION,
  projectDesignSystemAppearanceTokens,
} from './appearanceTokenProjection';

function getDarkSettings() {
  const settings = builtinAppearancePackages
    .find(pkg => pkg.id === 'bitfun-dark')
    ?.renderers?.['css-tokens']?.settings;
  if (!settings) throw new Error('Missing built-in dark Appearance CSS tokens');
  return settings;
}

describe('Appearance to design-system token projection', () => {
  beforeEach(() => {
    document.documentElement.removeAttribute('style');
    document.body.removeAttribute('style');
  });

  it('projects existing Appearance values onto canonical package variables', () => {
    const settings = getDarkSettings();
    const projected = projectDesignSystemAppearanceTokens(settings.tokens);

    expect(projected[themeCssVariables['color.surface.canvas']]).toBe(
      settings.tokens['--bf-appearance-token-color-bg-primary'],
    );
    expect(projected[themeCssVariables['color.action.primary.background']]).toBe(
      settings.tokens['--bf-appearance-token-btn-primary-bg'],
    );
    expect(projected[themeCssVariables['color.status.danger.content']]).toBe(
      settings.tokens['--bf-appearance-token-color-error'],
    );
    expect(projected[systemCssVariables['font.family.sans']]).toBe(
      settings.tokens['--bf-appearance-token-font-family-sans'],
    );
    expect(projected[systemCssVariables['radius.md']]).toBe(
      settings.tokens['--bf-appearance-token-size-radius-md'],
    );
  });

  it('keeps one canonical target per projection entry', () => {
    const targets = DESIGN_SYSTEM_APPEARANCE_TOKEN_PROJECTION.map(({ target }) => target);

    expect(new Set(targets).size).toBe(targets.length);
  });

  it('can source every projection from every built-in Appearance', () => {
    for (const appearance of builtinAppearancePackages) {
      const settings = appearance.renderers?.['css-tokens']?.settings;
      expect(settings, `${appearance.id} must expose CSS token settings`).toBeDefined();

      for (const { source } of DESIGN_SYSTEM_APPEARANCE_TOKEN_PROJECTION) {
        expect(
          settings?.tokens[source],
          `${appearance.id} must define projected source ${source}`,
        ).toBeDefined();
      }
    }
  });

  it('applies and removes canonical variables with the owning CSS renderer', async () => {
    const settings = getDarkSettings();
    const context = {
      appearanceId: 'bitfun-dark',
      assets: {},
      globals: {},
      mode: 'dark' as const,
      revision: 1,
    };

    await cssTokenAppearanceAdapter.apply(settings, undefined, context);
    expect(document.documentElement.style.getPropertyValue(
      themeCssVariables['color.surface.canvas'],
    )).toBe(settings.tokens['--bf-appearance-token-color-bg-primary']);
    expect(document.documentElement.style.getPropertyValue(
      systemCssVariables['motion.duration.fast'],
    )).toBe(settings.tokens['--bf-appearance-token-motion-fast']);

    await cssTokenAppearanceAdapter.apply(undefined, settings, context);
    for (const { target } of DESIGN_SYSTEM_APPEARANCE_TOKEN_PROJECTION) {
      expect(document.documentElement.style.getPropertyValue(target)).toBe('');
    }
  });
});
