import { AlertTriangle, Check, Loader2, RefreshCw, XCircle } from 'lucide-react';
import { Button, Modal } from '@/component-library';
import { InlineDiffPreview } from '@/flow_chat/components/InlineDiffPreview';
import { useI18n } from '@/infrastructure/i18n';
import type { LearningProposal } from '@/infrastructure/api/service-api/LearningProposalAPI';
import {
  canApplyLearningProposal,
  canShowLearningProposalApprove,
  isRemoteLearningProposal,
  learningProposalErrorMessage,
} from './learningProposalUtils';
import './LearningProposalReviewDialog.scss';

type Translate = (key: string, options?: Record<string, unknown>) => string;

function targetKindLabel(proposal: LearningProposal, t: Translate): string {
  switch (proposal.target?.kind) {
    case 'memory': return t('learningProposal.target.kind.memory');
    case 'skill': return t('learningProposal.target.kind.skill');
    case 'agents_md': return t('learningProposal.target.kind.agents_md');
    case 'none':
    default: return t('learningProposal.target.kind.none');
  }
}

function statusLabel(proposal: LearningProposal, t: Translate): string {
  switch (proposal.status) {
    case 'analyzing': return t('learningProposal.status.analyzing');
    case 'ready': return t('learningProposal.status.ready');
    case 'applying': return t('learningProposal.status.applying');
    case 'applied': return t('learningProposal.status.applied');
    case 'rejected': return t('learningProposal.status.rejected');
    case 'stale': return t('learningProposal.status.stale');
    case 'failed': return t('learningProposal.status.failed');
  }
}

function sourceKindLabel(proposal: LearningProposal, t: Translate): string {
  switch (proposal.source.sourceKind) {
    case 'user_message': return t('learningProposal.sourceKind.user_message');
    case 'assistant_text': return t('learningProposal.sourceKind.assistant_text');
    case 'assistant_thinking': return t('learningProposal.sourceKind.assistant_thinking');
    case 'tool': return t('learningProposal.sourceKind.tool');
    case 'unknown': return t('learningProposal.sourceKind.unknown');
  }
}

export type LearningProposalReviewBusyAction =
  | 'loading'
  | 'refreshing'
  | 'approving'
  | 'rejecting'
  | null;

interface LearningProposalReviewDialogProps {
  proposal: LearningProposal | null;
  busyAction: LearningProposalReviewBusyAction;
  clientError?: string;
  onClose: () => void;
  onRefresh: () => void;
  onApprove: () => void;
  onReject: () => void;
}

