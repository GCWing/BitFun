import { describe, expect, it } from 'vitest';
import { getModelSelectorDropdownStyle } from './modelSelectorDropdownPosition';

describe('getModelSelectorDropdownStyle', () => {
  it('keeps the dropdown inside the right edge of a narrow viewport', () => {
    const style = getModelSelectorDropdownStyle(
      { left: 250, top: 700, bottom: 724, width: 90 },
      'top',
      { width: 320, height: 800 },
    );

    expect(style.left).toBe('92px');
    expect(style.width).toBe('220px');
    expect(style.maxWidth).toBe('304px');
    expect(style.bottom).toBe('106px');
  });

  it('shrinks the dropdown when the viewport cannot fit the preferred minimum width', () => {
    const style = getModelSelectorDropdownStyle(
      { left: 80, top: 600, bottom: 624, width: 80 },
      'bottom',
      { width: 180, height: 700 },
    );

    expect(style.left).toBe('8px');
    expect(style.width).toBe('164px');
    expect(style.maxWidth).toBe('164px');
    expect(style.top).toBe('630px');
  });

  it('caps wide triggers at the dropdown maximum width', () => {
    const style = getModelSelectorDropdownStyle(
      { left: 32, top: 400, bottom: 424, width: 360 },
      'bottom',
      { width: 900, height: 700 },
    );

    expect(style.left).toBe('32px');
    expect(style.width).toBe('280px');
  });
});
