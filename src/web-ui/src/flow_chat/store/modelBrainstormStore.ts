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

export interface ModelBrainstormPublicContext {
  sessionId: string;
  batchId: string;
  candidateId: string;
  modelId: string;
  modelLabel: string;
  answer: string;
  createdAt: number;
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
  publicContexts: Record<string, ModelBrainstormPublicContext>;
  createBatch: (batch: ModelBrainstormBatch) => void;
  updateCandidate: (
    batchId: string,
    candidateId: string,
    updates: Partial<ModelBrainstormCandidate>,
  ) => void;
  selectCandidate: (batchId: string, candidateId: string) => void;
  setPublicContextForSession: (
    sessionId: string,
    context: ModelBrainstormPublicContext,
  ) => void;
  getPublicContextForSession: (sessionId: string) => ModelBrainstormPublicContext | undefined;
  consumePublicContextForSession: (sessionId: string) => ModelBrainstormPublicContext | undefined;
  getReusableCandidateSessionsForSession: (
    sessionId: string,
  ) => Record<string, ModelBrainstormReusableCandidateSession>;
  removeBatchesForSession: (sessionId: string) => void;
  reset: () => void;
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

function findPublicContextEntry(
  state: Pick<ModelBrainstormState, 'batches' | 'publicContexts'>,
  sessionId: string,
): [string, ModelBrainstormPublicContext] | undefined {
  const directContext = state.publicContexts[sessionId];
  if (directContext) {
    return [sessionId, directContext];
  }

  const connectedSessionIds = getConnectedSessionIds(state.batches, sessionId);
  let latestEntry: [string, ModelBrainstormPublicContext] | undefined;
  for (const [publicContextSessionId, publicContext] of Object.entries(state.publicContexts)) {
    if (!connectedSessionIds.has(publicContextSessionId)) {
      continue;
    }

    if (!latestEntry || publicContext.createdAt > latestEntry[1].createdAt) {
      latestEntry = [publicContextSessionId, publicContext];
    }
  }

  return latestEntry;
}

export const useModelBrainstormStore = create<ModelBrainstormState>((set, get) => ({
  batches: {},
  order: [],
  publicContexts: {},

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

  setPublicContextForSession: (sessionId, context) => set((state) => ({
    publicContexts: {
      ...state.publicContexts,
      [sessionId]: context,
    },
  })),

  getPublicContextForSession: (sessionId) => {
    const seedSessionId = sessionId.trim();
    if (!seedSessionId) {
      return undefined;
    }

    return findPublicContextEntry(get(), seedSessionId)?.[1];
  },

  consumePublicContextForSession: (sessionId) => {
    const seedSessionId = sessionId.trim();
    if (!seedSessionId) {
      return undefined;
    }

    const entry = findPublicContextEntry(get(), seedSessionId);
    if (!entry) {
      return undefined;
    }

    const [publicContextSessionId, context] = entry;
    set((state) => {
      if (!state.publicContexts[publicContextSessionId]) {
        return state;
      }

      const nextPublicContexts = { ...state.publicContexts };
      delete nextPublicContexts[publicContextSessionId];

      return {
        publicContexts: nextPublicContexts,
      };
    });

    return context;
  },

  getReusableCandidateSessionsForSession: (sessionId) => {
    const seedSessionId = sessionId.trim();
    if (!seedSessionId) {
      return {};
    }

    const state = get();
    const connectedSessionIds = getConnectedSessionIds(state.batches, seedSessionId);

    const reusableSessions: Record<string, ModelBrainstormReusableCandidateSession> = {};
    for (const batchId of state.order) {
      const batch = state.batches[batchId];
      if (!batch) {
        continue;
      }

      const batchSessionIds = getBatchSessionIds(batch);
      const isRelatedBatch = batchSessionIds.some(batchSessionId => connectedSessionIds.has(batchSessionId));
      if (!isRelatedBatch) {
        continue;
      }

      for (const candidate of batch.candidates) {
        if (!candidate.sessionId || !connectedSessionIds.has(candidate.sessionId)) {
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
    const nextPublicContexts = { ...state.publicContexts };
    let removedPublicContext = false;
    if (nextPublicContexts[sessionId]) {
      delete nextPublicContexts[sessionId];
      removedPublicContext = true;
    }

    const nextOrder = state.order.filter(batchId => {
      const batch = state.batches[batchId];
      if (batch?.sourceSessionId !== sessionId) {
        return true;
      }
      delete nextBatches[batchId];
      return false;
    });

    if (nextOrder.length === state.order.length && !removedPublicContext) {
      return state;
    }

    return {
      batches: nextBatches,
      order: nextOrder,
      publicContexts: nextPublicContexts,
    };
  }),

  reset: () => set({ batches: {}, order: [], publicContexts: {} }),
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
