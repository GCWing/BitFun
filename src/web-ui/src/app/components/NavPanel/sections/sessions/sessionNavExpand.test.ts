import { describe, expect, it } from 'vitest';

import {
  getEffectiveTopLevelSessionCount,
  getSessionExpandToggleState,
} from './sessionNavExpand';

describe('getSessionExpandToggleState', () => {
  it('hides the toggle when five or fewer top-level sessions are available', () => {
    expect(getSessionExpandToggleState(1, 0)).toMatchObject({
      collapsedRemainingCount: 0,
      shouldRender: false,
    });
    expect(getSessionExpandToggleState(5, 0)).toMatchObject({
      collapsedRemainingCount: 0,
      shouldRender: false,
    });
  });

  it('shows the first expand step only when there are hidden top-level sessions', () => {
    expect(getSessionExpandToggleState(6, 0)).toMatchObject({
      action: 'show-more',
      collapsedRemainingCount: 1,
      shouldRender: true,
    });
  });

  it('switches to show-all and show-less using the remaining top-level count', () => {
    expect(getSessionExpandToggleState(12, 1)).toMatchObject({
      action: 'show-all',
      expandedRemainingCount: 2,
      shouldRender: true,
    });
    expect(getSessionExpandToggleState(6, 1)).toMatchObject({
      action: 'show-less',
      expandedRemainingCount: 0,
      shouldRender: true,
    });
    expect(getSessionExpandToggleState(12, 2)).toMatchObject({
      action: 'show-less',
      shouldRender: true,
    });
  });
});

describe('getEffectiveTopLevelSessionCount', () => {
  it('falls back to the live count before metadata is available', () => {
    expect(getEffectiveTopLevelSessionCount(null, null, 3, false)).toBe(3);
  });

  it('keeps live creates visible even when metadata count is stale', () => {
    expect(getEffectiveTopLevelSessionCount(1, 1, 6, false)).toBe(6);
  });

  it('reduces the total count when loaded sessions are deleted', () => {
    expect(getEffectiveTopLevelSessionCount(6, 5, 4, false)).toBe(5);
    expect(getEffectiveTopLevelSessionCount(6, 5, 3, false)).toBe(4);
  });

  it('avoids overcorrecting while a metadata refresh is still loading', () => {
    expect(getEffectiveTopLevelSessionCount(12, 5, 10, true)).toBe(12);
  });
});
