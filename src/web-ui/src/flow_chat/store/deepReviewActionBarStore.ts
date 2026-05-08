/**
 * Shared review action bar state rendered at the bottom of BtwSessionPanel.
 *
 * The legacy DeepReview exports are intentionally kept as aliases so existing
 * callers can migrate incrementally while standard Code Review starts using
 * the same confirmation surface.
 */

import { create } from 'zustand';
import type {
  CodeReviewRemediationData,
  ReviewRemediationItem,
} from '../utils/codeReviewRemediation';
import {
  buildReviewRemediationItems,
  getDefaultSelectedRemediationIds,
} from '../utils/codeReviewRemediation';
import type { RemediationGroupId } from '../utils/codeReviewReport';
import type { DeepReviewInterruption } from '../utils/deepReviewContinuation';

export type ReviewActionMode = 'standard' | 'deep';

export type ReviewActionPhase =
  | 'idle'
  | 'review_completed'
  | 'fix_running'
  | 'fix_completed'
  | 'fix_failed'
  | 'fix_timeout'
  | 'fix_interrupted'
  | 'review_interrupted'
  | 'resume_blocked'
  | 'resume_running'
  | 'resume_failed'
  | 'review_error';

export type DeepReviewActionPhase = ReviewActionPhase;

export interface ReviewActionBarState {
  /** Which child session this bar belongs to */
  childSessionId: string | null;
  /** Parent session (used to fill-back the input) */
  parentSessionId: string | null;
  /** Which review mode owns this action bar */
  reviewMode: ReviewActionMode;
  /** Current phase of the review lifecycle */
  phase: ReviewActionPhase;
  /** The raw review result data (remediation plan, issues, etc.) */
  reviewData: CodeReviewRemediationData | null;
  /** Pre-built remediation items derived from reviewData */
  remediationItems: ReviewRemediationItem[];
  /** IDs of the remediation items the user selected */
  selectedRemediationIds: Set<string>;
  /** Whether the action bar was dismissed by the user */
  dismissed: boolean;
  /** Whether the action bar is minimized (collapsed to a floating button) */
  minimized: boolean;
  /** Which fix action is currently in flight */
  activeAction: 'fix' | 'fix-review' | 'resume' | null;
  /** Last user action that changed the action bar content */
  lastSubmittedAction: 'fix' | 'fix-review' | 'resume' | null;
  /** User-supplied custom instructions (from the textarea) */
  customInstructions: string;
  /** Error message when phase is fix_failed or review_error */
  errorMessage: string | null;
  /** Structured interruption state used to continue an incomplete Deep Review */
  interruption: DeepReviewInterruption | null;
  /** IDs of remediation items that have been fixed/completed */
  completedRemediationIds: Set<string>;
  /** IDs of items being fixed in the current fix_running session (snapshot at start) */
  fixingRemediationIds: Set<string>;
  /** IDs of items remaining when a fix was interrupted */
  remainingFixIds: string[];
  /** User's option choice for needs_decision items: map of item id -> option index */
  decisionSelections: Record<string, number>;

  // ---- actions ----
  showActionBar: (params: {
    childSessionId: string;
    parentSessionId: string | null;
    reviewData: CodeReviewRemediationData;
    reviewMode?: ReviewActionMode;
    phase?: ReviewActionPhase;
    completedRemediationIds?: Set<string>;
  }) => void;
  showInterruptedActionBar: (params: {
    childSessionId: string;
    parentSessionId: string | null;
    interruption: DeepReviewInterruption;
    phase?: Extract<ReviewActionPhase, 'review_interrupted' | 'resume_blocked' | 'resume_failed'>;
  }) => void;
  updatePhase: (phase: ReviewActionPhase, errorMessage?: string | null) => void;
  toggleRemediation: (id: string) => void;
  toggleAllRemediation: () => void;
  toggleGroupRemediation: (groupId: RemediationGroupId) => void;
  setActiveAction: (action: 'fix' | 'fix-review' | 'resume' | null) => void;
  setCustomInstructions: (value: string) => void;
  setSelectedRemediationIds: (ids: Set<string>) => void;
  dismiss: () => void;
  minimize: () => void;
  restore: () => void;
  skipRemainingFixes: () => void;
  setDecisionSelection: (itemId: string, optionIndex: number) => void;
  reset: () => void;
}

export type DeepReviewActionBarState = ReviewActionBarState;

const initialState = {
  childSessionId: null as string | null,
  parentSessionId: null as string | null,
  reviewMode: 'deep' as ReviewActionMode,
  phase: 'idle' as ReviewActionPhase,
  reviewData: null as CodeReviewRemediationData | null,
  remediationItems: [] as ReviewRemediationItem[],
  selectedRemediationIds: new Set<string>(),
  dismissed: false,
  minimized: false,
  activeAction: null as 'fix' | 'fix-review' | 'resume' | null,
  lastSubmittedAction: null as 'fix' | 'fix-review' | 'resume' | null,
  customInstructions: '',
  errorMessage: null as string | null,
  interruption: null as DeepReviewInterruption | null,
  completedRemediationIds: new Set<string>(),
  fixingRemediationIds: new Set<string>(),
  remainingFixIds: [] as string[],
  decisionSelections: {} as Record<string, number>,
};

