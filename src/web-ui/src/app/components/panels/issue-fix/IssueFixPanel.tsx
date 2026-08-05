import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertTriangle,
  CheckCircle,
  Circle,
  ExternalLink,
  Loader2,
  MessageSquare,
  Play,
  RefreshCw,
  Square,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, Checkbox, MarkdownRenderer } from '@/component-library';
import {
  agentAPI,
  issueFixAPI,
  reviewPlatformAPI,
  type IssueFixAutonomousStatusResponse,
  type IssueFixUserDecision,
  type ReviewPlatformIssueEvidence,
  type ReviewPlatformIssueSummary,
  type ReviewPlatformKind,
} from '@/infrastructure/api';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import {
  emptyRunState,
  mergeLightState,
  pruneSelection,
  rowLocked,
  rowState,
  rowStatusKey,
  runProgress,
  selectAllState,
  setAllSelected,
  toggleSelection,
  userTodoDisplayText,
  type IssueFixRowState,
} from './issueFixRunState';
import { IssueFixUserQuestion } from './IssueFixUserQuestion';
import './IssueFixPanel.scss';

const log = createLogger('IssueFixPanel');

export interface IssueFixPanelProps {
  workspacePath?: string;
  projectPath?: string;
  host?: string;
}

interface IssueMarkdownProps {
  content: string;
  emptyText: string;
  variant: 'body' | 'comment';
  basePath?: string;
}

const IssueMarkdown: React.FC<IssueMarkdownProps> = ({
  content,
  emptyText,
  variant,
  basePath,
}) => {
  const markdown = content.trim();
  if (!markdown) {
    return <p className="issue-fix__empty-text issue-fix__empty-text--inline">{emptyText}</p>;
  }
  return (
    <MarkdownRenderer
      content={markdown}
      basePath={basePath}
      className={`issue-fix__markdown issue-fix__markdown--${variant}`}
    />
  );
};

const ROW_ICONS: Record<IssueFixRowState, React.ReactNode> = {
  idle: <Circle size={12} className="issue-fix__row-icon issue-fix__row-icon--idle" />,
  queued: <Circle size={12} className="issue-fix__row-icon issue-fix__row-icon--queued" />,
  fixing: <Loader2 size={12} className="issue-fix__row-icon issue-fix__row-icon--fixing" />,
  done: <CheckCircle size={12} className="issue-fix__row-icon issue-fix__row-icon--done" />,
  blocked: (
    <AlertTriangle size={12} className="issue-fix__row-icon issue-fix__row-icon--blocked" />
  ),
};

function formatIssueTime(value: string | null | undefined): string {
  if (!value) return '';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return i18nService.formatDate(date, { dateStyle: 'medium', timeStyle: 'short' });
}

