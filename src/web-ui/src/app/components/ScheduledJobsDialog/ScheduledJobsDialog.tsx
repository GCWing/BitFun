import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Clock3, Plus, RefreshCw, Trash2 } from 'lucide-react';
import {
  Button,
  Checkbox,
  Input,
  Modal,
  Select,
  Textarea,
  confirmDanger,
} from '@/component-library';
import {
  cronAPI,
  type CreateCronJobRequest,
  type CronJob,
  type CronSchedule,
  type UpdateCronJobRequest,
} from '@/infrastructure/api';
import { useI18n } from '@/infrastructure/i18n';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { FlowChatState, Session } from '@/flow_chat/types/flow-chat';
import { compareSessionsForDisplay } from '@/flow_chat/utils/sessionOrdering';
import { notificationService } from '@/shared/notification-system/services/NotificationService';
import { createLogger } from '@/shared/utils/logger';
import './ScheduledJobsDialog.scss';

const log = createLogger('ScheduledJobsDialog');
const MINUTE_IN_MS = 60_000;

type JobListScope = 'workspace' | 'session';
type ScheduleKind = CronSchedule['kind'];

interface JobDraft {
  name: string;
  text: string;
  enabled: boolean;
  workspacePath: string;
  sessionId: string;
  scheduleKind: ScheduleKind;
  at: string;
  everyMinutes: string;
  anchorMs: string;
  expr: string;
  tz: string;
}

export interface ScheduledJobsDialogProps {
  isOpen: boolean;
  onClose: () => void;
  targetWorkspacePath?: string;
  targetSessionId?: string;
  assistantName?: string;
  hideTargetFields?: boolean;
  listScope?: JobListScope;
}

const createEmptyDraft = (workspacePath = '', sessionId = ''): JobDraft => ({
  name: '',
  text: '',
  enabled: true,
  workspacePath,
  sessionId,
  scheduleKind: 'cron',
  at: getCurrentLocalDateTimeInput(),
  everyMinutes: '60',
  anchorMs: '',
  expr: '0 8 * * *',
  tz: '',
});

