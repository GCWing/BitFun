import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('./ApiClient', () => ({
  api: { invoke: invokeMock },
}));

import { LearningProposalAPI, type LearningProposal } from './LearningProposalAPI';

function proposal(): LearningProposal {
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
  };
}

describe('LearningProposalAPI', () => {
  const api = new LearningProposalAPI();

  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(proposal());
  });

  it('creates a proposal through a structured request payload', async () => {
    const request = {
      sessionId: 'session-1',
      workspacePath: 'C:\\workspace',
      source: {
        selectedText: 'Important correction',
        turnId: 'turn-1',
        sourceKind: 'assistant_text' as const,
      },
    };

    await api.create(request);

    expect(invokeMock).toHaveBeenCalledWith('create_learning_proposal', { request });
  });

  it('binds approval to both the base and diff hashes', async () => {
    const request = {
      proposalId: 'proposal-1',
      workspacePath: 'C:\\workspace',
      baseHash: 'base-hash',
      diffHash: 'diff-hash',
    };

    await api.approve(request);

    expect(invokeMock).toHaveBeenCalledWith('approve_learning_proposal', { request });
  });

  it('lists unresolved proposals through the backend truth source', async () => {
    invokeMock.mockResolvedValueOnce([proposal()]);

    await expect(api.list({ includeResolved: false })).resolves.toHaveLength(1);
    expect(invokeMock).toHaveBeenCalledWith('list_learning_proposals', {
      request: { includeResolved: false },
    });
  });
});
