import { describe, expect, it } from 'vitest';

import {
  getEffectiveTopLevelSessionCount,
  getSessionBufferPrefetchLimit,
  getSessionDisplayLimit,
  getSessionExpandToggleState,
} from './sessionNavExpand';

describe('getSessionDisplayLimit', () => {
  it('keeps the grouped workspace list on its compact expansion levels', () => {
    expect(getSessionDisplayLimit({
      loadedTopLevelCount: 240,
      expandLevel: 0,
      level2DisplayCount: 200,
      showAllWithoutLimit: false,
    })).toBe(5);
    expect(getSessionDisplayLimit({
      loadedTopLevelCount: 240,
      expandLevel: 1,
      level2DisplayCount: 200,
      showAllWithoutLimit: false,
    })).toBe(10);
    expect(getSessionDisplayLimit({
      loadedTopLevelCount: 240,
      expandLevel: 2,
      level2DisplayCount: 200,
      showAllWithoutLimit: false,
    })).toBe(200);
  });

  it('shows every loaded aggregate session without a 200-row display cap', () => {
    expect(getSessionDisplayLimit({
      loadedTopLevelCount: 1_250,
      expandLevel: 0,
      level2DisplayCount: 200,
      showAllWithoutLimit: true,
    })).toBe(1_250);
  });
});

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

describe('getSessionBufferPrefetchLimit', () => {
  it('tops the collapsed view up to a few rows past what it renders', () => {
    expect(getSessionBufferPrefetchLimit({
      expandLevel: 0,
      loadedTopLevelCount: 5,
      totalTopLevelCount: 40,
      hasMore: true,
    })).toBe(3);
  });

  it('refills only the rows a delete consumed', () => {
    expect(getSessionBufferPrefetchLimit({
      expandLevel: 0,
      loadedTopLevelCount: 7,
      totalTopLevelCount: 40,
      hasMore: true,
    })).toBe(1);
  });

  it('stops once the buffer is full', () => {
    expect(getSessionBufferPrefetchLimit({
      expandLevel: 0,
      loadedTopLevelCount: 8,
      totalTopLevelCount: 40,
      hasMore: true,
    })).toBe(0);
  });

  it('never asks for more sessions than the workspace has', () => {
    expect(getSessionBufferPrefetchLimit({
      expandLevel: 0,
      loadedTopLevelCount: 5,
      totalTopLevelCount: 6,
      hasMore: true,
    })).toBe(1);
  });

  it('buffers past the first expand step as well', () => {
    expect(getSessionBufferPrefetchLimit({
      expandLevel: 1,
      loadedTopLevelCount: 10,
      totalTopLevelCount: 40,
      hasMore: true,
    })).toBe(3);
  });

  it('skips prefetching when everything is loaded or fully expanded', () => {
    expect(getSessionBufferPrefetchLimit({
      expandLevel: 0,
      loadedTopLevelCount: 5,
      totalTopLevelCount: 5,
      hasMore: false,
    })).toBe(0);
    expect(getSessionBufferPrefetchLimit({
      expandLevel: 2,
      loadedTopLevelCount: 40,
      totalTopLevelCount: 400,
      hasMore: true,
    })).toBe(0);
  });
});