export function LearningProposalReviewDialog({
  proposal,
  busyAction,
  clientError,
  onClose,
  onRefresh,
  onApprove,
  onReject,
}: LearningProposalReviewDialogProps) {
  const { t } = useI18n('flow-chat');
  const isBusy = busyAction !== null;
  const showApprove = proposal ? canShowLearningProposalApprove(proposal) : false;
  const canApprove = proposal ? canApplyLearningProposal(proposal) : false;
  const isResolved = proposal?.status === 'applied' || proposal?.status === 'rejected';
  const backendError = proposal ? learningProposalErrorMessage(proposal) : undefined;
  const readOnlyReason = proposal && !showApprove
    ? (isRemoteLearningProposal(proposal)
      ? t('learningProposal.review.remoteReadOnly')
      : t('learningProposal.review.targetReadOnly'))
    : undefined;

  return (
    <Modal
      isOpen={true}
      onClose={isBusy ? () => {} : onClose}
      title={t('learningProposal.review.title')}
      ariaLabel={t('learningProposal.review.title')}
      size="large"
      closeOnOverlayClick={!isBusy}
      contentClassName="learning-proposal-review-dialog__modal"
      testId="learning-proposal-review-dialog"
    >
      {!proposal ? (
        <div className="learning-proposal-review-dialog__loading" role="status">
          <Loader2 size={18} className="learning-proposal-review-dialog__spinner" aria-hidden="true" />
          <span>{clientError || t('learningProposal.review.loading')}</span>
        </div>
      ) : (
        <div className="learning-proposal-review-dialog" data-proposal-status={proposal.status}>
          <div className="learning-proposal-review-dialog__summary">
            <div className="learning-proposal-review-dialog__target">
              <span className="learning-proposal-review-dialog__eyebrow">
                {targetKindLabel(proposal, t)}
              </span>
              <strong>{proposal.target?.displayName || t('learningProposal.target.pending')}</strong>
              {(proposal.target?.filePath || proposal.target?.identifier) && (
                <code>{proposal.target.filePath || proposal.target.identifier}</code>
              )}
            </div>
            <span
              className={`learning-proposal-review-dialog__status learning-proposal-review-dialog__status--${proposal.status}`}
            >
              {statusLabel(proposal, t)}
            </span>
          </div>

          {(readOnlyReason || proposal.status === 'stale') && (
            <div className="learning-proposal-review-dialog__notice" role="note">
              <AlertTriangle size={15} aria-hidden="true" />
              <span>
                {proposal.status === 'stale'
                  ? t('learningProposal.review.staleNotice')
                  : readOnlyReason}
              </span>
            </div>
          )}

          {(backendError || clientError) && (
            <div className="learning-proposal-review-dialog__error" role="alert">
              {backendError || clientError}
            </div>
          )}

          <section>
            <h3>{t('learningProposal.review.sourceTitle')}</h3>
            <blockquote>{proposal.source.selectedText}</blockquote>
            <dl className="learning-proposal-review-dialog__provenance">
              <div>
                <dt>{t('learningProposal.review.sourceKind')}</dt>
                <dd>{sourceKindLabel(proposal, t)}</dd>
              </div>
              <div>
                <dt>{t('learningProposal.review.turn')}</dt>
                <dd><code>{proposal.source.turnId}</code></dd>
              </div>
              {proposal.source.roundId && (
                <div>
                  <dt>{t('learningProposal.review.round')}</dt>
                  <dd><code>{proposal.source.roundId}</code></dd>
                </div>
              )}
              {proposal.source.itemId && (
                <div>
                  <dt>{t('learningProposal.review.item')}</dt>
                  <dd><code>{proposal.source.itemId}</code></dd>
                </div>
              )}
            </dl>
          </section>

          {(proposal.rationale || proposal.futureUse) && (
            <section className="learning-proposal-review-dialog__analysis">
              {proposal.rationale && (
                <div>
                  <h3>{t('learningProposal.review.rationaleTitle')}</h3>
                  <p>{proposal.rationale}</p>
                </div>
              )}
              {proposal.futureUse && (
                <div>
                  <h3>{t('learningProposal.review.futureUseTitle')}</h3>
                  <p>{proposal.futureUse}</p>
                </div>
              )}
            </section>
          )}

          {proposal.preview && (
            <section className="learning-proposal-review-dialog__preview">
              <h3>{t('learningProposal.review.previewTitle')}</h3>
              <InlineDiffPreview
                originalContent={proposal.preview.originalContent}
                modifiedContent={proposal.preview.proposedContent}
                filePath={proposal.preview.filePath || proposal.target?.filePath}
                maxHeight={320}
                contextLines={4}
              />
            </section>
          )}

          <div className="learning-proposal-review-dialog__actions">
            <Button type="button" variant="ghost" size="small" onClick={onClose} disabled={isBusy}>
              {t('learningProposal.actions.close')}
            </Button>
            {!isResolved && (
              <Button
                type="button"
                variant="ghost"
                size="small"
                onClick={onReject}
                isLoading={busyAction === 'rejecting'}
                disabled={isBusy}
              >
                <XCircle size={14} aria-hidden="true" />
                {t('learningProposal.actions.reject')}
              </Button>
            )}
            {!isResolved && (
              <Button
                type="button"
                variant={showApprove ? 'secondary' : 'primary'}
                size="small"
                onClick={onRefresh}
                isLoading={busyAction === 'refreshing'}
                disabled={isBusy}
              >
                <RefreshCw size={14} aria-hidden="true" />
                {showApprove
                  ? t('learningProposal.actions.refresh')
                  : t('learningProposal.actions.requestReanalysis')}
              </Button>
            )}
            {showApprove && !isResolved && (
              <Button
                type="button"
                variant="primary"
                size="small"
                onClick={onApprove}
                isLoading={busyAction === 'approving'}
                disabled={isBusy || !canApprove}
              >
                <Check size={14} aria-hidden="true" />
                {t('learningProposal.actions.approve')}
              </Button>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}