const ScheduledJobsDialog: React.FC<ScheduledJobsDialogProps> = ({
  isOpen,
  onClose,
  targetWorkspacePath,
  targetSessionId,
  assistantName,
  hideTargetFields = false,
  listScope = 'session',
}) => {
  const { t } = useI18n('common');
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const [jobs, setJobs] = useState<CronJob[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const [draft, setDraft] = useState<JobDraft>(() =>
    createEmptyDraft(targetWorkspacePath ?? '', targetSessionId ?? '')
  );

  useEffect(() => {
    const unsubscribe = flowChatStore.subscribe((state) => {
      setFlowChatState(state);
    });
    return unsubscribe;
  }, []);

  const workspaceSessions = useMemo(() => {
    const workspacePath = draft.workspacePath.trim();
    if (!workspacePath) {
      return [] as Session[];
    }

    return Array.from(flowChatState.sessions.values())
      .filter((session) => (session.workspacePath || workspacePath) === workspacePath && !session.parentSessionId)
      .sort(compareSessionsForDisplay);
  }, [draft.workspacePath, flowChatState.sessions]);

  const defaultSessionIdForWorkspace = useMemo(
    () => workspaceSessions[0]?.sessionId ?? '',
    [workspaceSessions]
  );
  const listWorkspacePath = useMemo(() => {
    return hideTargetFields
      ? (targetWorkspacePath ?? '').trim()
      : draft.workspacePath.trim();
  }, [draft.workspacePath, hideTargetFields, targetWorkspacePath]);
  const listSessionId = useMemo(() => {
    if (listScope !== 'session') {
      return '';
    }

    return hideTargetFields
      ? (targetSessionId ?? '').trim()
      : draft.sessionId.trim();
  }, [draft.sessionId, hideTargetFields, listScope, targetSessionId]);

  const displayTargetLabel = hideTargetFields
    ? assistantName || t('nav.scheduledJobs.assistantFallback')
    : null;

  const sortedJobs = useMemo(() => {
    return [...jobs].sort((left, right) => {
      if (left.enabled !== right.enabled) {
        return left.enabled ? -1 : 1;
      }

      const configUpdatedAtDiff = right.configUpdatedAtMs - left.configUpdatedAtMs;
      if (configUpdatedAtDiff !== 0) {
        return configUpdatedAtDiff;
      }

      const createdAtDiff = right.createdAtMs - left.createdAtMs;
      if (createdAtDiff !== 0) {
        return createdAtDiff;
      }

      return left.id.localeCompare(right.id);
    });
  }, [jobs]);

  const selectedJob = useMemo(
    () => sortedJobs.find((job) => job.id === selectedJobId) ?? null,
    [selectedJobId, sortedJobs]
  );

  const loadJobs = useCallback(async () => {
    if (!isOpen) {
      return;
    }

    const request =
      listScope === 'workspace'
        ? { workspacePath: listWorkspacePath || undefined }
        : {
          workspacePath: listWorkspacePath || undefined,
          sessionId: listSessionId || undefined,
        };

    setLoading(true);
    try {
      const result = await cronAPI.listJobs(request);
      setJobs(result);
      setSelectedJobId((current) => {
        if (current && result.some((job) => job.id === current)) {
          return current;
        }
        return result[0]?.id ?? null;
      });
    } catch (error) {
      log.error('Failed to load scheduled jobs', { error });
      notificationService.error(
        t('nav.scheduledJobs.messages.loadFailed', {
          error: error instanceof Error ? error.message : String(error),
        })
      );
    } finally {
      setLoading(false);
    }
  }, [isOpen, listScope, listSessionId, listWorkspacePath, t]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const initialWorkspacePath = targetWorkspacePath ?? '';
    const initialSessionId = targetSessionId ?? '';
    setDraft((current) => {
      const nextDraft = current.name || current.text || current.workspacePath || current.sessionId
        ? current
        : createEmptyDraft(initialWorkspacePath, initialSessionId);
      const desiredWorkspacePath = hideTargetFields
        ? initialWorkspacePath
        : current.workspacePath || initialWorkspacePath;
      const desiredSessionId = hideTargetFields
        ? initialSessionId || nextDraft.sessionId
        : current.sessionId || initialSessionId;

      if (
        nextDraft.workspacePath === desiredWorkspacePath
        && nextDraft.sessionId === desiredSessionId
      ) {
        return nextDraft;
      }

      return {
        ...nextDraft,
        workspacePath: desiredWorkspacePath,
        sessionId: desiredSessionId,
      };
    });
  }, [hideTargetFields, isOpen, targetSessionId, targetWorkspacePath]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    setDraft((current) => {
      if (hideTargetFields) {
        const nextSessionId = targetSessionId || current.sessionId || defaultSessionIdForWorkspace;
        if (
          current.workspacePath === (targetWorkspacePath ?? '')
          && current.sessionId === nextSessionId
        ) {
          return current;
        }

        return {
          ...current,
          workspacePath: targetWorkspacePath ?? '',
          sessionId: nextSessionId,
        };
      }

      if (current.workspacePath && current.sessionId && workspaceSessions.some((session) => session.sessionId === current.sessionId)) {
        return current;
      }

      if (!current.workspacePath) {
        return current;
      }

      return {
        ...current,
        sessionId: current.sessionId || defaultSessionIdForWorkspace,
      };
    });
  }, [
    defaultSessionIdForWorkspace,
    hideTargetFields,
    isOpen,
    targetSessionId,
    targetWorkspacePath,
    workspaceSessions,
  ]);

  useEffect(() => {
    if (!isOpen) {
      setJobs([]);
      setSelectedJobId(null);
      setDraft(createEmptyDraft(targetWorkspacePath ?? '', targetSessionId ?? ''));
      return;
    }

    void loadJobs();
  }, [isOpen, loadJobs, targetSessionId, targetWorkspacePath]);

  useEffect(() => {
    if (!selectedJob) {
      return;
    }

    setDraft(jobToDraft(selectedJob));
  }, [selectedJob]);

  const handleCreateDraft = useCallback(() => {
    setSelectedJobId(null);
    setDraft(createEmptyDraft(draft.workspacePath, draft.sessionId || defaultSessionIdForWorkspace));
  }, [defaultSessionIdForWorkspace, draft.sessionId, draft.workspacePath]);

  const handleSelectJob = useCallback((job: CronJob) => {
    setSelectedJobId(job.id);
  }, []);

  const handleDeleteJob = useCallback(async (job: CronJob) => {
    const confirmed = await confirmDanger(
      t('nav.scheduledJobs.deleteDialog.title', { name: job.name }),
      null
    );
    if (!confirmed) {
      return;
    }

    try {
      await cronAPI.deleteJob(job.id);
      notificationService.success(t('nav.scheduledJobs.messages.deleteSuccess'));
      if (selectedJobId === job.id) {
        setSelectedJobId(null);
      }
      await loadJobs();
    } catch (error) {
      log.error('Failed to delete scheduled job', { jobId: job.id, error });
      notificationService.error(
        t('nav.scheduledJobs.messages.deleteFailed', {
          error: error instanceof Error ? error.message : String(error),
        })
      );
    }
  }, [loadJobs, selectedJobId, t]);

  const handleToggleEnabled = useCallback(async (job: CronJob, enabled: boolean) => {
    try {
      await cronAPI.updateJob(job.id, { enabled });
      await loadJobs();
    } catch (error) {
      log.error('Failed to toggle scheduled job', { jobId: job.id, error });
      notificationService.error(
        t('nav.scheduledJobs.messages.updateFailed', {
          error: error instanceof Error ? error.message : String(error),
        })
      );
    }
  }, [loadJobs, t]);

  const handleSave = useCallback(async () => {
    const validationError = validateDraft(draft, t);
    if (validationError) {
      notificationService.error(validationError);
      return;
    }

    let schedule: CronSchedule;
    try {
      schedule = buildScheduleFromDraft(draft);
    } catch (error) {
      notificationService.error(error instanceof Error ? error.message : String(error));
      return;
    }

    setSaving(true);
    try {
      if (selectedJobId) {
        const request: UpdateCronJobRequest = {
          name: draft.name.trim(),
          payload: {
            text: draft.text.trim(),
          },
          enabled: draft.enabled,
          schedule,
          workspacePath: draft.workspacePath.trim(),
          sessionId: draft.sessionId.trim(),
        };
        const updatedJob = await cronAPI.updateJob(selectedJobId, request);
        setSelectedJobId(updatedJob.id);
        setDraft(jobToDraft(updatedJob));
        notificationService.success(t('nav.scheduledJobs.messages.updateSuccess'));
      } else {
        const request: CreateCronJobRequest = {
          name: draft.name.trim(),
          payload: {
            text: draft.text.trim(),
          },
          enabled: draft.enabled,
          schedule,
          workspacePath: draft.workspacePath.trim(),
          sessionId: draft.sessionId.trim(),
        };
        const createdJob = await cronAPI.createJob(request);
        setSelectedJobId(createdJob.id);
        setDraft(jobToDraft(createdJob));
        notificationService.success(t('nav.scheduledJobs.messages.createSuccess'));
      }

      await loadJobs();
    } catch (error) {
      log.error('Failed to save scheduled job', { error });
      notificationService.error(
        t('nav.scheduledJobs.messages.saveFailed', {
          error: error instanceof Error ? error.message : String(error),
        })
      );
    } finally {
      setSaving(false);
    }
  }, [draft, loadJobs, selectedJobId, t]);

  const sessionOptions = useMemo(() => {
    return workspaceSessions.map((session) => ({
      value: session.sessionId,
      label: resolveSessionLabel(session),
      description: session.title || session.sessionId,
    }));
  }, [workspaceSessions]);

  const canCreateJob = Boolean(draft.workspacePath.trim() && draft.sessionId.trim());

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('nav.scheduledJobs.title')}
      titleExtra={displayTargetLabel ? (
        <span className="bitfun-scheduled-jobs-dialog__title-target">{displayTargetLabel}</span>
      ) : undefined}
      size="xlarge"
      contentInset={false}
    >
      <div className="bitfun-scheduled-jobs-dialog">
        <div className="bitfun-scheduled-jobs-dialog__sidebar">
          <div className="bitfun-scheduled-jobs-dialog__sidebar-header">
            <div>
              <div className="bitfun-scheduled-jobs-dialog__sidebar-title">
                <Clock3 size={16} />
                <span>{t('nav.scheduledJobs.listTitle')}</span>
              </div>
            </div>
            <div className="bitfun-scheduled-jobs-dialog__sidebar-actions">
              <button
                type="button"
                className="bitfun-scheduled-jobs-dialog__icon-btn"
                onClick={() => { void loadJobs(); }}
                aria-label={t('nav.scheduledJobs.actions.refresh')}
                disabled={loading}
              >
                <RefreshCw size={14} />
              </button>
              <button
                type="button"
                className="bitfun-scheduled-jobs-dialog__icon-btn"
                onClick={handleCreateDraft}
                aria-label={t('nav.scheduledJobs.actions.newJob')}
                disabled={hideTargetFields && !defaultSessionIdForWorkspace}
              >
                <Plus size={14} />
              </button>
            </div>
          </div>

          {loading ? (
            <div className="bitfun-scheduled-jobs-dialog__empty">
              {t('nav.scheduledJobs.loading')}
            </div>
          ) : sortedJobs.length === 0 ? (
            <div className="bitfun-scheduled-jobs-dialog__empty">
              <div className="bitfun-scheduled-jobs-dialog__empty-title">
                {t('nav.scheduledJobs.empty.title')}
              </div>
              <div className="bitfun-scheduled-jobs-dialog__empty-text">
                {t('nav.scheduledJobs.empty.description')}
              </div>
            </div>
          ) : (
            <div className="bitfun-scheduled-jobs-dialog__job-list">
              {sortedJobs.map((job) => {
                const isSelected = selectedJobId === job.id;
                return (
                  <div
                    key={job.id}
                    className={`bitfun-scheduled-jobs-dialog__job-card${isSelected ? ' is-selected' : ''}`}
                    onClick={() => handleSelectJob(job)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        handleSelectJob(job);
                      }
                    }}
                  >
                    <div className="bitfun-scheduled-jobs-dialog__job-main">
                      <div className="bitfun-scheduled-jobs-dialog__job-name-row">
                        <span className="bitfun-scheduled-jobs-dialog__job-name">{job.name}</span>
                        <span className={`bitfun-scheduled-jobs-dialog__status-badge is-${job.state.lastRunStatus ?? 'idle'}`}>
                          {formatStatusLabel(job, t)}
                        </span>
                      </div>
                      <div className="bitfun-scheduled-jobs-dialog__job-meta">
                        {formatScheduleSummary(job.schedule, t)}
                      </div>
                      <div className="bitfun-scheduled-jobs-dialog__job-meta">
                        {t('nav.scheduledJobs.nextRunLabel')}: {formatTimestamp(getNextExecutionAtMs(job), t)}
                      </div>
                      {job.state.lastError ? (
                        <div className="bitfun-scheduled-jobs-dialog__job-error">
                          {job.state.lastError}
                        </div>
                      ) : null}
                    </div>
                    <div className="bitfun-scheduled-jobs-dialog__job-actions">
                      <Checkbox
                        checked={job.enabled}
                        onChange={(event) => {
                          event.stopPropagation();
                          void handleToggleEnabled(job, event.currentTarget.checked);
                        }}
                        aria-label={t('nav.scheduledJobs.actions.toggleEnabled')}
                      />
                      <button
                        type="button"
                        className="bitfun-scheduled-jobs-dialog__icon-btn is-danger"
                        aria-label={t('nav.scheduledJobs.actions.delete')}
                        onClick={(event) => {
                          event.stopPropagation();
                          void handleDeleteJob(job);
                        }}
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        <div className="bitfun-scheduled-jobs-dialog__editor">
          <div className="bitfun-scheduled-jobs-dialog__editor-header">
            <div>
              <div className="bitfun-scheduled-jobs-dialog__editor-title">
                {selectedJobId
                  ? t('nav.scheduledJobs.editor.editTitle')
                  : t('nav.scheduledJobs.editor.createTitle')}
              </div>
            </div>
            <div className="bitfun-scheduled-jobs-dialog__editor-actions">
              <Button
                variant="primary"
                onClick={() => { void handleSave(); }}
                disabled={!canCreateJob}
                isLoading={saving}
              >
                {selectedJobId
                  ? t('nav.scheduledJobs.actions.save')
                  : t('nav.scheduledJobs.actions.create')}
              </Button>
            </div>
          </div>

          {!hideTargetFields ? (
            <div className="bitfun-scheduled-jobs-dialog__target-grid">
              <Input
                label={t('nav.scheduledJobs.fields.workspacePath')}
                value={draft.workspacePath}
                onChange={(event) => {
                  const workspacePath = event.currentTarget.value;
                  setDraft((current) => ({ ...current, workspacePath }));
                }}
                placeholder={t('nav.scheduledJobs.placeholders.workspacePath')}
              />
              <Select
                label={t('nav.scheduledJobs.fields.session')}
                options={sessionOptions}
                value={draft.sessionId}
                allowCustomValue
                searchable
                onChange={(value) => {
                  setDraft((current) => ({ ...current, sessionId: String(value) }));
                }}
                placeholder={t('nav.scheduledJobs.placeholders.session')}
              />
            </div>
          ) : null}

          {!canCreateJob ? (
            <div className="bitfun-scheduled-jobs-dialog__warning">
              {t('nav.scheduledJobs.messages.sessionRequired')}
            </div>
          ) : null}

          <div className="bitfun-scheduled-jobs-dialog__form-grid">
            <Input
              label={t('nav.scheduledJobs.fields.name')}
              value={draft.name}
              onChange={(event) => {
                const name = event.currentTarget.value;
                setDraft((current) => ({ ...current, name }));
              }}
              placeholder={t('nav.scheduledJobs.placeholders.name')}
            />

            <div className="bitfun-scheduled-jobs-dialog__field">
              <div className="bitfun-scheduled-jobs-dialog__field-label">
                {t('nav.scheduledJobs.fields.enabled')}
              </div>
              <Checkbox
                checked={draft.enabled}
                label={t('nav.scheduledJobs.fields.enabledHint')}
                onChange={(event) => {
                  const enabled = event.currentTarget.checked;
                  setDraft((current) => ({ ...current, enabled }));
                }}
              />
            </div>

            <Select
              label={t('nav.scheduledJobs.fields.scheduleKind')}
              value={draft.scheduleKind}
              options={[
                { value: 'at', label: t('nav.scheduledJobs.scheduleKinds.at') },
                { value: 'every', label: t('nav.scheduledJobs.scheduleKinds.every') },
                { value: 'cron', label: t('nav.scheduledJobs.scheduleKinds.cron') },
              ]}
              onChange={(value) => {
                setDraft((current) => {
                  if ((value as ScheduleKind) === 'at' && current.scheduleKind !== 'at' && !current.at.trim()) {
                    return {
                      ...current,
                      scheduleKind: 'at',
                      at: getCurrentLocalDateTimeInput(),
                    };
                  }

                  return {
                    ...current,
                    scheduleKind: value as ScheduleKind,
                  };
                });
              }}
            />

            {draft.scheduleKind === 'at' ? (
              <Input
                type="datetime-local"
                label={t('nav.scheduledJobs.fields.at')}
                value={draft.at}
                onChange={(event) => {
                  const at = event.currentTarget.value;
                  setDraft((current) => ({ ...current, at }));
                }}
              />
            ) : null}

            {draft.scheduleKind === 'every' ? (
              <>
                <Input
                  type="number"
                  label={t('nav.scheduledJobs.fields.everyMs')}
                  value={draft.everyMinutes}
                  onChange={(event) => {
                    const everyMinutes = event.currentTarget.value;
                    setDraft((current) => ({ ...current, everyMinutes }));
                  }}
                  placeholder="60"
                />
                <Input
                  type="datetime-local"
                  label={t('nav.scheduledJobs.fields.anchorMs')}
                  value={draft.anchorMs}
                  onChange={(event) => {
                    const anchorMs = event.currentTarget.value;
                    setDraft((current) => ({ ...current, anchorMs }));
                  }}
                  placeholder={t('nav.scheduledJobs.placeholders.anchorMs')}
                />
              </>
            ) : null}

            {draft.scheduleKind === 'cron' ? (
              <>
                <Input
                  label={t('nav.scheduledJobs.fields.cronExpr')}
                  value={draft.expr}
                  onChange={(event) => {
                    const expr = event.currentTarget.value;
                    setDraft((current) => ({ ...current, expr }));
                  }}
                  placeholder="0 8 * * *"
                />
                <Input
                  label={t('nav.scheduledJobs.fields.timezone')}
                  value={draft.tz}
                  onChange={(event) => {
                    const tz = event.currentTarget.value;
                    setDraft((current) => ({ ...current, tz }));
                  }}
                  placeholder={t('nav.scheduledJobs.placeholders.timezone')}
                />
              </>
            ) : null}
          </div>

          <Textarea
            label={t('nav.scheduledJobs.fields.prompt')}
            value={draft.text}
            onChange={(event) => {
              const text = event.currentTarget.value;
              setDraft((current) => ({ ...current, text }));
            }}
            autoResize
            showCount
            maxLength={4000}
            placeholder={t('nav.scheduledJobs.placeholders.prompt')}
            className="bitfun-scheduled-jobs-dialog__prompt"
          />
        </div>
      </div>
    </Modal>
  );
};

function resolveSessionLabel(session: Session): string {
  return session.title?.trim() || session.sessionId.slice(0, 8);
}

function jobToDraft(job: CronJob): JobDraft {
  const base = createEmptyDraft(job.workspacePath, job.sessionId);
  const draft: JobDraft = {
    ...base,
    name: job.name,
    text: job.payload.text,
    enabled: job.enabled,
  };

  if (job.schedule.kind === 'at') {
    draft.scheduleKind = 'at';
    draft.at = toLocalDateTimeInput(job.schedule.at);
  } else if (job.schedule.kind === 'every') {
    draft.scheduleKind = 'every';
    draft.everyMinutes = formatEveryMinutes(job.schedule.everyMs);
    draft.anchorMs = job.schedule.anchorMs != null
      ? timestampMsToLocalDateTimeInput(job.schedule.anchorMs)
      : '';
  } else {
    draft.scheduleKind = 'cron';
    draft.expr = job.schedule.expr;
    draft.tz = job.schedule.tz ?? '';
  }

  return draft;
}

function buildScheduleFromDraft(draft: JobDraft): CronSchedule {
  if (draft.scheduleKind === 'at') {
    if (!draft.at.trim()) {
      throw new Error('Please select a valid datetime.');
    }

    return {
      kind: 'at',
      at: new Date(draft.at).toISOString(),
    };
  }

  if (draft.scheduleKind === 'every') {
    const everyMinutes = Number(draft.everyMinutes);
    if (!Number.isFinite(everyMinutes) || everyMinutes <= 0) {
      throw new Error('Interval must be greater than 0 minutes.');
    }

    const anchorMs = draft.anchorMs.trim()
      ? new Date(draft.anchorMs).getTime()
      : undefined;
    if (anchorMs !== undefined && (!Number.isFinite(anchorMs) || anchorMs < 0)) {
      throw new Error('anchorMs must be a valid timestamp.');
    }

    return {
      kind: 'every',
      everyMs: Math.round(everyMinutes * MINUTE_IN_MS),
      anchorMs,
    };
  }

  if (!draft.expr.trim()) {
    throw new Error('Cron expression is required.');
  }

  return {
    kind: 'cron',
    expr: draft.expr.trim(),
    tz: draft.tz.trim() || undefined,
  };
}

function validateDraft(
  draft: JobDraft,
  t: (key: string, params?: Record<string, unknown>) => string
): string | null {
  if (!draft.name.trim()) {
    return t('nav.scheduledJobs.validation.nameRequired');
  }
  if (!draft.text.trim()) {
    return t('nav.scheduledJobs.validation.promptRequired');
  }
  if (!draft.workspacePath.trim()) {
    return t('nav.scheduledJobs.validation.workspaceRequired');
  }
  if (!draft.sessionId.trim()) {
    return t('nav.scheduledJobs.validation.sessionRequired');
  }
  return null;
}

function getNextExecutionAtMs(job: CronJob): number | null {
  return job.state.pendingTriggerAtMs
    ?? job.state.retryAtMs
    ?? job.state.nextRunAtMs
    ?? null;
}

function formatScheduleSummary(
  schedule: CronSchedule,
  t: (key: string, params?: Record<string, unknown>) => string
): string {
  switch (schedule.kind) {
    case 'at':
      return `${t('nav.scheduledJobs.scheduleKinds.at')}: ${formatTimestamp(new Date(schedule.at).getTime(), t)}`;
    case 'every':
      return t('nav.scheduledJobs.scheduleSummary.every', {
        everyMinutes: formatEveryMinutes(schedule.everyMs),
      });
    case 'cron':
      return schedule.tz
        ? t('nav.scheduledJobs.scheduleSummary.cronWithTz', { expr: schedule.expr, tz: schedule.tz })
        : t('nav.scheduledJobs.scheduleSummary.cron', { expr: schedule.expr });
    default:
      return '';
  }
}

function formatStatusLabel(
  job: CronJob,
  t: (key: string, params?: Record<string, unknown>) => string
): string {
  if (!job.enabled) {
    return t('nav.scheduledJobs.status.disabled');
  }

  const status = job.state.lastRunStatus;
  if (!status) {
    return t('nav.scheduledJobs.status.idle');
  }

  return t(`nav.scheduledJobs.status.${status}`);
}

function formatTimestamp(
  timestampMs: number | null | undefined,
  t: (key: string, params?: Record<string, unknown>) => string
): string {
  if (!timestampMs || !Number.isFinite(timestampMs)) {
    return t('nav.scheduledJobs.never');
  }

  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(timestampMs);
}

function toLocalDateTimeInput(isoTimestamp: string): string {
  const date = new Date(isoTimestamp);
  const timezoneOffset = date.getTimezoneOffset();
  const localDate = new Date(date.getTime() - timezoneOffset * 60_000);
  return localDate.toISOString().slice(0, 16);
}

function timestampMsToLocalDateTimeInput(timestampMs: number): string {
  return toLocalDateTimeInput(new Date(timestampMs).toISOString());
}

function getCurrentLocalDateTimeInput(): string {
  return toLocalDateTimeInput(new Date().toISOString());
}

function formatEveryMinutes(everyMs: number): string {
  const everyMinutes = everyMs / MINUTE_IN_MS;
  if (Number.isInteger(everyMinutes)) {
    return String(everyMinutes);
  }

  return everyMinutes.toFixed(2).replace(/\.?0+$/, '');
}

export default ScheduledJobsDialog;
