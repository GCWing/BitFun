import { describe, expect, it } from 'vitest';
import { fitProjectionRows } from './terminalProjectionGeometry';

describe('terminal projection row geometry', () => {
  it('keeps a fitting row that canvas averaging and parseInt would drop at 125%', () => {
    // Preserve complete rows, but never reclaim a row by clipping its canvas.
    expect(fitProjectionRows(648, 27, 1.25)).toBe(30);
    expect(fitProjectionRows(647.8, 27, 1.25)).toBe(29);
    // A 26-device-pixel row is 20.8px. A previous 22-row canvas reports
    // round(22 * 20.8) / 22 = 20.8182px, which incorrectly loses row 30.
    const averagedHeight = Math.round(22 * 26 / 1.25) / 22;
    expect(Math.floor(624 / averagedHeight)).toBe(29);
    expect(fitProjectionRows(624, 26, 1.25)).toBe(30);
    expect(fitProjectionRows(645.5, 26, 1.25)).toBe(31);
  });

  it('fits the maximum complete canvas without clipping or oscillating', () => {
    for (const dpr of [1, 1.25, 1.5, 2]) {
      for (const height of [420, 420.4, 624, 647.8, 648, 669.8, 670, 900]) {
        const rows = fitProjectionRows(height, 27, dpr)!;
        expect(Math.round(rows * 27 / dpr)).toBeLessThanOrEqual(height);
        expect(Math.round((rows + 1) * 27 / dpr)).toBeGreaterThan(height);
        expect(fitProjectionRows(height, 27, dpr)).toBe(rows);
      }
    }
  });

  it('waits for valid measurements', () => {
    expect(fitProjectionRows(0, 27, 1.25)).toBeUndefined();
    expect(fitProjectionRows(600, 0, 1.25)).toBeUndefined();
    expect(fitProjectionRows(600, 27, NaN)).toBeUndefined();
  });
});
