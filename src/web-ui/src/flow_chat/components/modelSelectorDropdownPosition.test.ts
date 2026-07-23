import { describe, expect, it } from 'vitest';
import { getModelSelectorDropdownStyle } from './modelSelectorDropdownPosition';

describe('getModelSelectorDropdownStyle', () => {
  it('preserves the measured intrinsic width in a wide viewport', () => {
    const style = getModelSelectorDropdownStyle(
      { left: 700, top: 700, bottom: 724 },
      { width: 268, height: 300 },
      'top',
      { width: 900, height: 900 },
    );

    expect(style.left).toBe('624px');
    expect(style.top).toBe('394px');
    expect(style).not.toHaveProperty('width');
  });

  it('keeps a viewport-constrained dropdown inside a narrow window', () => {
    const style = getModelSelectorDropdownStyle(
      { left: 140, top: 500, bottom: 524 },
      { width: 164, height: 300 },
      'top',
      { width: 180, height: 700 },
    );

    expect(style.left).toBe('8px');
    expect(style.top).toBe('194px');
  });

  it('flips below the trigger when the preferred top placement does not fit', () => {
    const style = getModelSelectorDropdownStyle(
      { left: 24, top: 20, bottom: 44 },
      { width: 240, height: 200 },
      'top',
      { width: 800, height: 600 },
    );

    expect(style.top).toBe('50px');
  });
});
