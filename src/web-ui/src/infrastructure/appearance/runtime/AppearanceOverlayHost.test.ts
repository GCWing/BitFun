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
const tokens = readSource('src/component-library/styles/tokens.scss');

/** Numeric value of a `$z-*` token, so the ordering below is read, not assumed. */
const zLevel = (token: string): number => {
  const match = tokens.match(new RegExp(`^\\$${token}:\\s*(\\d+);`, 'm'));
  if (!match) throw new Error(`missing z token: $${token}`);
  return Number(match[1]);
};

/** The host's own rule block, without the child rule that follows it. */
const hostRule = layerStyles.slice(
  layerStyles.indexOf('#bitfun-appearance-overlay-host {'),
  layerStyles.indexOf('}', layerStyles.indexOf('#bitfun-appearance-overlay-host {')),
);

describe('appearance overlay host', () => {
  afterEach(() => {
    document.getElementById('bitfun-appearance-overlay-host')?.remove();
  });

  it('mounts one reusable host on the body', () => {
    const host = getAppearanceOverlayHost();

    expect(host.parentElement).toBe(document.body);
    expect(host.getAttribute('data-bf-overlay-host')).toBe('true');
    expect(getAppearanceOverlayHost()).toBe(host);
    expect(document.querySelectorAll('[data-bf-overlay-host]')).toHaveLength(1);
  });

  // The host is a stacking layer, not just a mount point. Drop either half and
  // an overlay portaled out of a container that floats above app content — the
  // floating mini chat panel, a maximized canvas — paints behind that container
  // and never receives its clicks.
  it('gives portaled overlays their own stacking context', () => {
    expect(hostRule).toMatch(/position:\s*fixed/);
    expect(hostRule).toMatch(/z-index:\s*\$z-overlay-host/);
  });

  it('stacks above containers that host app UI and below always-on-top chrome', () => {
    expect(zLevel('z-overlay-host')).toBeGreaterThan(zLevel('z-overlay'));
    expect(zLevel('z-overlay-host')).toBeLessThan(zLevel('z-notification'));
    expect(zLevel('z-overlay-host')).toBeLessThan(zLevel('z-context-menu'));
  });

  // A containing block on the host would re-anchor every `position: fixed`
  // overlay to the host box instead of the viewport, moving all of them.
  it('never becomes a containing block for fixed-position overlays', () => {
    expect(hostRule).not.toMatch(/(^|[^-])(transform|filter|backdrop-filter|will-change|contain):/);
  });

  it('keeps the layer itself click-through and its overlays hit-testable', () => {
    expect(hostRule).toMatch(/pointer-events:\s*none/);
    expect(layerStyles).toMatch(
      /:where\(#bitfun-appearance-overlay-host\)\s*>\s*:where\(\*\)\s*\{\s*pointer-events:\s*auto/,
    );
  });
});
