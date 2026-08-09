import { describe, expect, it } from 'vitest';
import {
  TOOLBAR_EXPANDED_MIN,
  TOOLBAR_EXPANDED_SIZE,
} from './ToolbarModeContext';
import { resolveToolbarWindowGeometry } from './toolbarWindowGeometry';

describe('resolveToolbarWindowGeometry', () => {
  it('uses the authored logical size when the work area has room', () => {
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
    expect(geometry.width).toBe(TOOLBAR_EXPANDED_SIZE.width);
    expect(geometry.height).toBe(TOOLBAR_EXPANDED_SIZE.height);
  });

  it('fits expanded toolbar mode inside a short desktop work area', () => {
    const geometry = resolveToolbarWindowGeometry({
      monitor: {
        position: { x: 0, y: 0 },
        size: { width: 1365, height: 600 },
        workArea: {
          position: { x: 0, y: 0 },
          size: { width: 1365, height: 560 },
        },
        scaleFactor: 1,
      },
      targetSize: TOOLBAR_EXPANDED_SIZE,
      minSize: TOOLBAR_EXPANDED_MIN,
    });

    expect(geometry.y).toBeGreaterThanOrEqual(0);
    expect(geometry.y + geometry.height).toBeLessThanOrEqual(560);
    expect(geometry.height).toBe(520);
  });

  it('scales the authored logical size to physical pixels on HiDPI displays', () => {
    const geometry = resolveToolbarWindowGeometry({
      monitor: {
        position: { x: 0, y: 0 },
        size: { width: 3024, height: 1964 },
        workArea: {
          position: { x: 0, y: 0 },
          size: { width: 3024, height: 1866 },
        },
        scaleFactor: 2,
      },
      targetSize: TOOLBAR_EXPANDED_SIZE,
      minSize: TOOLBAR_EXPANDED_MIN,
    });

    // Same apparent size as on a 1x display — this is what keeps the window
    // matching the floating chat bubble panel rather than rendering half-size.
    expect(geometry.width).toBe(TOOLBAR_EXPANDED_SIZE.width * 2);
    expect(geometry.height).toBe(TOOLBAR_EXPANDED_SIZE.height * 2);
    expect(geometry.minWidth).toBe(TOOLBAR_EXPANDED_MIN.width * 2);
    expect(geometry.minHeight).toBe(TOOLBAR_EXPANDED_MIN.height * 2);
  });

  it('never reports a min size larger than the resolved rect', () => {
    const geometry = resolveToolbarWindowGeometry({
      monitor: {
        position: { x: 0, y: 0 },
        size: { width: 640, height: 480 },
        workArea: {
          position: { x: 0, y: 0 },
          size: { width: 640, height: 400 },
        },
        scaleFactor: 1,
      },
      targetSize: TOOLBAR_EXPANDED_SIZE,
      minSize: TOOLBAR_EXPANDED_MIN,
    });

    expect(geometry.minWidth).toBeLessThanOrEqual(geometry.width);
    expect(geometry.minHeight).toBeLessThanOrEqual(geometry.height);
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
