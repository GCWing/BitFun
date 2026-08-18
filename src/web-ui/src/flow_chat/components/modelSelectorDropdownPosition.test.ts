import { describe, expect, it } from 'vitest';
import {
  getModelSelectorDropdownLayout,
  getModelSelectorDropdownStyle,
} from './modelSelectorDropdownPosition';

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

  it('aligns the menu right edge with the trigger when end-aligned', () => {
    const style = getModelSelectorDropdownStyle(
      { left: 700, right: 820, top: 700, bottom: 724 },
      { width: 268, height: 300 },
      'top',
      { width: 900, height: 900 },
      'end',
    );

    // 820 - 268: the menu's right edge lands on the button's right edge.
    expect(style.left).toBe('552px');
    expect(style.top).toBe('394px');
  });

  it('still clamps an end-aligned menu into a narrow viewport', () => {
    const style = getModelSelectorDropdownStyle(
      { left: 24, right: 120, top: 500, bottom: 524 },
      { width: 268, height: 300 },
      'top',
      { width: 200, height: 700 },
      'end',
    );

    // 200 - 268 - 8 would be negative, so the padding clamp owns the left edge.
    expect(style.left).toBe('8px');
    expect(style.top).toBe('194px');
  });

  it('shrinks a tall menu to the space above instead of overlapping the trigger', () => {
    const layout = getModelSelectorDropdownLayout(
      { left: 700, right: 820, top: 380, bottom: 410 },
      { width: 268, height: 392 },
      'top',
      { width: 900, height: 500 },
      'end',
    );

    // The viewport has 366 px between its 8 px inset and the trigger's 6 px gap.
    expect(layout.style.left).toBe('552px');
    expect(layout.style.top).toBe('8px');
    expect(layout.style.maxHeight).toBe('366px');
    expect(layout.placement).toBe('top');
  });

  it('flips below the trigger when the preferred top placement does not fit', () => {
    const layout = getModelSelectorDropdownLayout(
      { left: 24, top: 20, bottom: 44 },
      { width: 240, height: 200 },
      'top',
      { width: 800, height: 600 },
    );

    expect(layout.style.top).toBe('50px');
    expect(layout.placement).toBe('bottom');
  });
});
