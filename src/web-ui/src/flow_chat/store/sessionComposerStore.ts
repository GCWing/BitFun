import { create } from 'zustand';

import type { ContextItem } from '@/shared/types/context';
import type { ComposerPresentation } from '../utils/composerPresentation';

export type PendingLargePasteMap = Record<string, string>;

export interface SessionComposerDraft {
  value: string;
  contexts: ContextItem[];
  presentation: ComposerPresentation | null;
  pendingLargePastes: PendingLargePasteMap;
  updatedAt: number;
}

interface SessionComposerState {
  drafts: Record<string, SessionComposerDraft>;
  getDraft: (sessionId: string) => SessionComposerDraft;
  activateDraft: (
    previousSessionId: string | null,
    nextSessionId: string | null,
    currentContexts: ContextItem[],
    currentPresentation: ComposerPresentation | null,
  ) => SessionComposerDraft;
  setValue: (sessionId: string, value: string) => void;
  setContexts: (sessionId: string, contexts: ContextItem[]) => void;
  setPresentation: (sessionId: string, presentation: ComposerPresentation | null) => void;
  setPendingLargePastes: (sessionId: string, pendingLargePastes: PendingLargePasteMap) => void;
  clearDraft: (sessionId: string) => void;
  removeDrafts: (sessionIds: Iterable<string>) => void;
}

const EMPTY_CONTEXTS: ContextItem[] = [];
const EMPTY_PENDING_LARGE_PASTES: PendingLargePasteMap = {};

function createEmptyDraft(): SessionComposerDraft {
  return {
    value: '',
    contexts: EMPTY_CONTEXTS,
    presentation: null,
    pendingLargePastes: EMPTY_PENDING_LARGE_PASTES,
    updatedAt: 0,
  };
}

function updateDraft(
  state: SessionComposerState,
  sessionId: string,
  update: Partial<Omit<SessionComposerDraft, 'updatedAt'>>,
): Pick<SessionComposerState, 'drafts'> {
  const current = state.drafts[sessionId] ?? createEmptyDraft();
  return {
    drafts: {
      ...state.drafts,
      [sessionId]: {
        ...current,
        ...update,
        updatedAt: Date.now(),
      },
    },
  };
}

export const useSessionComposerStore = create<SessionComposerState>((set, get) => ({
  drafts: {},

  getDraft: (sessionId) => get().drafts[sessionId] ?? createEmptyDraft(),

  activateDraft: (
    previousSessionId,
    nextSessionId,
    currentContexts,
    currentPresentation,
  ) => {
    if (previousSessionId && previousSessionId !== nextSessionId) {
      get().setContexts(previousSessionId, currentContexts);
      get().setPresentation(previousSessionId, currentPresentation);
    }
    return nextSessionId ? get().getDraft(nextSessionId) : createEmptyDraft();
  },

  setValue: (sessionId, value) => {
    set(state => updateDraft(state, sessionId, { value }));
  },

  setContexts: (sessionId, contexts) => {
    set(state => updateDraft(state, sessionId, { contexts: [...contexts] }));
  },

  setPresentation: (sessionId, presentation) => {
    set(state => updateDraft(state, sessionId, { presentation }));
  },

  setPendingLargePastes: (sessionId, pendingLargePastes) => {
    set(state => updateDraft(state, sessionId, {
      pendingLargePastes: { ...pendingLargePastes },
    }));
  },

  clearDraft: (sessionId) => {
    set(state => {
      if (!state.drafts[sessionId]) {
        return state;
      }

      return updateDraft(state, sessionId, {
        value: '',
        contexts: [],
        presentation: null,
        pendingLargePastes: {},
      });
    });
  },

  removeDrafts: (sessionIds) => {
    const ids = new Set(sessionIds);
    if (ids.size === 0) {
      return;
    }

    set(state => {
      const drafts = { ...state.drafts };
      let changed = false;
      ids.forEach(sessionId => {
        if (sessionId in drafts) {
          delete drafts[sessionId];
          changed = true;
        }
      });
      return changed ? { drafts } : state;
    });
  },
}));

export const sessionComposerStore = useSessionComposerStore;
