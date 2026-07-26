import { create } from 'zustand';
import { useShallow } from 'zustand/react/shallow';

export type ModelBrainstormCandidateStatus = 'pending' | 'starting' | 'running' | 'failed';
export type ModelBrainstormContextMode = 'independent' | 'shared';

export interface ModelBrainstormCandidate {
  id: string;
  modelId: string;
  modelLabel: string;
  providerName?: string;
  sessionId?: string;
  status: ModelBrainstormCandidateStatus;
  error?: string;
  answer?: string;
  completedAt?: number;
}

export interface ModelBrainstormPublicSelection {
  id: string;
  candidateId: string;
  modelId: string;
  modelLabel: string;
  answer: string;
  createdAt: number;
}

export interface ModelBrainstormBatch {
  id: string;
  roomId?: string;
  sourceSessionId: string;
  contextMode?: ModelBrainstormContextMode;
  question: string;
  displayQuestion: string;
  createdAt: number;
  selectedCandidateId?: string;
  selectedCandidateIds?: string[];
  publicSelections?: ModelBrainstormPublicSelection[];
  candidates: ModelBrainstormCandidate[];
}

export interface ModelBrainstormReusableCandidateSession {
  sessionId: string;
  batchId: string;
  candidateId: string;
  modelId: string;
  modelLabel: string;
  createdAt: number;
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
  setCandidateAnswer: (
    batchId: string,
    candidateId: string,
    answer: string,
    completedAt?: number,
  ) => void;
  selectCandidate: (batchId: string, candidateId: string) => void;
  toggleCandidatePublicSelection: (
    batchId: string,
    candidateId: string,
    answer: string,
    selected?: boolean,
  ) => void;
  getBatchesForSession: (sessionId: string) => ModelBrainstormBatch[];
  getReusableCandidateSessionsForSession: (
    sessionId: string,
  ) => Record<string, ModelBrainstormReusableCandidateSession>;
  removeBatchesForSession: (sessionId: string) => void;
  reset: () => void;
}

function normalizeBatch(batch: ModelBrainstormBatch): ModelBrainstormBatch {
  const selectedCandidateIds =
    batch.selectedCandidateIds ??
    (batch.selectedCandidateId ? [batch.selectedCandidateId] : []);

  return {
    ...batch,
    roomId: batch.roomId || batch.sourceSessionId,
    contextMode: batch.contextMode || 'independent',
    selectedCandidateIds,
    selectedCandidateId: selectedCandidateIds[0],
    publicSelections: batch.publicSelections ?? [],
  };
}

function getBatchSessionIds(batch: ModelBrainstormBatch): string[] {
  return [
    batch.sourceSessionId,
    ...batch.candidates
      .map(candidate => candidate.sessionId)
      .filter((sessionId): sessionId is string => Boolean(sessionId)),
  ];
}

function getConnectedSessionIds(
  batches: Record<string, ModelBrainstormBatch>,
  seedSessionId: string,
): Set<string> {
  const connectedSessionIds = new Set<string>([seedSessionId]);
  let changed = true;

  while (changed) {
    changed = false;
    for (const batch of Object.values(batches)) {
      const batchSessionIds = getBatchSessionIds(batch);
      if (!batchSessionIds.some(batchSessionId => connectedSessionIds.has(batchSessionId))) {
        continue;
      }

      for (const batchSessionId of batchSessionIds) {
        if (!connectedSessionIds.has(batchSessionId)) {
          connectedSessionIds.add(batchSessionId);
          changed = true;
        }
      }
    }
  }

  return connectedSessionIds;
}

function getRelatedBatches(
  batches: Record<string, ModelBrainstormBatch>,
  order: string[],
  sessionId: string,
): ModelBrainstormBatch[] {
  const seedSessionId = sessionId.trim();
  if (!seedSessionId) {
    return [];
  }

  const connectedSessionIds = getConnectedSessionIds(batches, seedSessionId);
  return order
    .map(batchId => batches[batchId])
    .filter((batch): batch is ModelBrainstormBatch => {
      if (!batch) {
        return false;
      }
      return getBatchSessionIds(batch).some(batchSessionId => connectedSessionIds.has(batchSessionId));
    });
}