function requestId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export const IssueFixPanel: React.FC<IssueFixPanelProps> = ({
  workspacePath,
  projectPath: projectPathProp,
  host: hostProp,
}) => {
  const { t } = useTranslation('panels/issue-fix');
  const [issues, setIssues] = useState<ReviewPlatformIssueSummary[]>([]);
  const [selection, setSelection] = useState(emptyRunState);
  const [control, setControl] = useState<IssueFixAutonomousStatusResponse | null>(null);
  const [selectedIssueId, setSelectedIssueId] = useState<string | null>(null);
  const [available, setAvailable] = useState<boolean | null>(null);
  const [loading, setLoading] = useState(false);
  const [running, setRunning] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [controlError, setControlError] = useState<string | null>(null);
  const [answeringQuestion, setAnsweringQuestion] = useState(false);
  const [questionError, setQuestionError] = useState<string | null>(null);
  // Monotonic ticket so a slow response can never overwrite a newer one
  // (e.g. a stale refresh resurrecting a gate the user already answered).
  const controlTicketRef = useRef(0);
  const appliedControlTicketRef = useRef(0);
  // Remembers a host session created by a failed start, so retries do not
  // orphan one new session per attempt. Scoped to the current workspace.
  const createdSessionIdRef = useRef<string | null>(null);
  // While a mutation (start/stop/answer) is in flight, background polls are
  // paused: a poll issued mid-mutation would read pre-mutation state yet win
  // the ticket order, resurrecting what the mutation just changed.
  const mutationDepthRef = useRef(0);
  const mountedRef = useRef(true);

  useEffect(() => {
    createdSessionIdRef.current = null;
  }, [workspacePath]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const takeControlTicket = useCallback(() => ++controlTicketRef.current, []);

  const applyControl = useCallback(
    (
      ticket: number,
      update: (current: IssueFixAutonomousStatusResponse | null) => IssueFixAutonomousStatusResponse | null,
    ): boolean => {
      if (ticket < appliedControlTicketRef.current) return false;
      appliedControlTicketRef.current = ticket;
      setControl(update);
      return true;
    },
    [],
  );
  const [issueEvidenceById, setIssueEvidenceById] = useState<
    Record<string, ReviewPlatformIssueEvidence>
  >({});
  const [issueEvidenceErrors, setIssueEvidenceErrors] = useState<Record<string, string>>({});
  const [issueEvidenceLoadingId, setIssueEvidenceLoadingId] = useState<string | null>(null);
  const [resolved, setResolved] = useState<{
    projectPath: string;
    host: string;
    platform: ReviewPlatformKind;
  } | null>(
    projectPathProp
      ? { projectPath: projectPathProp, host: hostProp ?? 'github.com', platform: 'github' }
      : null,
  );

  useEffect(() => {
    if (projectPathProp || !workspacePath) return;
    let cancelled = false;
    void (async () => {
      try {
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
        if (!cancelled) setLoadError(error instanceof Error ? error.message : String(error));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [projectPathProp, workspacePath]);

  useEffect(() => {
    let cancelled = false;
    void issueFixAPI.probe().then((result) => {
      if (!cancelled) setAvailable(result.available);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const projectPath = resolved?.projectPath;
  const host = resolved?.host ?? 'github.com';
  const platform = resolved?.platform ?? 'github';
  const issueIds = useMemo(() => issues.map((issue) => issue.issueId), [issues]);

  const loadIssues = useCallback(async () => {
    if (!projectPath) return;
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
      setSelection((current) =>
        pruneSelection(current, page.items.map((issue) => issue.issueId)),
      );
      setSelectedIssueId((current) => current ?? page.items[0]?.issueId ?? null);
    } catch (error) {
      log.error('Failed to list issues', { projectPath, error });
      setLoadError(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }, [host, platform, projectPath, workspacePath]);

  const loadControl = useCallback(async () => {
    if (!workspacePath || available !== true) return;
    setControlError(null);
    const ticket = takeControlTicket();
    try {
      const status = await issueFixAPI.getAutonomousStatus(workspacePath);
      applyControl(ticket, () => status);
    } catch (error) {
      log.error('Failed to project continuous Issue-Fix state', { workspacePath, error });
      if (ticket >= appliedControlTicketRef.current && mountedRef.current) {
        setControlError(error instanceof Error ? error.message : String(error));
      }
    }
  }, [applyControl, available, takeControlTicket, workspacePath]);

  useEffect(() => {
    void loadIssues();
  }, [loadIssues]);

  useEffect(() => {
    if (available === true) void loadControl();
  }, [available, loadControl]);

  // While the host loop is enabled the panel must notice state LoopX changes
  // between beats — above all a user gate opening mid-run, which blocks all
  // progress until answered. The poll endpoint reads only the todo list (no
  // `quota should-run`, which writes a rollout event per call); a finished
  // beat (activeTurnId dropping) triggers one full quota-backed refresh.
  const hostLoopEnabled = control?.hostLoop.enabled ?? false;
  useEffect(() => {
    if (!hostLoopEnabled || available !== true || !workspacePath) return;
    let lastActiveTurnId = control?.hostLoop.activeTurnId ?? null;
    let lastNextRunAtMs = control?.hostLoop.nextRunAtMs ?? null;
    let cancelled = false;
    const tick = async () => {
      if (document.hidden || mutationDepthRef.current > 0) return;
      const ticket = takeControlTicket();
      try {
        const poll = await issueFixAPI.pollAutonomous(workspacePath);
        if (cancelled) return;
        // A beat boundary shows up either as the active turn draining or as
        // the schedule advancing (which also catches beats shorter than one
        // poll interval); each boundary earns one full quota-backed refresh.
        const beatFinished =
          (lastActiveTurnId !== null && !poll.hostLoop.activeTurnId) ||
          (lastNextRunAtMs !== null &&
            poll.hostLoop.nextRunAtMs != null &&
            poll.hostLoop.nextRunAtMs !== lastNextRunAtMs);
        lastActiveTurnId = poll.hostLoop.activeTurnId ?? null;
        lastNextRunAtMs = poll.hostLoop.nextRunAtMs ?? null;
        applyControl(ticket, (current) => (current ? mergeLightState(current, poll) : current));
        if (beatFinished) void loadControl();
      } catch (error) {
        log.warn('Continuous Issue-Fix poll failed', { workspacePath, error });
      }
    };
    const interval = window.setInterval(() => void tick(), 30_000);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
    // activeTurnId is tracked inside the closure; depending on it would reset the interval each beat.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [applyControl, available, hostLoopEnabled, loadControl, takeControlTicket, workspacePath]);

  const refresh = useCallback(async () => {
    await Promise.all([loadIssues(), loadControl()]);
  }, [loadControl, loadIssues]);

  const detail = useMemo(
    () => issues.find((issue) => issue.issueId === selectedIssueId) ?? null,
    [issues, selectedIssueId],
  );
  const detailEvidence = detail ? issueEvidenceById[detail.issueId] : undefined;
  const detailEvidenceLoading = detail ? issueEvidenceLoadingId === detail.issueId : false;
  const detailEvidenceError = detail ? issueEvidenceErrors[detail.issueId] : undefined;

  useEffect(() => {
    if (!detail || !projectPath || issueEvidenceById[detail.issueId]) return;
    let cancelled = false;
    const issueId = detail.issueId;
    setIssueEvidenceLoadingId(issueId);
    setIssueEvidenceErrors((current) => ({ ...current, [issueId]: '' }));
    void (async () => {
      try {
        const evidence = await reviewPlatformAPI.getIssue({
          platform,
          host,
          projectPath,
          issueId,
          page: 1,
          perPage: 20,
          repositoryPath: workspacePath ?? null,
        });
        if (!cancelled) {
          setIssueEvidenceById((current) => ({ ...current, [issueId]: evidence }));
        }
      } catch (error) {
        log.error('Failed to load issue evidence', { issueId, error });
        if (!cancelled) {
          setIssueEvidenceErrors((current) => ({
            ...current,
            [issueId]: error instanceof Error ? error.message : String(error),
          }));
        }
      } finally {
        if (!cancelled) {
          setIssueEvidenceLoadingId((current) => (current === issueId ? null : current));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [detail, host, issueEvidenceById, platform, projectPath, workspacePath]);

  const allState = useMemo(
    () => selectAllState(selection, control, issueIds),
    [control, issueIds, selection],
  );
  const progress = useMemo(
    () => runProgress(selection, control, issueIds),
    [control, issueIds, selection],
  );

  const handleToggleAll = useCallback(() => {
    setSelection((current) => setAllSelected(current, control, issueIds, allState !== 'all'));
  }, [allState, control, issueIds]);

  const ensureHostSession = useCallback(async (): Promise<string> => {
    // Prefer whichever known session actually exists in the store: the loop's
    // bound session may have been deleted while a locally created one (from a
    // failed start) is still valid.
    const { sessions } = flowChatStore.getState();
    for (const candidate of [control?.hostLoop.sessionId, createdSessionIdRef.current]) {
      if (candidate && sessions.has(candidate)) return candidate;
    }
    if (!workspacePath) throw new Error(t('autonomous.missingWorkspace'));

    const activeSession = flowChatStore.getActiveSession();
    const agentType = activeSession?.mode ?? activeSession?.config.agentType ?? 'agentic';
    const modelName = activeSession?.config.modelName ?? 'default';
    const executionTargetRequest = { kind: 'local' as const };
    const response = await agentAPI.createSession({
      sessionName: t('autonomous.sessionTitle'),
      agentType,
      workspacePath,
      projectWorkspacePath: workspacePath,
      workspaceId: activeSession?.workspaceId,
      executionTarget: executionTargetRequest,
      requestId: requestId(),
      config: {
        modelName,
        enableTools: true,
        safeMode: true,
        autoCompact: true,
        enableContextCompression: true,
      },
    });
    const sessionWorkspacePath = response.workspacePath ?? workspacePath;
    const resolvedAgentType = response.agentType || agentType;
    flowChatStore.createSession(
      response.sessionId,
      {
        modelName,
        agentType: resolvedAgentType,
        workspacePath: sessionWorkspacePath,
        projectWorkspacePath: response.projectWorkspacePath ?? workspacePath,
        workspaceId: response.workspaceId ?? activeSession?.workspaceId,
        executionTargetRequest,
        executionTarget: response.executionTarget,
      },
      undefined,
      response.sessionName || t('autonomous.sessionTitle'),
      activeSession?.maxContextTokens,
      resolvedAgentType,
      sessionWorkspacePath,
    );
    createdSessionIdRef.current = response.sessionId;
    return response.sessionId;
  }, [control?.hostLoop.sessionId, t, workspacePath]);

  const handleStart = useCallback(async () => {
    if (!projectPath || !workspacePath || selection.selectedIssueIds.size === 0) return;
    setRunning(true);
    setControlError(null);
    mutationDepthRef.current += 1;
    const ticket = takeControlTicket();
    try {
      const sessionId = await ensureHostSession();
      const selectedIssues = issues.filter((issue) => selection.selectedIssueIds.has(issue.issueId));
      const started = await issueFixAPI.startAutonomous({
        sessionId,
        repo: projectPath,
        repositoryPath: workspacePath,
        issues: selectedIssues.map((issue) => ({
          issueRef: issue.issueId,
          issueUrl: issue.webUrl,
        })),
      });
      createdSessionIdRef.current = null;
      if (applyControl(ticket, () => started)) {
        setSelection(emptyRunState());
        if (mountedRef.current) flowChatStore.switchSession(sessionId);
      } else {
        // A newer response outranked this start; re-sync from the backend so
        // the panel cannot show a state the host loop no longer has.
        void loadControl();
      }
    } catch (error) {
      log.error('Failed to start continuous Issue-Fix', { projectPath, error });
      if (mountedRef.current) {
        setControlError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      mutationDepthRef.current -= 1;
      if (mountedRef.current) setRunning(false);
    }
  }, [
    applyControl,
    ensureHostSession,
    issues,
    loadControl,
    projectPath,
    selection.selectedIssueIds,
    takeControlTicket,
    workspacePath,
  ]);

  const handleStop = useCallback(async () => {
    if (!workspacePath) return;
    setStopping(true);
    setControlError(null);
    mutationDepthRef.current += 1;
    const ticket = takeControlTicket();
    try {
      const hostLoop = await issueFixAPI.stopAutonomous(workspacePath);
      if (!applyControl(ticket, (current) => (current ? { ...current, hostLoop } : current))) {
        void loadControl();
      }
    } catch (error) {
      log.error('Failed to stop continuous Issue-Fix', { workspacePath, error });
      if (mountedRef.current) {
        setControlError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      mutationDepthRef.current -= 1;
      if (mountedRef.current) setStopping(false);
    }
  }, [applyControl, loadControl, takeControlTicket, workspacePath]);

  const openHostSession = useCallback(() => {
    if (control?.hostLoop.sessionId) flowChatStore.switchSession(control.hostLoop.sessionId);
  }, [control?.hostLoop.sessionId]);

  const handleUserQuestion = useCallback(async (
    decision: IssueFixUserDecision,
    reason: string,
  ) => {
    const question = control?.userQuestion;
    if (!workspacePath || !question) return;
    setAnsweringQuestion(true);
    setQuestionError(null);
    mutationDepthRef.current += 1;
    const ticket = takeControlTicket();
    try {
      const answered = await issueFixAPI.answerUserQuestion({
        repositoryPath: workspacePath,
        todoId: question.todoId,
        decision,
        reason: reason || null,
      });
      applyControl(ticket, () => answered);
    } catch (error) {
      log.error('Failed to answer continuous Issue-Fix user question', {
        todoId: question.todoId,
        decision,
        error,
      });
      if (mountedRef.current) {
        setQuestionError(error instanceof Error ? error.message : String(error));
      }
      // The gate may already be closed on the LoopX side (answered elsewhere,
      // superseded); re-project Kernel truth so a dead card cannot stick.
      void loadControl();
    } finally {
      mutationDepthRef.current -= 1;
      if (mountedRef.current) setAnsweringQuestion(false);
    }
  }, [applyControl, control?.userQuestion, loadControl, takeControlTicket, workspacePath]);

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
            {control
              ? t('autonomous.kernelSummary', {
                  goal: control.goalId,
                  state: control.kernelState,
                  queued: progress.queued,
                })
              : t('autonomous.loadingKernel')}
          </span>
        </div>
        <div className="issue-fix__actions">
          {control?.hostLoop.sessionId ? (
            <Button size="small" variant="ghost" onClick={openHostSession}>
              <MessageSquare size={13} />
              <span>{t('autonomous.openSession')}</span>
            </Button>
          ) : null}
          {control?.hostLoop.enabled ? (
            <Button
              size="small"
              variant="ghost"
              onClick={() => void handleStop()}
              disabled={stopping || running}
            >
              {stopping ? <Loader2 size={13} className="issue-fix__spin" /> : <Square size={13} />}
              <span>{stopping ? t('autonomous.stopping') : t('autonomous.stop')}</span>
            </Button>
          ) : null}
          <Button
            size="small"
            variant="primary"
            onClick={() => void handleStart()}
            disabled={
              !available ||
              running ||
              stopping ||
              control?.actionRequired ||
              selection.selectedIssueIds.size === 0
            }
          >
            {running ? <Loader2 size={13} className="issue-fix__spin" /> : <Play size={13} />}
            <span>{running ? t('autonomous.starting') : t('autonomous.start')}</span>
          </Button>
          <Button
            size="small"
            variant="ghost"
            iconOnly
            onClick={() => void refresh()}
            disabled={loading || running}
            aria-label={t('refresh')}
          >
            <RefreshCw size={14} />
          </Button>
        </div>
      </header>

      {available === false ? (
        <div className="issue-fix__notice issue-fix__notice--warning" role="status">
          <AlertTriangle size={14} />
          <span>{t('loopxMissing')}</span>
        </div>
      ) : null}
      {control?.hostLoop.lastError &&
      ((control.hostLoop.consecutiveFailures ?? 0) > 0 ||
        (!control.hostLoop.enabled && control.hostLoop.lastRunStatus === 'error')) ? (
        <div className="issue-fix__notice issue-fix__notice--warning" role="status">
          <AlertTriangle size={14} />
          <span>{t('autonomous.hostLoopFailure', { error: control.hostLoop.lastError })}</span>
        </div>
      ) : null}
      {control?.userQuestion ? (
        <IssueFixUserQuestion
          question={control.userQuestion}
          submitting={answeringQuestion}
          error={questionError}
          onSubmit={(decision, reason) => void handleUserQuestion(decision, reason)}
        />
      ) : control?.actionRequired ? (
        <div className="issue-fix__notice issue-fix__notice--warning" role="status">
          <AlertTriangle size={14} />
          <span>{control.gatePrompt ?? control.recommendedAction ?? t('autonomous.actionRequired')}</span>
        </div>
      ) : control?.hostLoop.enabled ? (
        <div className="issue-fix__notice issue-fix__notice--active" role="status">
          <CheckCircle size={14} />
          <span>{t('autonomous.hostLoopActive', { agent: control.agentId })}</span>
        </div>
      ) : null}
      {controlError ? <p className="issue-fix__error issue-fix__error--banner">{controlError}</p> : null}

      {control?.userTodos?.length ? (
        <section
          className="issue-fix__user-todos"
          aria-label={t('autonomous.userTodos.title', { count: control.userTodos.length })}
        >
          <h4 className="issue-fix__user-todos-title">
            {t('autonomous.userTodos.title', { count: control.userTodos.length })}
          </h4>
          <ul className="issue-fix__user-todos-list">
            {control.userTodos.map((todo) => (
              <li key={todo.todoId} className="issue-fix__user-todo">
                <span
                  className={`issue-fix__user-todo-badge issue-fix__user-todo-badge--${
                    todo.taskClass === 'user_gate' ? 'gate' : 'action'
                  }`}
                >
                  {t(
                    todo.taskClass === 'user_gate'
                      ? 'autonomous.userTodos.gateBadge'
                      : 'autonomous.userTodos.actionBadge',
                  )}
                </span>
                <span className="issue-fix__user-todo-text" title={todo.text}>
                  {userTodoDisplayText(todo)}
                </span>
                {todo.link ? (
                  <a
                    className="issue-fix__user-todo-link"
                    href={todo.link}
                    target="_blank"
                    rel="noreferrer"
                    aria-label={t('autonomous.userTodos.openLink')}
                  >
                    <ExternalLink size={12} aria-hidden="true" />
                  </a>
                ) : null}
              </li>
            ))}
          </ul>
        </section>
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
                selected: selection.selectedIssueIds.size,
                total: progress.total,
              })}
            </span>
          </div>

          {loadError ? (
            <p className="issue-fix__error issue-fix__error--banner">{loadError}</p>
          ) : loading && issues.length === 0 ? (
            <p className="issue-fix__loading">{t('loading')}</p>
          ) : issues.length === 0 ? (
            <p className="issue-fix__empty-text">{t('noIssues')}</p>
          ) : (
            <ul className="issue-fix__rows">
              {issues.map((issue) => {
                const state = rowState(selection, control, issue.issueId);
                const locked = rowLocked(control, issue.issueId);
                const statusKey = rowStatusKey(state);
                return (
                  <li
                    key={issue.issueId}
                    className={`issue-fix__row issue-fix__row--${state}${
                      selectedIssueId === issue.issueId ? ' is-selected' : ''
                    }`}
                    data-issue-state={state}
                  >
                    <Checkbox
                      checked={selection.selectedIssueIds.has(issue.issueId) || locked}
                      disabled={locked}
                      onChange={() =>
                        setSelection((current) => toggleSelection(current, control, issue.issueId))
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
                        <span className="issue-fix__row-status">{t(`status.${statusKey}`)}</span>
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
              <div className="issue-fix__detail-title-row">
                <h3 className="issue-fix__detail-title">
                  #{detail.number} {detail.title}
                </h3>
                <a
                  className="issue-fix__detail-link"
                  href={detail.webUrl}
                  target="_blank"
                  rel="noreferrer"
                >
                  <span>{t('detail.openOnProvider')}</span>
                  <ExternalLink size={12} aria-hidden="true" />
                </a>
              </div>
              <div className="issue-fix__detail-scroll">
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
                  {detail.createdAt ? (
                    <>
                      <dt>{t('detail.created')}</dt>
                      <dd>{formatIssueTime(detail.createdAt) || detail.createdAt}</dd>
                    </>
                  ) : null}
                  {detail.updatedAt ? (
                    <>
                      <dt>{t('detail.updated')}</dt>
                      <dd>{formatIssueTime(detail.updatedAt) || detail.updatedAt}</dd>
                    </>
                  ) : null}
                </dl>
                {detail.labels.length > 0 ? (
                  <ul className="issue-fix__labels">
                    {detail.labels.map((label) => (
                      <li key={label} className="issue-fix__label">{label}</li>
                    ))}
                  </ul>
                ) : null}

                <section className="issue-fix__detail-section">
                  <div className="issue-fix__section-heading">
                    <h4>{t('detail.body')}</h4>
                    {detailEvidenceLoading ? <span>{t('detail.loadingEvidence')}</span> : null}
                  </div>
                  {detailEvidenceError ? (
                    <p className="issue-fix__error">{detailEvidenceError}</p>
                  ) : (
                    <IssueMarkdown
                      content={detailEvidence?.body ?? ''}
                      emptyText={t('detail.emptyBody')}
                      variant="body"
                      basePath={workspacePath}
                    />
                  )}
                </section>

                <section className="issue-fix__detail-section">
                  <div className="issue-fix__section-heading">
                    <h4>{t('detail.commentsTitle', { count: detail.commentsCount })}</h4>
                  </div>
                  {detailEvidence?.comments.length ? (
                    <ol className="issue-fix__comments">
                      {detailEvidence.comments.map((comment) => (
                        <li key={comment.id} className="issue-fix__comment">
                          <div className="issue-fix__comment-header">
                            <span className="issue-fix__comment-author">
                              {comment.author || t('detail.unknownAuthor')}
                            </span>
                            <span className="issue-fix__comment-time">
                              {formatIssueTime(comment.createdAt) || comment.createdAt}
                            </span>
                          </div>
                          <IssueMarkdown
                            content={comment.body}
                            emptyText={t('detail.emptyComment')}
                            variant="comment"
                            basePath={workspacePath}
                          />
                        </li>
                      ))}
                    </ol>
                  ) : (
                    <p className="issue-fix__empty-text issue-fix__empty-text--inline">
                      {detailEvidenceLoading ? t('detail.loadingEvidence') : t('detail.noComments')}
                    </p>
                  )}
                </section>
              </div>
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
