import { describe, expect, it } from 'vitest';

import { buildCanvasRuntimeInstallerScript } from './canvasRuntimeInstaller';

describe('Canvas runtime installer', () => {
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
