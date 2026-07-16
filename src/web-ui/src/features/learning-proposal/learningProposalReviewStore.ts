import { create } from 'zustand';
import type { LearningProposal } from '@/infrastructure/api/service-api/LearningProposalAPI';

export interface LearningProposalReviewRequest {
  proposalId: string;
  initialProposal?: LearningProposal;
  notificationId?: string;
}

interface LearningProposalReviewState {
  request: LearningProposalReviewRequest | null;
  open: (request: LearningProposalReviewRequest) => void;
  close: () => void;
}

export const useLearningProposalReviewStore = create<LearningProposalReviewState>((set) => ({
  request: null,
  open: (request) => set({ request }),
  close: () => set({ request: null }),
}));

export function openLearningProposalReview(request: LearningProposalReviewRequest): void {
  useLearningProposalReviewStore.getState().open(request);
}
