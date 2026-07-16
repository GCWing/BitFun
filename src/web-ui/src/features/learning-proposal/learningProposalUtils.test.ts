import { describe, expect, it } from 'vitest';
import type { LearningProposal } from '@/infrastructure/api/service-api/LearningProposalAPI';
import {
  canApplyLearningProposal,
  canShowLearningProposalApprove,
  learningProposalErrorMessage,
  learningProposalRequest,
} from './learningProposalUtils';

function proposal(overrides: Partial<LearningProposal> = {}): LearningProposal {
  return {
    schemaVersion: 1,
    proposalId: 'proposal-1',
    status: 'ready',
    source: {
      sessionId: 'session-1',
      workspacePath: 'C:\\workspace',
      selectedText: 'Important correction',
      turnId: 'turn-1',
      roundId: 'round-1',
      itemId: 'item-1',
      sourceKind: 'assistant_text',
    },
    target: {
      kind: 'memory',
      applyMode: 'memory_note',
      displayName: 'Workspace memory',
    },
    preview: {
      originalContent: 'before',
      proposedContent: 'after',
    },
    baseHash: 'base-hash',
    diffHash: 'diff-hash',
    createdAt: 1,
    updatedAt: 2,
    ...overrides,
  };
}

describe('learning proposal review policy', () => {
  it('allows approval only for a ready local memory note with both hashes', () => {
    const candidate = proposal();

    expect(canShowLearningProposalApprove(candidate)).toBe(true);
    expect(canApplyLearningProposal(candidate)).toBe(true);
    expect(canApplyLearningProposal(proposal({ status: 'stale' }))).toBe(false);
    expect(canApplyLearningProposal(proposal({ preview: undefined }))).toBe(false);
    expect(canApplyLearningProposal(proposal({ diffHash: undefined }))).toBe(false);
  });

  it('keeps skill and remote memory proposals read-only', () => {
    const skill = proposal({
      target: {
        kind: 'skill',
        applyMode: 'read_only',
        displayName: 'Browser skill',
      },
    });
    const remote = proposal({
      source: {
        ...proposal().source,
        remoteConnectionId: 'remote-1',
      },
    });

    expect(canShowLearningProposalApprove(skill)).toBe(false);
    expect(canApplyLearningProposal(skill)).toBe(false);
    expect(canShowLearningProposalApprove(remote)).toBe(false);
    expect(canApplyLearningProposal(remote)).toBe(false);
  });

  it('uses returned provenance for reload requests and typed errors', () => {
    const candidate = proposal({
      source: {
        ...proposal().source,
        remoteConnectionId: 'remote-1',
        remoteSshHost: 'build-host',
      },
      error: { code: 'target_read_only', message: 'Read-only target' },
    });

    expect(learningProposalRequest(candidate)).toEqual({
      proposalId: 'proposal-1',
      workspacePath: 'C:\\workspace',
      remoteConnectionId: 'remote-1',
      remoteSshHost: 'build-host',
    });
    expect(learningProposalErrorMessage(candidate)).toBe('Read-only target');
  });
});
