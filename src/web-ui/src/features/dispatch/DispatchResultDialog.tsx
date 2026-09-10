import {
  Alert, Button, ScrollArea, Dialog, DialogBody, DialogClose, DialogDescription,
  DialogFooter, DialogHeader, DialogHeading, DialogTitle, Disclosure, Icon,
} from '@openbitfun/ui';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import { Loader2 } from 'lucide-react';
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
      <div
        className="dispatch-result-dialog"
        data-openbitfun-component="dispatch-result-dialog"
        data-openbitfun-part="root"
      >
        <DialogHeader
          data-openbitfun-component="dispatch-result-dialog"
          data-openbitfun-part="header"
        >
          <DialogHeading>
            <DialogTitle id={DIALOG_TITLE_ID}>{t('dispatch.syncTitle')}</DialogTitle>
            <DialogDescription>
              {targetLabel
                ? t('dispatch.syncSubtitleWithTarget', { target: targetLabel })
                : t('dispatch.syncSubtitle')}
            </DialogDescription>
          </DialogHeading>
          <DialogClose />
        </DialogHeader>

        <DialogBody
          className="dispatch-result-dialog__body"
          data-openbitfun-component="dispatch-result-dialog"
          data-openbitfun-part="body"
        >
          {error ? (
            <Alert tone="error" message={error} closable onClose={() => setError(null)} />
          ) : null}
          {baselineMissing ? (
            <Alert tone="error" message={t('dispatch.syncBaselineMissing')} />
          ) : null}

          {resolvedBranch || resolvedBaselinePath || resolvedHeadCommit ? (
            <Disclosure
              className="dispatch-result-dialog__details"
              summary={t('dispatch.syncDetails')}
            >
              <dl className="dispatch-result-dialog__details-body">
                {resolvedBranch ? (
                  <div className="dispatch-result-dialog__field">
                    <dt className="dispatch-result-dialog__field-label">
                      {t('dispatch.syncBranch')}
                    </dt>
                    <dd>{resolvedBranch}</dd>
                  </div>
                ) : null}
                {resolvedBaselinePath ? (
                  <div className="dispatch-result-dialog__field">
                    <dt className="dispatch-result-dialog__field-label">
                      {t('dispatch.syncBaselineWorktree')}
                    </dt>
                    <dd>{resolvedBaselinePath}</dd>
                  </div>
                ) : null}
                {resolvedHeadCommit ? (
                  <div className="dispatch-result-dialog__field">
                    <dt className="dispatch-result-dialog__field-label">
                      {t('dispatch.syncHeadCommit')}
                    </dt>
                    <dd>{resolvedHeadCommit}</dd>
                  </div>
                ) : null}
              </dl>
            </Disclosure>
          ) : null}

          {syncing ? (
            <div className="dispatch-result-dialog__pending" role="status">
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
                    <Icon name="commit" size="sm" />
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
        </DialogBody>

        <div
          className="dispatch-result-dialog__actions"
          data-openbitfun-component="dispatch-result-dialog"
          data-openbitfun-part="actions"
        >
          <DialogFooter>
          <Button variant="fill" size="sm" onClick={onClose}>
            {t('dispatch.syncClose')}
          </Button>
          <Button
            variant="primary"
            size="sm"
            disabled={syncing || baselineMissing || !jobId}
            onClick={() => void sync()}
          >
            {syncing ? <Loader2 size={14} className="dispatch-result-dialog__spin" /> : null}
            {t('dispatch.syncAction')}
          </Button>
          </DialogFooter>
        </div>
      </div>
    </Dialog>
  );
};
