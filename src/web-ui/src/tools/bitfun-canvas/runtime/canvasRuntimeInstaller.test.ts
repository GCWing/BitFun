import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';

import { buildCanvasRuntimeInstallerScript } from './canvasRuntimeInstaller';

describe('Canvas runtime installer', () => {
  it('keeps iframe-local shape, spacing, and type fallbacks out of the host payload contract', () => {
    const runtimeCss = readFileSync(new URL('./styles/canvas-runtime.scss', import.meta.url), 'utf8');
    const iframeFallbackVars = [
      '--font-size-xs',
      '--font-size-sm',
      '--font-size-base',
      '--font-size-lg',
      '--font-size-2xl',
      '--font-weight-medium',
      '--font-weight-semibold',
      '--size-radius-sm',
      '--size-radius-base',
      '--size-radius-md',
      '--size-radius-lg',
      '--size-radius-xl',
      '--size-radius-2xl',
      '--size-radius-full',
      '--size-gap-1',
      '--size-gap-2',
      '--size-gap-3',
      '--size-gap-4',
      '--size-gap-5',
      '--size-gap-6',
      '--size-gap-8',
      '--size-gap-10',
      '--size-gap-12',
      '--size-gap-16',
    ];

    for (const name of iframeFallbackVars) {
      expect(runtimeCss).toContain(`${name}:`);
    }
    const smallFontSizeValues = [...runtimeCss.matchAll(/--font-size-sm:\s*([^;]+);/g)].map(
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

  it('syncs browser color scheme when the host theme changes', () => {
    const script = buildCanvasRuntimeInstallerScript('rev_test');

    expect(script).toContain('nextTheme.type === "dark" || nextTheme.type === "light"');
    expect(script).toContain('document.documentElement.style.colorScheme = nextTheme.type');
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
