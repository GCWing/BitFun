import { describe, expect, it } from 'vitest';

import { isPointInSubmenuBridge, isPointerMovingTowardSubmenu } from './useSubmenuIntent';

const rightSubmenu = {
  left: 220,
  right: 440,
  top: 20,
  bottom: 220,
};

describe('isPointerMovingTowardSubmenu', () => {
  it('recognizes a diagonal path toward a submenu that opens to the right', () => {
    expect(isPointerMovingTowardSubmenu(
      { x: 170, y: 48 },
      { x: 190, y: 82 },
      rightSubmenu,
    )).toBe(true);
  });

  it('recognizes a diagonal path toward a submenu that opens to the left', () => {
    expect(isPointerMovingTowardSubmenu(
      { x: 490, y: 48 },
      { x: 465, y: 92 },
      { left: 240, right: 440, top: 20, bottom: 220 },
    )).toBe(true);
  });

  it('rejects movement away from the open submenu', () => {
    expect(isPointerMovingTowardSubmenu(
      { x: 190, y: 82 },
      { x: 165, y: 96 },
      rightSubmenu,
    )).toBe(false);
  });

  it('rejects vertical movement that does not get closer to the submenu', () => {
    expect(isPointerMovingTowardSubmenu(
      { x: 190, y: 82 },
      { x: 190, y: 130 },
      rightSubmenu,
    )).toBe(false);
  });

  it('keeps the corridor active once the pointer reaches the submenu', () => {
    expect(isPointerMovingTowardSubmenu(
      { x: 205, y: 82 },
      { x: 225, y: 110 },
      rightSubmenu,
    )).toBe(true);
  });
});

describe('isPointInSubmenuBridge', () => {
  const parent = { left: 0, right: 215, top: 20, bottom: 220 };

  it('protects the gap and the inner edge padding without needing a movement vector', () => {
    expect(isPointInSubmenuBridge({ x: 218, y: 80 }, parent, rightSubmenu)).toBe(true);
    expect(isPointInSubmenuBridge({ x: 213, y: 80 }, parent, rightSubmenu)).toBe(true);
    expect(isPointInSubmenuBridge({ x: 180, y: 80 }, parent, rightSubmenu)).toBe(false);
  });

  it('protects a left-opening submenu with upward viewport alignment', () => {
    expect(isPointInSubmenuBridge(
      { x: 442, y: 50 },
      { left: 445, right: 665, top: 150, bottom: 350 },
      rightSubmenu,
    )).toBe(true);
  });

  it('does not protect empty space beyond the height of the submenu', () => {
    expect(isPointInSubmenuBridge({ x: 218, y: 400 }, parent, rightSubmenu)).toBe(false);
  });

  it('uses current geometry instead of assuming a fixed gap width', () => {
    expect(isPointInSubmenuBridge({ x: 230, y: 80 }, parent, {
      ...rightSubmenu, left: 240,
    })).toBe(true);
    expect(isPointInSubmenuBridge({ x: 230, y: 80 }, parent, rightSubmenu)).toBe(false);
  });

  it('does not invent a bridge for unmeasured or overlapping menus', () => {
    expect(isPointInSubmenuBridge({ x: 218, y: 80 }, { ...parent, right: 0 }, rightSubmenu)).toBe(false);
    expect(isPointInSubmenuBridge({ x: 218, y: 80 }, parent, { ...rightSubmenu, left: 100 })).toBe(false);
  });
});
