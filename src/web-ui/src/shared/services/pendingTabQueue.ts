/**
 * pendingTabQueue — stores tab-open events that were fired before the target
 * scene's ContentCanvas had a chance to mount and register its event listener.
 *
 * Usage:
 *   - Producer: call `enqueuePendingTab` instead of dispatching the window event
 *     when the target scene was just added to openTabs (i.e. not yet mounted).
 *   - Consumer: `useTabLifecycle` calls `drainPendingTabs` right after it
 *     registers the window event listener, so no events are missed.
 */

export type TabQueueMode = 'project' | 'agent' | 'git';

export interface PendingTabDetail {
  type: string;
  title: string;
  data: any;
  metadata?: Record<string, any>;
  checkDuplicate?: boolean;
  duplicateCheckKey?: string;
  replaceExisting?: boolean;
  targetGroup?: string;
  enableSplitView?: boolean;
}

const queues = new Map<TabQueueMode, PendingTabDetail[]>();

/** Enqueue a tab-open event for later processing. */
export function enqueuePendingTab(mode: TabQueueMode, detail: PendingTabDetail): void {
  const current = queues.get(mode) ?? [];
  queues.set(mode, [...current, detail]);
}

/**
 * Drain all pending tab events for the given mode.
 * Clears the queue so subsequent calls return an empty array.
 */
export function drainPendingTabs(mode: TabQueueMode): PendingTabDetail[] {
  const items = queues.get(mode) ?? [];
  queues.delete(mode);
  return items;
}
