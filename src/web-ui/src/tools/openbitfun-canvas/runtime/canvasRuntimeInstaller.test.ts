import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';

import { buildCanvasRuntimeInstallerScript } from './canvasRuntimeInstaller';

describe('Canvas runtime installer', () => {
  it('loads generated typography tokens while keeping iframe-local shape and spacing fallbacks', () => {
    const runtimeCss = readFileSync(new URL('./styles/canvas-runtime.scss', import.meta.url), 'utf8');
    const runtimeEntry = readFileSync(new URL('./entry.tsx', import.meta.url), 'utf8');
    const iframeFallbackVars = [
      '--openbitfun-radius-sm',
      '--openbitfun-radius-base',
      '--openbitfun-radius-md',
      '--openbitfun-radius-lg',
      '--openbitfun-radius-xl',
      '--openbitfun-radius-2xl',
      '--openbitfun-radius-pill',
      '--openbitfun-space-1',
      '--openbitfun-space-2',
      '--openbitfun-space-3',
      '--openbitfun-space-4',
      '--openbitfun-space-5',
      '--openbitfun-space-6',
      '--openbitfun-space-8',
      '--openbitfun-space-10',
      '--openbitfun-space-12',
      '--openbitfun-space-16',
    ];

    for (const name of iframeFallbackVars) {
      expect(runtimeCss).toContain(`${name}:`);
    }
    expect(runtimeEntry).toContain("import '@openbitfun/design-tokens/tokens.css';");
    expect(runtimeCss).not.toMatch(/^\s*--openbitfun-(?:font|letter-spacing|line-height|type)-/m);
  });

  it('merges bundled SDK adapters before user module startup', () => {
    const script = buildCanvasRuntimeInstallerScript('rev_test');

    expect(script).toContain('function installSdkAdapters()');
    expect(script).toContain('...runtimeWindow.OpenBitFunCanvasSDKAdapters');
    expect(script).toContain('runtimeWindow.OpenBitFunCanvasRuntimeHooks');
    expect(script.indexOf('installSdkAdapters();')).toBeLessThan(
      script.indexOf('openbitfun-canvas-module-started'),
    );
  });

  it('keeps fallback SDK scoped to runtime hooks while bundled adapters own components', () => {
    const script = buildCanvasRuntimeInstallerScript('rev_test');

    expect(script).toContain('runtimeWindow.OpenBitFunCanvasSDK = {');
    expect(script).toContain('...runtimeWindow.OpenBitFunCanvasRuntimeHooks');
    expect(script).not.toContain('function Stack');
    expect(script).not.toContain('function BarChart');
    expect(script).not.toContain('function DependencyGraph');
  });

  it('syncs browser color scheme when the host appearance changes', () => {
    const script = buildCanvasRuntimeInstallerScript('rev_test');

    expect(script).toContain('nextAppearance.type === "dark" || nextAppearance.type === "light"');
    expect(script).toContain('document.documentElement.style.colorScheme = nextAppearance.type');
  });

  it('projects code-change colors into the renderer without changing status colors', () => {
    const runtimeCss = readFileSync(new URL('./styles/canvas-runtime.scss', import.meta.url), 'utf8');
    const script = buildCanvasRuntimeInstallerScript('rev_test');

    expect(runtimeCss).toContain('--openbitfun-color-code-change-added: var(--openbitfun-canvas-success)');
    expect(runtimeCss).toContain('--openbitfun-color-code-change-removed: var(--openbitfun-canvas-danger)');
    expect(script).toContain('appearanceVars["--openbitfun-color-code-change-added"]');
    expect(script).toContain('appearanceVars["--openbitfun-color-code-change-removed"]');
    expect(script).toContain('stripAdded: codeChangeAdded');
    expect(script).toContain('stripRemoved: codeChangeRemoved');
  });

  it('installs design-mode element selection handlers', () => {
    const script = buildCanvasRuntimeInstallerScript('rev_test');

    expect(script).toContain('openbitfun-canvas-design-mode');
    expect(script).toContain('data-openbitfun-canvas-design-mode');
    expect(script).toContain('openbitfun-canvas-element-selected');
    expect(script).toContain('document.addEventListener("pointermove"');
    expect(script).toContain('document.addEventListener("click"');
  });
});
