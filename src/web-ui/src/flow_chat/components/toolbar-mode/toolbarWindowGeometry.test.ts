import { describe, expect, it } from 'vitest';
import {
  TOOLBAR_EXPANDED_MIN,
  TOOLBAR_EXPANDED_SIZE,
} from './ToolbarModeContext';
import { resolveToolbarWindowGeometry } from './toolbarWindowGeometry';

describe('resolveToolbarWindowGeometry', () => {
  it('fits expanded toolbar mode inside a short desktop work area', () => {
    const geometry = resolveToolbarWindowGeometry({
      monitor: {
        position: { x: 0, y: 0 },
        size: { width: 1365, height: 768 },
        workArea: {
          position: { x: 0, y: 0 },
          size: { width: 1365, height: 728 },
        },
        scaleFactor: 1,
      },
      targetSize: TOOLBAR_EXPANDED_SIZE,
      minSize: TOOLBAR_EXPANDED_MIN,
    });

    expect(geometry.y).toBeGreaterThanOrEqual(0);
    expect(geometry.y + geometry.height).toBeLessThanOrEqual(728);
    expect(geometry.height).toBe(688);
  });

  it('keeps the toolbar within a secondary monitor work area', () => {
    const geometry = resolveToolbarWindowGeometry({
      monitor: {
        position: { x: -1280, y: 0 },
        size: { width: 1280, height: 1024 },
        workArea: {
          position: { x: -1280, y: 24 },
          size: { width: 1280, height: 960 },
        },
        scaleFactor: 1,
      },
      targetSize: TOOLBAR_EXPANDED_SIZE,
      minSize: TOOLBAR_EXPANDED_MIN,
    });

    expect(geometry.x).toBeGreaterThanOrEqual(-1280);
    expect(geometry.x + geometry.width).toBeLessThanOrEqual(0);
    expect(geometry.y).toBeGreaterThanOrEqual(24);
    expect(geometry.y + geometry.height).toBeLessThanOrEqual(984);
  });

  it('preserves the bottom edge margin when expanding from compact mode', () => {
    const geometry = resolveToolbarWindowGeometry({
      monitor: {
        position: { x: 0, y: 0 },
        size: { width: 1920, height: 1080 },
        workArea: {
          position: { x: 0, y: 0 },
          size: { width: 1920, height: 1040 },
        },
        scaleFactor: 1,
      },
      targetSize: TOOLBAR_EXPANDED_SIZE,
      minSize: TOOLBAR_EXPANDED_MIN,
      anchor: {
        x: 1200,
        y: 900,
        width: 700,
        height: 140,
      },
    });

    expect(geometry.y + geometry.height).toBe(1020);
  });
});