export const useReviewActionBarStore = create<ReviewActionBarState>((set, get) => ({
  ...initialState,

  showActionBar: ({ childSessionId, parentSessionId, reviewData, reviewMode, phase, completedRemediationIds }) => {
    const items = buildReviewRemediationItems(reviewData);
    const defaultIds = new Set(getDefaultSelectedRemediationIds(items));

    // If completedRemediationIds is provided, filter out items that no longer exist
    const existingIds = new Set(items.map((i) => i.id));
    const preservedCompleted = completedRemediationIds
      ? new Set([...completedRemediationIds].filter((id) => existingIds.has(id)))
      : new Set<string>();

    // Remove completed items from default selection
    for (const id of preservedCompleted) {
      defaultIds.delete(id);
    }

    set({
      childSessionId,
      parentSessionId,
      reviewMode: reviewMode ?? reviewData.review_mode ?? 'deep',
      reviewData,
      remediationItems: items,
      selectedRemediationIds: defaultIds,
      phase: phase ?? 'review_completed',
      dismissed: false,
      minimized: false,
      activeAction: null,
      lastSubmittedAction: null,
      customInstructions: '',
      errorMessage: null,
      interruption: null,
      completedRemediationIds: preservedCompleted,
      fixingRemediationIds: new Set(),
      remainingFixIds: [],
      decisionSelections: {},
    });
  },

  showInterruptedActionBar: ({ childSessionId, parentSessionId, interruption, phase }) => {
    set({
      childSessionId,
      parentSessionId,
      reviewMode: 'deep',
      reviewData: null,
      remediationItems: [],
      selectedRemediationIds: new Set(),
      phase: phase ?? interruption.phase,
      dismissed: false,
      minimized: false,
      activeAction: null,
      lastSubmittedAction: null,
      customInstructions: '',
      errorMessage: null,
      interruption,
      completedRemediationIds: new Set(),
      fixingRemediationIds: new Set(),
      remainingFixIds: [],
      decisionSelections: {},
    });
  },

  updatePhase: (phase, errorMessage) => {
    const prevPhase = get().phase;
    if (prevPhase === 'fix_running' && phase === 'fix_completed') {
      const { fixingRemediationIds, completedRemediationIds } = get();
      const nextCompleted = new Set(completedRemediationIds);
      for (const id of fixingRemediationIds) {
        nextCompleted.add(id);
      }
      set({
        phase,
        errorMessage: errorMessage ?? null,
        completedRemediationIds: nextCompleted,
        fixingRemediationIds: new Set(),
        remainingFixIds: [],
      });
    } else {
      set({ phase, errorMessage: errorMessage ?? null });
    }
  },

  toggleRemediation: (id) => {
    const next = new Set(get().selectedRemediationIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    set({ selectedRemediationIds: next });
  },

  toggleAllRemediation: () => {
    const { remediationItems, selectedRemediationIds } = get();
    const allSelected = remediationItems.length > 0 &&
      selectedRemediationIds.size === remediationItems.length;
    if (allSelected) {
      set({ selectedRemediationIds: new Set() });
    } else {
      set({ selectedRemediationIds: new Set(remediationItems.map((i) => i.id)) });
    }
  },

  toggleGroupRemediation: (groupId) => {
    const { remediationItems, selectedRemediationIds } = get();
    const groupIds = new Set(remediationItems.filter((i) => i.groupId === groupId).map((i) => i.id));
    if (groupIds.size === 0) return;

    const allGroupSelected = [...groupIds].every((id) => selectedRemediationIds.has(id));
    const next = new Set(selectedRemediationIds);

    if (allGroupSelected) {
      for (const id of groupIds) {
        next.delete(id);
      }
    } else {
      for (const id of groupIds) {
        next.add(id);
      }
    }

    set({ selectedRemediationIds: next });
  },

  setActiveAction: (action) => {
    if (action === 'fix' || action === 'fix-review') {
      set({
        activeAction: action,
        lastSubmittedAction: action,
        fixingRemediationIds: new Set(get().selectedRemediationIds),
      });
    } else if (action === 'resume') {
      set({
        activeAction: action,
        lastSubmittedAction: action,
      });
    } else {
      set({ activeAction: action });
    }
  },
  setCustomInstructions: (value) => set({ customInstructions: value }),
  setSelectedRemediationIds: (ids) => set({ selectedRemediationIds: ids }),
  dismiss: () => set({ dismissed: true }),
  minimize: () => set({ minimized: true }),
  restore: () => set({ minimized: false }),
  setDecisionSelection: (itemId, optionIndex) =>
    set((state) => ({
      decisionSelections: { ...state.decisionSelections, [itemId]: optionIndex },
    })),
  skipRemainingFixes: () => set({
    phase: 'review_completed',
    remainingFixIds: [],
    activeAction: null,
    lastSubmittedAction: null,
  }),
  reset: () => set({ ...initialState, selectedRemediationIds: new Set() }),
}));

// Subscribe to state changes and persist when relevant fields change
let persistTimer: ReturnType<typeof setTimeout> | null = null;
const PERSIST_DEBOUNCE_MS = 1000;

useReviewActionBarStore.subscribe((state, prevState) => {
  if (!state.childSessionId) return;

  const shouldPersist =
    state.phase !== prevState.phase ||
    state.minimized !== prevState.minimized ||
    state.completedRemediationIds !== prevState.completedRemediationIds ||
    state.customInstructions !== prevState.customInstructions;

  if (!shouldPersist) return;

  if (persistTimer) clearTimeout(persistTimer);

  persistTimer = setTimeout(() => {
    import('../services/ReviewActionBarPersistenceService').then(({ persistReviewActionState }) => {
      persistReviewActionState(state).catch(() => {
        // Silently ignore persistence errors
      });
    });
  }, PERSIST_DEBOUNCE_MS);
});

export const useDeepReviewActionBarStore = useReviewActionBarStore;
