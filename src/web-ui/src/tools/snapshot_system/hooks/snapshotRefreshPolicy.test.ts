import { describe, expect, it } from 'vitest';
import { hasSessionFileSnapshots, shouldRefreshSnapshotForSession } from './snapshotRefreshPolicy';

describe('snapshot refresh policy', () => {
  it.each([
    { remoteConnectionId: 'ssh-disconnected' },
    { remoteSshHost: 'saved-host' },
    { config: { remoteConnectionId: 'ssh-legacy' } },
    { config: { remoteSshHost: 'legacy-host' } },
    { remoteConnectionId: 'ssh-loopback', remoteSshHost: 'localhost' },
  ])('does not request local file snapshots for remote session %j', (binding) => {
    const session = { ...binding, isHistorical: true, historyState: 'ready' as const };
    expect(hasSessionFileSnapshots(session)).toBe(false);
    expect(shouldRefreshSnapshotForSession(session)).toBe(false);
  });

  it('keeps snapshots available on the owning host for legacy local sessions', () => {
    expect(hasSessionFileSnapshots({ remoteSshHost: 'localhost' })).toBe(true);
    expect(hasSessionFileSnapshots({ config: { remoteSshHost: '127.0.0.1' } })).toBe(true);
  });

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

  it('defers snapshot refresh while backend context restore is pending', () => {
    expect(shouldRefreshSnapshotForSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
    })).toBe(false);
    expect(shouldRefreshSnapshotForSession({
      isHistorical: true,
      historyState: 'ready',
      contextRestoreState: 'pending',
    })).toBe(false);
    expect(shouldRefreshSnapshotForSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
    })).toBe(true);
  });
});
