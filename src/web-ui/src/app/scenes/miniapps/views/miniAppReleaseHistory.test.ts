import { describe, expect, it } from 'vitest';
import { buildReleaseHistory } from './miniAppReleaseHistory';

describe('MiniApp market release history', () => {
  it('collapses to the newest release', () => {
    const history = buildReleaseHistory(
      [{ releaseNumber: 2 }, { releaseNumber: 1 }],
      false,
    );

    expect(history.visible).toEqual([{ releaseNumber: 2 }]);
    expect(history.hiddenCount).toBe(1);
  });

  it('shows every release newest-first once expanded', () => {
    const history = buildReleaseHistory(
      [{ releaseNumber: 1 }, { releaseNumber: 3 }, { releaseNumber: 2 }],
      true,
    );

    expect(history.visible.map((release) => release.releaseNumber)).toEqual([3, 2, 1]);
    expect(history.hiddenCount).toBe(0);
  });

  it('keeps the newest release visible when the list arrives out of order', () => {
    const history = buildReleaseHistory(
      [{ releaseNumber: 1 }, { releaseNumber: 4 }],
      false,
    );

    expect(history.visible).toEqual([{ releaseNumber: 4 }]);
  });

  it('offers nothing to expand for a single release', () => {
    const history = buildReleaseHistory([{ releaseNumber: 1 }], false);

    expect(history.visible).toEqual([{ releaseNumber: 1 }]);
    expect(history.hiddenCount).toBe(0);
  });
});
