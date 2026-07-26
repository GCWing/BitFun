/**
 * Mini App scene store — app catalog + lifecycle state.
 */
import { create } from 'zustand';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';

/**
 * Window event dispatched by the floating session bubble when a MiniApp has
 * claimed its composer (`app.chat.claimComposer`). Detail:
 * `{ appId, token, text }`. `useMiniAppBridge` listens for it and forwards the
 * text into the MiniApp iframe as a 'chat:userMessage' event.
 */
export const MINIAPP_COMPOSER_MESSAGE_EVENT = 'miniapp-composer-message';

/**
 * Window event asking the bubble to open and prefill its composer without
 * sending (`app.chat.setComposerDraft`). Detail: `{ token, text }`.
 */
export const MINIAPP_COMPOSER_DRAFT_EVENT = 'miniapp-composer-draft';

/** A MiniApp's claim on the floating bubble composer (`app.chat.claimComposer`). */
export interface MiniAppComposerClaim {
  /**
   * Identifies the exact iframe holding the claim. One app ID can have several
   * live runners at once — the installed app and its draft preview are mounted
   * side by side during AI customization — so messages and releases are keyed
   * by token, not by app ID, or one bubble message would start a run in both.
   */
  token: string;
  /** Placeholder shown in the bubble composer while this MiniApp is active. */
  placeholder?: string;
}

interface MiniAppState {
  apps: MiniAppMeta[];
  loading: boolean;
  /** App IDs whose scenes are currently open in the viewport. */
  openedAppIds: string[];
  /** App IDs whose JS workers are currently running. */
  runningWorkerIds: string[];
  /** App IDs with an active customization surface in the MiniApp tab. */
  customizingAppIds: string[];
  /** Floating bubble composer claims, keyed by app ID (`app.chat.claimComposer`). */
  composerClaims: Record<string, MiniAppComposerClaim>;

  setApps: (apps: MiniAppMeta[]) => void;
  setLoading: (loading: boolean) => void;
  openApp: (id: string) => void;
  closeApp: (id: string) => void;
  setRunningWorkerIds: (ids: string[]) => void;
  markWorkerRunning: (id: string) => void;
  markWorkerStopped: (id: string) => void;
  markCustomizationActive: (id: string) => void;
  markCustomizationIdle: (id: string) => void;
  claimComposer: (id: string, claim: MiniAppComposerClaim) => void;
  /** Releases only if `token` still holds the claim; omit to force-release. */
  releaseComposer: (id: string, token?: string) => void;
}

export const useMiniAppStore = create<MiniAppState>((set) => ({
  apps: [],
  loading: false,
  openedAppIds: [],
  runningWorkerIds: [],
  customizingAppIds: [],
  composerClaims: {},

  setApps: (apps) =>
    set((state) => {
      const validIds = new Set(apps.map((app) => app.id));
      return {
        apps,
        openedAppIds: state.openedAppIds.filter((id) => validIds.has(id)),
        runningWorkerIds: state.runningWorkerIds.filter((id) => validIds.has(id)),
        customizingAppIds: state.customizingAppIds.filter((id) => validIds.has(id)),
        composerClaims: Object.fromEntries(
          Object.entries(state.composerClaims).filter(([id]) => validIds.has(id))
        ),
      };
    }),
  setLoading: (loading) => set({ loading }),

  openApp: (id) =>
    set((state) =>
      state.openedAppIds.includes(id) ? state : { openedAppIds: [...state.openedAppIds, id] }
    ),
  closeApp: (id) =>
    set((state) => {
      const { [id]: _removed, ...composerClaims } = state.composerClaims;
      return {
        openedAppIds: state.openedAppIds.filter((value) => value !== id),
        composerClaims,
      };
    }),
  setRunningWorkerIds: (ids) => set({ runningWorkerIds: Array.from(new Set(ids)) }),
  markWorkerRunning: (id) =>
    set((state) =>
      state.runningWorkerIds.includes(id) ? state : { runningWorkerIds: [...state.runningWorkerIds, id] }
    ),
  markWorkerStopped: (id) =>
    set((state) => ({
      runningWorkerIds: state.runningWorkerIds.filter((value) => value !== id),
    })),
  markCustomizationActive: (id) =>
    set((state) =>
      state.customizingAppIds.includes(id) ? state : { customizingAppIds: [...state.customizingAppIds, id] }
    ),
  markCustomizationIdle: (id) =>
    set((state) => ({
      customizingAppIds: state.customizingAppIds.filter((value) => value !== id),
    })),
  claimComposer: (id, claim) =>
    set((state) => ({
      composerClaims: { ...state.composerClaims, [id]: claim },
    })),
  releaseComposer: (id, token) =>
    set((state) => {
      const current = state.composerClaims[id];
      if (!current) return state;
      // A runner that lost the claim to another runner of the same app must not
      // release it on its way out.
      if (token !== undefined && current.token !== token) return state;
      const { [id]: _removed, ...composerClaims } = state.composerClaims;
      return { composerClaims };
    }),
}));
