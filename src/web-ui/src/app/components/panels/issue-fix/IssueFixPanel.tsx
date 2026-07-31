import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { AlertTriangle, CheckCircle, Circle, Loader2, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, Checkbox } from '@/component-library';
import {
  reviewPlatformAPI,
  type ReviewPlatformIssueSummary,
  type ReviewPlatformKind,
} from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import {
  emptyRunState,
  isBlockedOnHuman,
  rowLocked,
  rowState,
  rowStatusKey,
  runProgress,
  selectAllState,
  setAllSelected,
  toggleSelection,
  type IssueFixRowState,
} from './issueFixRunState';
import './IssueFixPanel.scss';

const log = createLogger('IssueFixPanel');

export interface IssueFixPanelProps {
  /** Local checkout the issues belong to; also resolves provider auth. */
  workspacePath?: string;
  /** `owner/repo`. When absent the panel resolves it from the workspace remote. */
  projectPath?: string;
  host?: string;
}

const ROW_ICONS: Record<IssueFixRowState, React.ReactNode> = {
  idle: <Circle size={12} className="issue-fix__row-icon issue-fix__row-icon--idle" />,
  queued: <Circle size={12} className="issue-fix__row-icon issue-fix__row-icon--queued" />,
  fixing: <Loader2 size={12} className="issue-fix__row-icon issue-fix__row-icon--fixing" />,
  done: <CheckCircle size={12} className="issue-fix__row-icon issue-fix__row-icon--done" />,
  blocked: (
    <AlertTriangle size={12} className="issue-fix__row-icon issue-fix__row-icon--blocked" />
  ),
};

/**
 * Lists a repository's open issues and tracks a fix run across them.
 *
 * Row state comes from `issueFixRunState`, which maps LoopX's decisions onto what
 * a user sees. The mapping that matters most: a `user_gate` renders as blocked,
 * never as done, because it is the one outcome that needs a person.
 */