export const useModelBrainstormStore = create<ModelBrainstormState>((set, get) => ({
  batches: {},
  order: [],

  createBatch: (batch) => set((state) => {
    const nextBatch = normalizeBatch(batch);
    return {
      batches: {
        ...state.batches,
        [nextBatch.id]: nextBatch,
      },
      order: state.order.includes(nextBatch.id)
        ? state.order
        : [...state.order, nextBatch.id],
    };
  }),

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

  setCandidateAnswer: (batchId, candidateId, answer, completedAt) => set((state) => {
    const batch = state.batches[batchId];
    const trimmedAnswer = answer.trim();
    if (!batch || !trimmedAnswer) {
      return state;
    }

    let changed = false;
    const candidates = batch.candidates.map(candidate => {
      if (candidate.id !== candidateId) {
        return candidate;
      }
      if (candidate.answer === trimmedAnswer && candidate.completedAt === completedAt) {
        return candidate;
      }
      changed = true;
      return {
        ...candidate,
        answer: trimmedAnswer,
        completedAt: completedAt ?? candidate.completedAt ?? Date.now(),
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
          selectedCandidateIds: [candidateId],
        },
      },
    };
  }),

  toggleCandidatePublicSelection: (batchId, candidateId, answer, selected) => set((state) => {
    const batch = state.batches[batchId];
    const candidate = batch?.candidates.find(item => item.id === candidateId);
    const trimmedAnswer = answer.trim();
    if (!batch || !candidate || !trimmedAnswer) {
      return state;
    }

    const currentSelectedIds = batch.selectedCandidateIds ?? (batch.selectedCandidateId ? [batch.selectedCandidateId] : []);
    const isSelected = currentSelectedIds.includes(candidateId);
    const shouldSelect = selected ?? !isSelected;
    const nextSelectedIds = shouldSelect
      ? [...currentSelectedIds.filter(id => id !== candidateId), candidateId]
      : currentSelectedIds.filter(id => id !== candidateId);
    const currentSelections = batch.publicSelections ?? [];
    const nextPublicSelections = shouldSelect
      ? [
          ...currentSelections.filter(selection => selection.candidateId !== candidateId),
          {
            id: `${batchId}:${candidateId}`,
            candidateId,
            modelId: candidate.modelId,
            modelLabel: candidate.modelLabel,
            answer: trimmedAnswer,
            createdAt: Date.now(),
          },
        ]
      : currentSelections.filter(selection => selection.candidateId !== candidateId);
    const candidates = batch.candidates.map(item => item.id === candidateId
      ? {
          ...item,
          answer: trimmedAnswer,
          completedAt: item.completedAt ?? Date.now(),
        }
      : item);

    return {
      batches: {
        ...state.batches,
        [batchId]: {
          ...batch,
          selectedCandidateId: nextSelectedIds[0],
          selectedCandidateIds: nextSelectedIds,
          publicSelections: nextPublicSelections,
          candidates,
        },
      },
    };
  }),

  getBatchesForSession: (sessionId) => {
    const state = get();
    return getRelatedBatches(state.batches, state.order, sessionId);
  },

  getReusableCandidateSessionsForSession: (sessionId) => {
    const state = get();
    const relatedBatches = getRelatedBatches(state.batches, state.order, sessionId);
    const reusableSessions: Record<string, ModelBrainstormReusableCandidateSession> = {};

    for (const batch of relatedBatches) {
      for (const candidate of batch.candidates) {
        if (!candidate.sessionId) {
          continue;
        }

        reusableSessions[candidate.modelId] = {
          sessionId: candidate.sessionId,
          batchId: batch.id,
          candidateId: candidate.id,
          modelId: candidate.modelId,
          modelLabel: candidate.modelLabel,
          createdAt: batch.createdAt,
        };
      }
    }

    return reusableSessions;
  },

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

    return getRelatedBatches(state.batches, state.order, sessionId)
      .filter(batch => batch.sourceSessionId === sessionId);
  }));
}
