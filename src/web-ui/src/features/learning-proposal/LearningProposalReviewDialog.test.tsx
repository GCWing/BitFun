import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import type { LearningProposal } from '@/infrastructure/api/service-api/LearningProposalAPI';

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    disabled,
  }: {
    children: React.ReactNode;
    disabled?: boolean;
  }) => <button disabled={disabled}>{children}</button>,
  Modal: ({ children, isOpen }: { children: React.ReactNode; isOpen: boolean }) => (
    isOpen ? <div role="dialog">{children}</div> : null
  ),
}));

vi.mock('@/flow_chat/components/InlineDiffPreview', () => ({
  InlineDiffPreview: () => <div data-testid="diff-preview" />,
}));

import { LearningProposalReviewDialog } from './LearningProposalReviewDialog';

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

function renderProposal(candidate: LearningProposal): string {
  return renderToStaticMarkup(
    <LearningProposalReviewDialog
      proposal={candidate}
      busyAction={null}
      onClose={() => {}}
      onRefresh={() => {}}
      onApprove={() => {}}
      onReject={() => {}}
    />,
  );
}

describe('LearningProposalReviewDialog', () => {
  it('shows approval and the diff only for a writable local memory proposal', () => {
    const html = renderProposal(proposal());

    expect(html).toContain('learningProposal.actions.approve');
    expect(html).toContain('learningProposal.actions.refresh');
    expect(html).toContain('data-testid="diff-preview"');
  });

  it('renders a skill proposal as suggestion-only without approval', () => {
    const html = renderProposal(proposal({
      target: {
        kind: 'skill',
        applyMode: 'read_only',
        displayName: 'Browser skill',
        identifier: 'browser:control-in-app-browser',
      },
    }));

    expect(html).not.toContain('learningProposal.actions.approve');
    expect(html).toContain('learningProposal.actions.requestReanalysis');
    expect(html).toContain('learningProposal.review.targetReadOnly');
    expect(html).toContain('<code>browser:control-in-app-browser</code>');
  });

  it('renders remote memory proposals as read-only', () => {
    const html = renderProposal(proposal({
      source: {
        ...proposal().source,
        remoteConnectionId: 'remote-1',
      },
    }));

    expect(html).not.toContain('learningProposal.actions.approve');
    expect(html).toContain('learningProposal.review.remoteReadOnly');
  });
});
