import { create } from 'zustand';
import { useShallow } from 'zustand/react/shallow';

export type ModelBrainstormCandidateStatus = 'pending' | 'starting' | 'running' | 'failed';

export interface ModelBrainstormCandidate {
  id: string;
  modelId: string;
  modelLabel: string;
  providerName?: string;
  sessionId?: string;
  status: ModelBrainstormCandidateStatus;
  error?: string;
}

export interface ModelBrainstormBatch {
  id: string;
  sourceSessionId: string;
  question: string;
  displayQuestion: string;
  createdAt: number;
  selectedCandidateId?: string;
  candidates: ModelBrainstormCandidate[];
}

interface ModelBrainstormState {
  batches: Record<string, ModelBrainstormBatch>;
  order: string[];
  createBatch: (batch: ModelBrainstormBatch) => void;
  updateCandidate: (
    batchId: string,
    candidateId: string,
    updates: Partial<ModelBrainstormCandidate>,
  ) => void;
  selectCandidate: (batchId: string, candidateId: string) => void;
  removeBatchesForSession: (sessionId: string) => void;
  reset: () => void;
}

export const useModelBrainstormStore = create<ModelBrainstormState>((set) => ({
  batches: {},
  order: [],

  createBatch: (batch) => set((state) => ({
    batches: {
      ...state.batches,
      [batch.id]: batch,
    },
    order: state.order.includes(batch.id)
      ? state.order
      : [...state.order, batch.id],
  })),

  updateCandidate: (batchId, candidateId, updates) => set((state) => {
    const batch = state.batches[batchId];
    if (!batch) {
      return state;
    }

    let changed = false;
    const candidates = batch.candidates.map(candidate => {
      if (candidate.id !== candidateId) {
        return candidate;
      }
      changed = true;
      return {
        ...candidate,
        ...updates,
      };
    });

    if (!changed) {
      return state;
    }

    return {
      batches: {
        ...state.batches,
        [batchId]: {
          ...batch,
          candidates,
        },
      },
    };
  }),

  selectCandidate: (batchId, candidateId) => set((state) => {
    const batch = state.batches[batchId];
    if (!batch || !batch.candidates.some(candidate => candidate.id === candidateId)) {
      return state;
    }

    return {
      batches: {
        ...state.batches,
        [batchId]: {
          ...batch,
          selectedCandidateId: candidateId,
        },
      },
    };
  }),

  removeBatchesForSession: (sessionId) => set((state) => {
    const nextBatches = { ...state.batches };
    const nextOrder = state.order.filter(batchId => {
      const batch = state.batches[batchId];
      if (batch?.sourceSessionId !== sessionId) {
        return true;
      }
      delete nextBatches[batchId];
      return false;
    });

    if (nextOrder.length === state.order.length) {
      return state;
    }

    return {
      batches: nextBatches,
      order: nextOrder,
    };
  }),

  reset: () => set({ batches: {}, order: [] }),
}));

export function useModelBrainstormBatchesForSession(sessionId: string | undefined | null): ModelBrainstormBatch[] {
  return useModelBrainstormStore(useShallow((state) => {
    if (!sessionId) {
      return [];
    }

    return state.order
      .map(batchId => state.batches[batchId])
      .filter((batch): batch is ModelBrainstormBatch => batch?.sourceSessionId === sessionId);
  }));
}
