import { describe, expect, it } from 'vitest';
import { shouldRefreshSnapshotForSession } from './snapshotRefreshPolicy';

describe('snapshot refresh policy', () => {
  it('defers snapshot refresh while persisted history is not ready', () => {
    expect(shouldRefreshSnapshotForSession({
      isHistorical: true,
      historyState: 'metadata-only',
    })).toBe(false);
    expect(shouldRefreshSnapshotForSession({
      isHistorical: true,
      historyState: 'hydrating',
    })).toBe(false);
    expect(shouldRefreshSnapshotForSession({
      isHistorical: true,
      historyState: 'failed',
    })).toBe(false);
  });

  it('keeps snapshot refresh enabled for ready, new, and unknown sessions', () => {
    expect(shouldRefreshSnapshotForSession({
      isHistorical: true,
      historyState: 'ready',
    })).toBe(true);
    expect(shouldRefreshSnapshotForSession({
      isHistorical: false,
      historyState: 'new',
    })).toBe(true);
    expect(shouldRefreshSnapshotForSession(null)).toBe(true);
  });
});
