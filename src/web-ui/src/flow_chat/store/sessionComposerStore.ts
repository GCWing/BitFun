import { create } from 'zustand';

import type { ContextItem } from '@/shared/types/context';
import {
  getActiveSurfaceId,
  surfaceScopedKey,
  type DeviceSurfaceId,
} from '@/infrastructure/peer-device/deviceSurface';
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
    persistPreviousContexts?: boolean,
  ) => SessionComposerDraft;
  setValue: (sessionId: string, value: string) => void;
  setContexts: (sessionId: string, contexts: ContextItem[]) => void;
  setPresentation: (sessionId: string, presentation: ComposerPresentation | null) => void;
  setPendingLargePastes: (sessionId: string, pendingLargePastes: PendingLargePasteMap) => void;
  clearDraft: (sessionId: string) => void;
  removeDrafts: (sessionIds: Iterable<string>) => void;
  removeSurfaceDrafts: (surfaceId: DeviceSurfaceId) => void;
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

function draftKey(sessionId: string, surfaceId = getActiveSurfaceId()): string {
  return surfaceScopedKey(surfaceId, sessionId);
}

function updateDraft(
  state: SessionComposerState,
  sessionId: string,
  update: Partial<Omit<SessionComposerDraft, 'updatedAt'>>,
): Pick<SessionComposerState, 'drafts'> {
  const key = draftKey(sessionId);
  const current = state.drafts[key] ?? createEmptyDraft();
  return {
    drafts: {
      ...state.drafts,
      [key]: {
        ...current,
        ...update,
        updatedAt: Date.now(),
      },
    },
  };
}

export const useSessionComposerStore = create<SessionComposerState>((set, get) => ({
  drafts: {},

  getDraft: (sessionId) => get().drafts[draftKey(sessionId)] ?? createEmptyDraft(),

  activateDraft: (
    previousSessionId,
    nextSessionId,
    currentContexts,
    currentPresentation,
    persistPreviousContexts = true,
  ) => {
    if (
      persistPreviousContexts
      && previousSessionId
      && previousSessionId !== nextSessionId
    ) {
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
      if (!state.drafts[draftKey(sessionId)]) {
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
    const ids = new Set(Array.from(sessionIds, sessionId => draftKey(sessionId)));
    if (ids.size === 0) {
      return;
    }

    set(state => {
      const drafts = { ...state.drafts };
      let changed = false;
      ids.forEach(key => {
        if (key in drafts) {
          delete drafts[key];
          changed = true;
        }
      });
      return changed ? { drafts } : state;
    });
  },

  removeSurfaceDrafts: (surfaceId) => {
    set(state => {
      const drafts = { ...state.drafts };
      let changed = false;
      for (const key of Object.keys(drafts)) {
        try {
          const [ownedSurfaceId] = JSON.parse(key) as [DeviceSurfaceId, string];
          if (ownedSurfaceId === surfaceId) {
            delete drafts[key];
            changed = true;
          }
        } catch {
          // Ignore legacy/malformed in-memory keys; no persisted shape exists.
        }
      }
      return changed ? { drafts } : state;
    });
  },
}));

export const sessionComposerStore = useSessionComposerStore;
