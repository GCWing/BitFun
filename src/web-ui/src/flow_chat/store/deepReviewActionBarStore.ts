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
import type { DeepReviewInterruption } from '../utils/deepReviewContinuation';

export type ReviewActionMode = 'standard' | 'deep';

export type ReviewActionPhase =
  | 'idle'
  | 'review_completed'
  | 'fix_running'
  | 'fix_completed'
  | 'fix_failed'
  | 'fix_timeout'
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
  /** Which fix action is currently in flight */
  activeAction: 'fix' | 'fix-review' | 'resume' | null;
  /** User-supplied custom instructions (from the textarea) */
  customInstructions: string;
  /** Error message when phase is fix_failed or review_error */
  errorMessage: string | null;
  /** Structured interruption state used to continue an incomplete Deep Review */
  interruption: DeepReviewInterruption | null;

  // ---- actions ----
  showActionBar: (params: {
    childSessionId: string;
    parentSessionId: string | null;
    reviewData: CodeReviewRemediationData;
    reviewMode?: ReviewActionMode;
    phase?: ReviewActionPhase;
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
  setActiveAction: (action: 'fix' | 'fix-review' | 'resume' | null) => void;
  setCustomInstructions: (value: string) => void;
  dismiss: () => void;
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
  activeAction: null as 'fix' | 'fix-review' | 'resume' | null,
  customInstructions: '',
  errorMessage: null as string | null,
  interruption: null as DeepReviewInterruption | null,
};

export const useReviewActionBarStore = create<ReviewActionBarState>((set, get) => ({
  ...initialState,

  showActionBar: ({ childSessionId, parentSessionId, reviewData, reviewMode, phase }) => {
    const items = buildReviewRemediationItems(reviewData);
    const defaultIds = new Set(getDefaultSelectedRemediationIds(items));
    set({
      childSessionId,
      parentSessionId,
      reviewMode: reviewMode ?? reviewData.review_mode ?? 'deep',
      reviewData,
      remediationItems: items,
      selectedRemediationIds: defaultIds,
      phase: phase ?? 'review_completed',
      dismissed: false,
      activeAction: null,
      customInstructions: '',
      errorMessage: null,
      interruption: null,
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
      activeAction: null,
      customInstructions: '',
      errorMessage: null,
      interruption,
    });
  },

  updatePhase: (phase, errorMessage) => {
    set({ phase, errorMessage: errorMessage ?? null });
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

  setActiveAction: (action) => set({ activeAction: action }),
  setCustomInstructions: (value) => set({ customInstructions: value }),
  dismiss: () => set({ dismissed: true }),
  reset: () => set({ ...initialState, selectedRemediationIds: new Set() }),
}));

export const useDeepReviewActionBarStore = useReviewActionBarStore;
