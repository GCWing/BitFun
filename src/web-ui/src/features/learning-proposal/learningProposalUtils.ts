import type {
  GetLearningProposalRequest,
  LearningProposal,
} from '@/infrastructure/api/service-api/LearningProposalAPI';

export function learningProposalErrorMessage(proposal: LearningProposal): string | undefined {
  return proposal.error?.message;
}

export function isRemoteLearningProposal(proposal: LearningProposal): boolean {
  return Boolean(proposal.source.remoteConnectionId || proposal.source.remoteSshHost);
}

export function canShowLearningProposalApprove(proposal: LearningProposal): boolean {
  return proposal.target?.kind === 'memory'
    && proposal.target.applyMode === 'memory_note'
    && !isRemoteLearningProposal(proposal);
}

export function canApplyLearningProposal(proposal: LearningProposal): boolean {
  return canShowLearningProposalApprove(proposal)
    && proposal.status === 'ready'
    && Boolean(proposal.preview)
    && Boolean(proposal.baseHash)
    && Boolean(proposal.diffHash);
}

export function learningProposalRequest(
  proposal: LearningProposal,
): GetLearningProposalRequest {
  return {
    proposalId: proposal.proposalId,
    workspacePath: proposal.source.workspacePath,
    remoteConnectionId: proposal.source.remoteConnectionId,
    remoteSshHost: proposal.source.remoteSshHost,
  };
}
