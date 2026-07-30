import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Alert, Button, Modal, confirmWarning } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import { FileDiff, FilePlus, FileX, Loader2 } from 'lucide-react';
import { dispatchApi } from './dispatchApi';
import type {
  DispatchResultApplyOutcome,
  DispatchResultBundle,
} from './types';
import './DispatchResultDialog.scss';

const log = createLogger('DispatchResultDialog');
const DIALOG_TITLE_ID = 'dispatch-result-dialog-title';

interface DispatchResultDialogProps {
  open: boolean;
  jobId: string;
  /** Local workspace an applied bundle would be written into. */
  workspacePath: string;
  targetLabel?: string;
  onClose: () => void;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Review what a finished dispatch job changed on its target, then decide
 * whether any of it reaches the local workspace.
 *
 * The target and the local tree diverged independently after the snapshot, so
 * nothing is written until the user has seen the list and said so. When a path
 * moved on both sides the apply aborts rather than picking a winner.
 */
export const DispatchResultDialog: React.FC<DispatchResultDialogProps> = ({
  open,
  jobId,
  workspacePath,
  targetLabel,
  onClose,
}) => {
  const { t } = useI18n('common');
  const [bundle, setBundle] = useState<DispatchResultBundle | null>(null);
  const [outcome, setOutcome] = useState<DispatchResultApplyOutcome | null>(null);
  const [pulling, setPulling] = useState(false);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Discards results of requests that resolve after the dialog moved on.
  const generationRef = useRef(0);

  useEffect(() => {
    if (!open) {
      generationRef.current += 1;
      setBundle(null);
      setOutcome(null);
      setError(null);
      setPulling(false);
      setApplying(false);
    }
  }, [open]);

  const pull = useCallback(async () => {
    if (!jobId) return;
    const generation = ++generationRef.current;
    setPulling(true);
    setError(null);
    setOutcome(null);
    try {
      const result = await dispatchApi.pullResult(jobId);
      if (generation !== generationRef.current) return;
      setBundle(result);
    } catch (nextError) {
      if (generation !== generationRef.current) return;
      setError(errorMessage(nextError));
      log.warn('Failed to pull dispatch result', { jobId, error: nextError });
    } finally {
      if (generation === generationRef.current) setPulling(false);
    }
  }, [jobId]);

  useEffect(() => {
    if (open && jobId) void pull();
  }, [open, jobId, pull]);

  const apply = useCallback(async (overwriteConflicts: boolean) => {
    if (!jobId || !workspacePath) return;
    const generation = ++generationRef.current;
    if (overwriteConflicts) {
      const confirmed = await confirmWarning(
        t('dispatch.resultOverwriteTitle'),
        t('dispatch.resultOverwriteMessage'),
        {
          confirmText: t('dispatch.resultOverwriteConfirm'),
          cancelText: t('dispatch.cancel'),
        },
      );
      if (!confirmed || generation !== generationRef.current) return;
    }
    setApplying(true);
    setError(null);
    try {
      const applied = await dispatchApi.applyResult(jobId, workspacePath, overwriteConflicts);
      if (generation !== generationRef.current) return;
      setOutcome(applied);
    } catch (nextError) {
      if (generation !== generationRef.current) return;
      setError(errorMessage(nextError));
      log.warn('Failed to apply dispatch result', { jobId, error: nextError });
    } finally {
      if (generation === generationRef.current) setApplying(false);
    }
  }, [jobId, t, workspacePath]);

  const summary = bundle?.summary;
  const changeCount =
    (summary?.added.length ?? 0) + (summary?.modified.length ?? 0) + (summary?.deleted.length ?? 0);
  const busy = pulling || applying;
  const applied = !!outcome && !outcome.aborted;

  const groups: Array<{ key: string; icon: React.ReactNode; label: string; paths: string[] }> = [
    { key: 'added', icon: <FilePlus size={14} />, label: t('dispatch.resultAdded'), paths: summary?.added ?? [] },
    { key: 'modified', icon: <FileDiff size={14} />, label: t('dispatch.resultModified'), paths: summary?.modified ?? [] },
    { key: 'deleted', icon: <FileX size={14} />, label: t('dispatch.resultDeleted'), paths: summary?.deleted ?? [] },
  ];

  return (
    <Modal
      isOpen={open}
      onClose={onClose}
      size="medium"
      closeOnOverlayClick
      showCloseButton
      ariaLabelledBy={DIALOG_TITLE_ID}
      testId="dispatch-result-dialog"
    >
      <div className="dispatch-result-dialog">
        <div className="dispatch-result-dialog__header">
          <h2 id={DIALOG_TITLE_ID} className="dispatch-result-dialog__title">
            {t('dispatch.resultTitle')}
          </h2>
          <span className="dispatch-result-dialog__subtitle">
            {targetLabel
              ? t('dispatch.resultSubtitleWithTarget', { target: targetLabel })
              : t('dispatch.resultSubtitle')}
          </span>
        </div>

        <div className="dispatch-result-dialog__body">
          {error ? (
            <Alert type="error" message={error} closable onClose={() => setError(null)} />
          ) : null}

          {pulling ? (
            <div className="dispatch-result-dialog__pending">
              <Loader2 size={14} className="dispatch-result-dialog__spin" />
              {t('dispatch.resultPulling')}
            </div>
          ) : null}

          {summary && !pulling ? (
            changeCount === 0 ? (
              <Alert type="info" message={t('dispatch.resultNoChanges')} />
            ) : (
              <>
                <div className="dispatch-result-dialog__field">
                  <span className="dispatch-result-dialog__field-label">
                    {t('dispatch.resultTargetWorkspace')}
                  </span>
                  <code>{bundle?.workspacePath}</code>
                </div>
                {groups
                  .filter(group => group.paths.length > 0)
                  .map(group => (
                    <section key={group.key} className="dispatch-result-dialog__group">
                      <div className="dispatch-result-dialog__group-header">
                        {group.icon}
                        <strong>{group.label}</strong>
                        <span>{group.paths.length}</span>
                      </div>
                      <ul data-kind={group.key}>
                        {group.paths.map(path => (
                          <li key={path}>{path}</li>
                        ))}
                      </ul>
                    </section>
                  ))}
              </>
            )
          ) : null}

          {outcome?.aborted ? (
            <Alert
              type="warning"
              message={t('dispatch.resultConflictWarning', { count: outcome.conflicts.length })}
            />
          ) : null}
          {outcome?.aborted ? (
            <section className="dispatch-result-dialog__group" data-conflict="true">
              <div className="dispatch-result-dialog__group-header">
                <strong>{t('dispatch.resultConflicts')}</strong>
                <span>{outcome.conflicts.length}</span>
              </div>
              <ul>
                {outcome.conflicts.map(conflict => (
                  <li key={conflict.path}>
                    {conflict.path}
                    <em>
                      {conflict.reason === 'locallyModified'
                        ? t('dispatch.resultConflictModified')
                        : t('dispatch.resultConflictMissing')}
                    </em>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {applied ? (
            <Alert
              type="success"
              message={t('dispatch.resultApplied', {
                written: outcome.written.length,
                removed: outcome.removed.length,
              })}
            />
          ) : null}
        </div>

        <div className="dispatch-result-dialog__actions">
          <Button variant="secondary" size="small" onClick={onClose}>
            {applied ? t('dispatch.resultClose') : t('dispatch.cancel')}
          </Button>
          {outcome?.aborted ? (
            <Button
              variant="primary"
              size="small"
              disabled={busy}
              onClick={() => void apply(true)}
            >
              {applying ? <Loader2 size={14} className="dispatch-result-dialog__spin" /> : null}
              {t('dispatch.resultOverwriteConfirm')}
            </Button>
          ) : (
            <Button
              variant="primary"
              size="small"
              disabled={busy || !summary || changeCount === 0 || applied || !workspacePath}
              onClick={() => void apply(false)}
            >
              {applying ? <Loader2 size={14} className="dispatch-result-dialog__spin" /> : null}
              {t('dispatch.resultApply')}
            </Button>
          )}
        </div>
      </div>
    </Modal>
  );
};
