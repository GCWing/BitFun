import type { DeviceSurfaceId } from '@/infrastructure/peer-device/deviceSurface';
import type { Session } from '../types/flow-chat';
import type { SessionNavStatus, SessionNavStatusKind } from './sessionNavStatus';

interface Entry {
  session: Session;
  status: SessionNavStatus;
  lastKnownKind?: SessionNavStatusKind;
  sortTimestamp: number;
}

/** Navigation recency is a view projection, independent of selection and transcript writes. */
export class SessionNavOrdering {
  private surfaces = new Map<DeviceSurfaceId, Map<string, Entry>>();
  private latestTimestamps = new Map<DeviceSurfaceId, number>();

  get(surfaceId: DeviceSurfaceId, sessionId: string): Readonly<Entry> | undefined {
    return this.surfaces.get(surfaceId)?.get(sessionId);
  }

  sessionIds(surfaceId: DeviceSurfaceId): Iterable<string> {
    return this.surfaces.get(surfaceId)?.keys() ?? [];
  }

  observe(surfaceId: DeviceSurfaceId, session: Session, status: SessionNavStatus, now = Date.now()): boolean {
    let entries = this.surfaces.get(surfaceId);
    if (!entries) this.surfaces.set(surfaceId, entries = new Map());
    const previous = entries.get(session.sessionId);
    // Capture historical recency once. Hydration, read receipts, title changes,
    // streaming and completion must never become new navigation activity.
    let sortTimestamp = previous?.sortTimestamp ?? session.lastFinishedAt ?? session.createdAt;
    if (status.kind === 'running' && previous?.lastKnownKind && previous.lastKnownKind !== 'running') {
      // Keep consecutive starts ordered even within one clock tick or after a clock adjustment.
      sortTimestamp = Math.max(now, (this.latestTimestamps.get(surfaceId) ?? sortTimestamp) + 1);
    }
    this.latestTimestamps.set(surfaceId, Math.max(this.latestTimestamps.get(surfaceId) ?? 0, sortTimestamp));
    entries.set(session.sessionId, {
      session,
      status,
      sortTimestamp,
      // Losing connectivity is not an execution transition. A running session
      // must not jump again when the same host snapshot becomes available.
      lastKnownKind: status.kind === 'syncing' ? previous?.lastKnownKind : status.kind,
    });
    return !previous || previous.sortTimestamp !== sortTimestamp
      || (previous.status.kind === 'running') !== (status.kind === 'running');
  }

  delete(surfaceId: DeviceSurfaceId, sessionId: string): boolean {
    return this.surfaces.get(surfaceId)?.delete(sessionId) ?? false;
  }

  clearSurface(surfaceId: DeviceSurfaceId): void {
    this.surfaces.delete(surfaceId);
    this.latestTimestamps.delete(surfaceId);
  }

  clear(): void {
    this.surfaces.clear();
    this.latestTimestamps.clear();
  }
}
