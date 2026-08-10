import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';

import { buildCanvasRuntimeInstallerScript } from './canvasRuntimeInstaller';

describe('Canvas runtime installer', () => {
  it('keeps iframe-local shape, spacing, and type fallbacks out of the host payload contract', () => {
    const runtimeCss = readFileSync(new URL('./styles/canvas-runtime.scss', import.meta.url), 'utf8');
    const iframeFallbackVars = [
      '--bf-appearance-token-font-size-xs',
      '--bf-appearance-token-font-size-sm',
      '--bf-appearance-token-font-size-base',
      '--bf-appearance-token-font-size-lg',
      '--bf-appearance-token-font-size-2xl',
      '--bf-appearance-token-font-weight-medium',
      '--bf-appearance-token-font-weight-semibold',
      '--bf-appearance-token-size-radius-sm',
      '--bf-appearance-token-size-radius-base',
      '--bf-appearance-token-size-radius-md',
      '--bf-appearance-token-size-radius-lg',
      '--bf-appearance-token-size-radius-xl',
      '--bf-appearance-token-size-radius-2xl',
      '--bf-appearance-token-size-radius-full',
      '--bf-appearance-token-size-gap-1',
      '--bf-appearance-token-size-gap-2',
      '--bf-appearance-token-size-gap-3',
      '--bf-appearance-token-size-gap-4',
      '--bf-appearance-token-size-gap-5',
      '--bf-appearance-token-size-gap-6',
      '--bf-appearance-token-size-gap-8',
      '--bf-appearance-token-size-gap-10',
      '--bf-appearance-token-size-gap-12',
      '--bf-appearance-token-size-gap-16',
    ];

    for (const name of iframeFallbackVars) {
      expect(runtimeCss).toContain(`${name}:`);
    }
    const smallFontSizeValues = [...runtimeCss.matchAll(/--bf-appearance-token-font-size-sm:\s*([^;]+);/g)].map(
      (match) => match[1]?.trim()
    );

    expect(smallFontSizeValues).toEqual(['13px']);
  });

  it('merges bundled SDK adapters before user module startup', () => {
    const script = buildCanvasRuntimeInstallerScript('rev_test');

    expect(script).toContain('function installSdkAdapters()');
    expect(script).toContain('...runtimeWindow.BitfunCanvasSDKAdapters');
    expect(script).toContain('runtimeWindow.BitfunCanvasRuntimeHooks');
    expect(script.indexOf('installSdkAdapters();')).toBeLessThan(
      script.indexOf('bitfun-canvas-module-started'),
    );
  });

  it('keeps fallback SDK scoped to runtime hooks while bundled adapters own components', () => {
    const script = buildCanvasRuntimeInstallerScript('rev_test');

    expect(script).toContain('runtimeWindow.BitfunCanvasSDK = {');
    expect(script).toContain('...runtimeWindow.BitfunCanvasRuntimeHooks');
    expect(script).not.toContain('function Stack');
    expect(script).not.toContain('function BarChart');
    expect(script).not.toContain('function DependencyGraph');
  });

  it('syncs browser color scheme when the host appearance changes', () => {
    const script = buildCanvasRuntimeInstallerScript('rev_test');

    expect(script).toContain('nextAppearance.type === "dark" || nextAppearance.type === "light"');
    expect(script).toContain('document.documentElement.style.colorScheme = nextAppearance.type');
  });

  it('installs design-mode element selection handlers', () => {
    const script = buildCanvasRuntimeInstallerScript('rev_test');

    expect(script).toContain('bitfun-canvas-design-mode');
    expect(script).toContain('data-bitfun-canvas-design-mode');
    expect(script).toContain('bitfun-canvas-element-selected');
    expect(script).toContain('document.addEventListener("pointermove"');
    expect(script).toContain('document.addEventListener("click"');
  });
});
