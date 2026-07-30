import { create } from 'zustand';

export interface RuntimeStatusEntry {
  sessionId: string;
  turnId: string;
  roundId: string;
}

export interface RuntimeStatusFilter {
  sessionId: string;
  turnId?: string;
  roundId?: string;
}

interface RuntimeStatusStoreState {
  bySessionId: Map<string, RuntimeStatusEntry>;
  show: (entry: RuntimeStatusEntry) => void;
  clear: (filter: RuntimeStatusFilter) => void;
  reset: () => void;
}

function matchesFilter(entry: RuntimeStatusEntry, filter: RuntimeStatusFilter): boolean {
  return entry.sessionId === filter.sessionId
    && (!filter.turnId || entry.turnId === filter.turnId)
    && (!filter.roundId || entry.roundId === filter.roundId);
}

export const useRuntimeStatusStore = create<RuntimeStatusStoreState>((set) => ({
  bySessionId: new Map(),

  show: (entry) => set((state) => {
    if (state.bySessionId.get(entry.sessionId) === entry) {
      return state;
    }
    const next = new Map(state.bySessionId);
    next.set(entry.sessionId, entry);
    return { bySessionId: next };
  }),

  clear: (filter) => set((state) => {
    const current = state.bySessionId.get(filter.sessionId);
    if (!current || !matchesFilter(current, filter)) {
      return state;
    }
    const next = new Map(state.bySessionId);
    next.delete(filter.sessionId);
    return { bySessionId: next };
  }),

  reset: () => set({ bySessionId: new Map() }),
}));

export function getRuntimeStatus(sessionId: string): RuntimeStatusEntry | undefined {
  return useRuntimeStatusStore.getState().bySessionId.get(sessionId);
}

export function showRuntimeStatus(entry: RuntimeStatusEntry): void {
  useRuntimeStatusStore.getState().show(entry);
}

export function clearRuntimeStatusState(filter: RuntimeStatusFilter): void {
  useRuntimeStatusStore.getState().clear(filter);
}

export function resetRuntimeStatuses(): void {
  useRuntimeStatusStore.getState().reset();
}
