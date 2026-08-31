import { describe, expect, it } from 'vitest';
import { builtinAppearancePackages } from '../builtins/catalog';
import { themeTokenAppearanceAdapter } from './ThemeTokenAppearanceAdapter';
import { MonacoAppearanceAdapter } from './MonacoAppearanceAdapter';
import { widgetAppearanceAdapter } from './WidgetAppearanceAdapter';

describe('renderer appearance contracts', () => {
  it('accepts every built-in theme token payload and rejects unregistered token names', () => {
    builtinAppearancePackages.forEach(pkg => {
      expect(themeTokenAppearanceAdapter.validate(pkg.renderers?.['theme-tokens']?.settings)).toEqual([]);
    });

    const settings = builtinAppearancePackages[0].renderers?.['theme-tokens']?.settings;
    expect(settings).toBeDefined();
    expect(themeTokenAppearanceAdapter.validate({
      ...settings,
      tokens: { ...settings?.tokens, '--bf-color-unregistered': '#ffffff' },
    })).toContain('Unsupported root token name: --bf-color-unregistered');
  });

  it('accepts every built-in Widget payload and rejects unregistered variables', () => {
    builtinAppearancePackages.forEach(pkg => {
      expect(widgetAppearanceAdapter.validate(pkg.renderers?.['generative-widget']?.settings)).toEqual([]);
    });

    const settings = builtinAppearancePackages[0].renderers?.['generative-widget']?.settings;
    expect(settings).toBeDefined();
    expect(widgetAppearanceAdapter.validate({
      ...settings,
      vars: { ...settings?.vars, '--unregistered-widget-variable': '#ffffff' },
    })).toContain('vars.--unregistered-widget-variable is not registered by the widget host contract');
  });

  it('enforces Monaco theme names before Monaco is attached', async () => {
    const adapter = new MonacoAppearanceAdapter();
    const validSettings = {
      id: 'appearance-bitfun-linglong',
      base: 'vs-dark' as const,
      inherit: true,
      rules: [],
      colors: { 'editor.background': '#0b1420' },
    };

    expect(adapter.validate(validSettings)).toEqual([]);
    expect(adapter.validate({ ...validSettings, id: 'appearance.bitfun-linglong' })).toContain(
      'id must contain only lowercase letters, digits, and hyphens',
    );
    await expect(adapter.apply(
      { ...validSettings, id: 'appearance.bitfun-linglong' },
      undefined,
      { revision: 1, appearanceId: 'bitfun-linglong', mode: 'dark', globals: {}, assets: {} },
    )).rejects.toThrow('Invalid Monaco appearance');
  });
});