export const IssueFixPanel: React.FC<IssueFixPanelProps> = ({
  workspacePath,
  projectPath: projectPathProp,
  host: hostProp,
}) => {
  const { t } = useTranslation('panels/issue-fix');
  const [issues, setIssues] = useState<ReviewPlatformIssueSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [runState, setRunState] = useState(emptyRunState);
  const [selectedIssueId, setSelectedIssueId] = useState<string | null>(null);
  const [resolved, setResolved] = useState<{
    projectPath: string;
    host: string;
    platform: ReviewPlatformKind;
  } | null>(
    projectPathProp
      ? { projectPath: projectPathProp, host: hostProp ?? 'github.com', platform: 'github' }
      : null,
  );

  const issueIds = useMemo(() => issues.map((issue) => issue.issueId), [issues]);

  // The caller only knows the local checkout, so resolve `owner/repo` and the
  // host from the workspace's selected remote.
  useEffect(() => {
    if (projectPathProp || !workspacePath) {
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        // Only the remote list is needed here, so ask for the smallest page.
        const snapshot = await reviewPlatformAPI.getWorkspaceSnapshot(workspacePath, null, 1, 1);
        const remote =
          snapshot.remotes.find((candidate) => candidate.id === snapshot.selectedRemoteId) ??
          snapshot.remotes[0];
        if (!cancelled && remote) {
          setResolved({
            projectPath: remote.projectPath,
            host: remote.host,
            platform: remote.platform,
          });
        }
      } catch (error) {
        log.error('Failed to resolve the workspace remote', { workspacePath, error });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [projectPathProp, workspacePath]);

  const projectPath = resolved?.projectPath;
  const host = resolved?.host ?? 'github.com';
  const platform = resolved?.platform ?? 'github';

  const loadIssues = useCallback(async () => {
    if (!projectPath) {
      return;
    }
    setLoading(true);
    setLoadError(null);
    try {
      const page = await reviewPlatformAPI.listIssues({
        platform,
        host,
        projectPath,
        state: 'open',
        perPage: 50,
        repositoryPath: workspacePath ?? null,
      });
      setIssues(page.items);
      setSelectedIssueId((current) => current ?? page.items[0]?.issueId ?? null);
    } catch (error) {
      log.error('Failed to list issues', { projectPath, error });
      setLoadError(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }, [host, platform, projectPath, workspacePath]);

  useEffect(() => {
    void loadIssues();
  }, [loadIssues]);

  const progress = useMemo(() => runProgress(runState, issueIds), [runState, issueIds]);
  const allState = useMemo(() => selectAllState(runState, issueIds), [runState, issueIds]);
  const blocked = useMemo(() => isBlockedOnHuman(runState, issueIds), [runState, issueIds]);
  const detail = useMemo(
    () => issues.find((issue) => issue.issueId === selectedIssueId) ?? null,
    [issues, selectedIssueId],
  );

  const handleToggleAll = useCallback(() => {
    setRunState((current) => setAllSelected(current, issueIds, allState !== 'all'));
  }, [allState, issueIds]);

  if (!projectPath) {
    return (
      <div className="issue-fix issue-fix--empty">
        <p className="issue-fix__empty-text">{t('noRepository')}</p>
      </div>
    );
  }

  return (
    <div className="issue-fix">
      <header className="issue-fix__header">
        <div className="issue-fix__header-main">
          <span className="issue-fix__repo">{projectPath}</span>
          <span className="issue-fix__progress">
            {t('progress', {
              done: progress.done,
              total: progress.total,
            })}
          </span>
        </div>
        <Button
          size="small"
          variant="ghost"
          iconOnly
          onClick={() => void loadIssues()}
          disabled={loading}
          aria-label={t('refresh')}
        >
          <RefreshCw size={14} />
        </Button>
      </header>

      {blocked ? (
        <div className="issue-fix__gate-notice" role="status">
          <AlertTriangle size={14} />
          <span>{t('gateNotice')}</span>
        </div>
      ) : null}

      <div className="issue-fix__body">
        <section className="issue-fix__list" aria-label={t('issuesLabel')}>
          <div className="issue-fix__list-header">
            <Checkbox
              checked={allState === 'all'}
              indeterminate={allState === 'some'}
              onChange={handleToggleAll}
              disabled={loading || issueIds.length === 0}
              size="small"
            />
            <span className="issue-fix__list-count">
              {t('selectedCount', {
                selected: runState.selectedIssueIds.size,
                total: progress.total,
              })}
            </span>
          </div>

          {loadError ? (
            <p className="issue-fix__error">{loadError}</p>
          ) : loading && issues.length === 0 ? (
            <p className="issue-fix__loading">{t('loading')}</p>
          ) : issues.length === 0 ? (
            <p className="issue-fix__empty-text">{t('noIssues')}</p>
          ) : (
            <ul className="issue-fix__rows">
              {issues.map((issue) => {
                const state = rowState(runState, issue.issueId);
                const locked = rowLocked(runState, issue.issueId);
                const statusKey = rowStatusKey(runState, issue.issueId);
                return (
                  <li
                    key={issue.issueId}
                    className={`issue-fix__row issue-fix__row--${state}${
                      selectedIssueId === issue.issueId ? ' is-selected' : ''
                    }`}
                    data-issue-state={state}
                  >
                    <Checkbox
                      checked={runState.selectedIssueIds.has(issue.issueId) || state === 'done'}
                      disabled={locked}
                      onChange={() =>
                        setRunState((current) => toggleSelection(current, issue.issueId))
                      }
                      size="small"
                    />
                    <button
                      type="button"
                      className="issue-fix__row-button"
                      onClick={() => setSelectedIssueId(issue.issueId)}
                    >
                      <span className="issue-fix__row-number">#{issue.number}</span>
                      <span className="issue-fix__row-title">{issue.title}</span>
                      {ROW_ICONS[state]}
                      {statusKey ? (
                        <span className="issue-fix__row-status">
                          {t(`status.${statusKey}`)}
                        </span>
                      ) : null}
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        <section className="issue-fix__detail" aria-label={t('detailLabel')}>
          {detail ? (
            <>
              <h3 className="issue-fix__detail-title">
                #{detail.number} {detail.title}
              </h3>
              <dl className="issue-fix__detail-facts">
                <dt>{t('detail.state')}</dt>
                <dd>{detail.state}</dd>
                {detail.author ? (
                  <>
                    <dt>{t('detail.author')}</dt>
                    <dd>{detail.author}</dd>
                  </>
                ) : null}
                <dt>{t('detail.comments')}</dt>
                <dd>{detail.commentsCount}</dd>
              </dl>
              {detail.labels.length > 0 ? (
                <ul className="issue-fix__labels">
                  {detail.labels.map((label) => (
                    <li key={label} className="issue-fix__label">
                      {label}
                    </li>
                  ))}
                </ul>
              ) : null}
              {(() => {
                // Show LoopX's reason codes verbatim rather than paraphrasing
                // them, so a declined fix explains itself in LoopX's own terms.
                const entry = runState.entries[detail.issueId];
                if (!entry?.reasonCodes?.length && !entry?.error) {
                  return null;
                }
                return (
                  <div className="issue-fix__decision">
                    {entry.error ? (
                      <p className="issue-fix__error">{entry.error}</p>
                    ) : (
                      <ul className="issue-fix__reasons">
                        {entry.reasonCodes?.map((code) => (
                          <li key={code} className="issue-fix__reason">
                            {code}
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                );
              })()}
              <a
                className="issue-fix__detail-link"
                href={detail.webUrl}
                target="_blank"
                rel="noreferrer"
              >
                {t('detail.openOnProvider')}
              </a>
            </>
          ) : (
            <p className="issue-fix__empty-text">{t('noSelection')}</p>
          )}
        </section>
      </div>
    </div>
  );
};

export default IssueFixPanel;
