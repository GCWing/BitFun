// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import { getAppearanceOverlayHost } from './AppearanceOverlayHost';

// Read as text rather than imported: the bundler hands stylesheets to the test
// runner as empty modules, and these assertions are about the CSS itself.
const readSource = (fromProjectRoot: string): string =>
  readFileSync(resolve(process.cwd(), fromProjectRoot), 'utf8');

const layerStyles = readSource('src/infrastructure/appearance/runtime/AppearanceOverlayHost.scss');
const systemTokens = JSON.parse(readSource(
  '../../design-system/packages/design-tokens/src/system.tokens.json',
)) as {
  layer: Record<string, { $value: number }>;
};

/** The host's own rule block, without the child rule that follows it. */
const hostRule = layerStyles.slice(
  layerStyles.indexOf('#openbitfun-appearance-overlay-host {'),
  layerStyles.indexOf('}', layerStyles.indexOf('#openbitfun-appearance-overlay-host {')),
);

describe('appearance overlay host', () => {
  afterEach(() => {
    document.getElementById('openbitfun-appearance-overlay-host')?.remove();
  });

  it('mounts one reusable host on the body', () => {
    const host = getAppearanceOverlayHost();

    expect(host.parentElement).toBe(document.body);
    expect(host.getAttribute('data-openbitfun-overlay-host')).toBe('true');
    expect(getAppearanceOverlayHost()).toBe(host);
    expect(document.querySelectorAll('[data-openbitfun-overlay-host]')).toHaveLength(1);
  });

  // The host is a stacking layer, not just a mount point. Drop either half and
  // an overlay portaled out of a container that floats above app content — the
  // floating mini chat panel, a maximized canvas — paints behind that container
  // and never receives its clicks.
  it('gives portaled overlays their own stacking context', () => {
    expect(hostRule).toMatch(/position:\s*fixed/);
    expect(hostRule).toMatch(/z-index:\s*var\(--openbitfun-layer-overlay-host\)/);
  });

  it('stacks above containers that host app UI and below always-on-top chrome', () => {
    expect(systemTokens.layer.overlayHost.$value).toBeGreaterThan(systemTokens.layer.overlay.$value);
    expect(systemTokens.layer.overlayHost.$value).toBeLessThan(systemTokens.layer.notification.$value);
    expect(systemTokens.layer.overlayHost.$value).toBeLessThan(systemTokens.layer.contextMenu.$value);
  });

  // A containing block on the host would re-anchor every `position: fixed`
  // overlay to the host box instead of the viewport, moving all of them.
  it('never becomes a containing block for fixed-position overlays', () => {
    expect(hostRule).not.toMatch(/(^|[^-])(transform|filter|backdrop-filter|will-change|contain):/);
  });

  it('keeps the layer itself click-through and its overlays hit-testable', () => {
    expect(hostRule).toMatch(/pointer-events:\s*none/);
    expect(layerStyles).toMatch(
      /:where\(#openbitfun-appearance-overlay-host\)\s*>\s*:where\(\*\)\s*\{\s*pointer-events:\s*auto/,
    );
  });
});
