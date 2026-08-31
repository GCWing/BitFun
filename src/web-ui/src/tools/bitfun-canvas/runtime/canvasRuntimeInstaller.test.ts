import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';

import { buildCanvasRuntimeInstallerScript } from './canvasRuntimeInstaller';

describe('Canvas runtime installer', () => {
  it('loads generated typography tokens while keeping iframe-local shape and spacing fallbacks', () => {
    const runtimeCss = readFileSync(new URL('./styles/canvas-runtime.scss', import.meta.url), 'utf8');
    const runtimeEntry = readFileSync(new URL('./entry.tsx', import.meta.url), 'utf8');
    const iframeFallbackVars = [
      '--bf-radius-sm',
      '--bf-radius-base',
      '--bf-radius-md',
      '--bf-radius-lg',
      '--bf-radius-xl',
      '--bf-radius-2xl',
      '--bf-radius-pill',
      '--bf-space-1',
      '--bf-space-2',
      '--bf-space-3',
      '--bf-space-4',
      '--bf-space-5',
      '--bf-space-6',
      '--bf-space-8',
      '--bf-space-10',
      '--bf-space-12',
      '--bf-space-16',
    ];

    for (const name of iframeFallbackVars) {
      expect(runtimeCss).toContain(`${name}:`);
    }
    expect(runtimeEntry).toContain("import '@bitfun/design-tokens/tokens.css';");
    expect(runtimeCss).not.toMatch(/^\s*--bf-(?:font|letter-spacing|line-height|type)-/m);
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
