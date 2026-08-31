import {
  Alert,
  Button,
  ScrollArea,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
} from '@bitfun/ui';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import { GitCommitHorizontal, Loader2 } from 'lucide-react';
import { dispatchApi } from './dispatchApi';
import type { DispatchSyncResult } from './types';
import './DispatchResultDialog.scss';

const log = createLogger('DispatchSyncDialog');
const DIALOG_TITLE_ID = 'dispatch-sync-dialog-title';

interface DispatchResultDialogProps {
  open: boolean;
  jobId: string;
  branch?: string;
  baselineWorktreePath?: string;
  baselineMissing?: boolean;
  targetLabel?: string;
  onClose: () => void;
}

/**
 * Commit the target worktree and fast-forward the controller's managed
 * baseline worktree from a Git bundle. The user's checkout is never touched.
 */
export const DispatchResultDialog: React.FC<DispatchResultDialogProps> = ({
  open,
  jobId,
  branch,
  baselineWorktreePath,
  baselineMissing = false,
  targetLabel,
  onClose,
}) => {
  const { t } = useI18n('common');
  const [result, setResult] = useState<DispatchSyncResult | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const generationRef = useRef(0);

  useEffect(() => {
    generationRef.current += 1;
    setResult(null);
    setError(null);
    setSyncing(false);
  }, [jobId, open]);

  const sync = useCallback(async () => {
    if (!jobId || baselineMissing) return;
    const generation = ++generationRef.current;
    setSyncing(true);
    setError(null);
    try {
      const synced = await dispatchApi.syncResult(jobId);
      if (generation !== generationRef.current) return;
      setResult(synced);
    } catch (nextError) {
      if (generation !== generationRef.current) return;
      setError(t('dispatch.syncFailed'));
      log.warn('Failed to sync dispatch result', { jobId, error: nextError });
    } finally {
      if (generation === generationRef.current) setSyncing(false);
    }
  }, [baselineMissing, jobId, t]);

  const resolvedBranch = result?.branch || branch;
  const resolvedBaselinePath = result?.baselineWorktreePath || baselineWorktreePath;
  const resolvedHeadCommit = result?.headCommit;

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => { if (!nextOpen) onClose(); }}
      size="md"
      closeOnPointerOutside
      aria-labelledby={DIALOG_TITLE_ID}
      data-testid="dispatch-sync-dialog"
    >
      <DialogHeader>
        <DialogClose />
      </DialogHeader>
      <DialogBody inset="none">
      <div
        className="dispatch-result-dialog"
        data-bf-component="dispatch-result-dialog"
        data-bf-part="root"
      >
        <div
          className="dispatch-result-dialog__header"
          data-bf-component="dispatch-result-dialog"
          data-bf-part="header"
        >
          <h2 id={DIALOG_TITLE_ID} className="dispatch-result-dialog__title">
            {t('dispatch.syncTitle')}
          </h2>
          <span className="dispatch-result-dialog__subtitle">
            {targetLabel
              ? t('dispatch.syncSubtitleWithTarget', { target: targetLabel })
              : t('dispatch.syncSubtitle')}
          </span>
        </div>

        <ScrollArea
          className="dispatch-result-dialog__body"
          data-bf-component="dispatch-result-dialog"
          data-bf-part="body"
        >
          {error ? (
            <Alert tone="error" message={error} closable onClose={() => setError(null)} />
          ) : null}
          {baselineMissing ? (
            <Alert tone="error" message={t('dispatch.syncBaselineMissing')} />
          ) : null}

          {resolvedBranch || resolvedBaselinePath || resolvedHeadCommit ? (
            <details className="dispatch-result-dialog__details">
              <summary>{t('dispatch.syncDetails')}</summary>
              <div className="dispatch-result-dialog__details-body">
                {resolvedBranch ? (
                  <div className="dispatch-result-dialog__field">
                    <span className="dispatch-result-dialog__field-label">
                      {t('dispatch.syncBranch')}
                    </span>
                    <code>{resolvedBranch}</code>
                  </div>
                ) : null}
                {resolvedBaselinePath ? (
                  <div className="dispatch-result-dialog__field">
                    <span className="dispatch-result-dialog__field-label">
                      {t('dispatch.syncBaselineWorktree')}
                    </span>
                    <code>{resolvedBaselinePath}</code>
                  </div>
                ) : null}
                {resolvedHeadCommit ? (
                  <div className="dispatch-result-dialog__field">
                    <span className="dispatch-result-dialog__field-label">
                      {t('dispatch.syncHeadCommit')}
                    </span>
                    <code>{resolvedHeadCommit}</code>
                  </div>
                ) : null}
              </div>
            </details>
          ) : null}

          {syncing ? (
            <div className="dispatch-result-dialog__pending">
              <Loader2 size={14} className="dispatch-result-dialog__spin" />
              {t('dispatch.syncingResult')}
            </div>
          ) : null}

          {result && !syncing ? (
            result.changed ? (
              <>
                <Alert
                  tone="success"
                  message={t('dispatch.syncSucceeded', { count: result.commitCount })}
                />
                <section className="dispatch-result-dialog__group">
                  <div className="dispatch-result-dialog__group-header">
                    <GitCommitHorizontal size={14} />
                    <strong>{t('dispatch.syncChangedFiles')}</strong>
                    <span>{result.changes.length}</span>
                  </div>
                  {result.changes.length > 0 ? (
                    <ScrollArea className="dispatch-result-dialog__change-list">
                      <ul>
                        {result.changes.map(change => (
                          <li key={`${change.status}:${change.path}`}>
                            <strong>{change.status}</strong>
                            <span>{change.path}</span>
                          </li>
                        ))}
                      </ul>
                    </ScrollArea>
                  ) : (
                    <div className="dispatch-result-dialog__empty">
                      {t('dispatch.syncNoFileList')}
                    </div>
                  )}
                </section>
                {result.truncatedChanges ? (
                  <Alert tone="info" message={t('dispatch.syncChangesTruncated')} />
                ) : null}
              </>
            ) : (
              <Alert tone="info" message={t('dispatch.syncNoChanges')} />
            )
          ) : null}
        </ScrollArea>

        <div
          className="dispatch-result-dialog__actions"
          data-bf-component="dispatch-result-dialog"
          data-bf-part="actions"
        >
          <Button variant="outline" size="sm" onClick={onClose}>
            {t('dispatch.syncClose')}
          </Button>
          <Button
            variant="fill"
            size="sm"
            disabled={syncing || baselineMissing || !jobId}
            onClick={() => void sync()}
          >
            {syncing ? <Loader2 size={14} className="dispatch-result-dialog__spin" /> : null}
            {t('dispatch.syncAction')}
          </Button>
        </div>
      </div>
          </DialogBody>
    </Dialog>
  );
};
