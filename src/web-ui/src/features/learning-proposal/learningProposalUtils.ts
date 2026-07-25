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
  if (!proposal.target) {
    return false;
  }
  if (proposal.target.applyMode === 'read_only') {
    return false;
  }
  if (proposal.target.applyMode === 'memory_note' && isRemoteLearningProposal(proposal)) {
    return false;
  }
  return true;
}

export function canApplyLearningProposal(proposal: LearningProposal): boolean {
  return canShowLearningProposalApprove(proposal)
    && proposal.status === 'ready'
    && Boolean(proposal.preview)
    && Boolean(proposal.baseHash)
    && Boolean(proposal.diffHash);
}

export function approveActionLabelKey(proposal: LearningProposal): string {
  return proposal.target?.applyMode === 'agent_patch'
    ? 'learningProposal.actions.applyViaAgent'
    : 'learningProposal.actions.approve';
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
