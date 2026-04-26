/**
 * AssistantScheduleView — inline view for managing scheduled jobs of an assistant.
 *
 * Redesigned for the 40%-wide right panel: single-column layout,
 * job list at top, inline editor expands below selected job.
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { RefreshCw, Trash2 } from 'lucide-react';
import {
  Button,
  IconButton,
  Input,
  Modal,
  Select,
  Switch,
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
import { i18nService } from '@/infrastructure/i18n';
import { resolveSessionTitle } from '@/flow_chat/utils/sessionTitle';
import './AssistantScheduleView.scss';

const log = createLogger('AssistantScheduleView');
const MINUTE_IN_MS = 60_000;

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

export interface AssistantScheduleViewProps {
  workspacePath?: string;
  sessionId?: string;
  assistantName?: string;
}

function getCurrentLocalDateTimeInput(): string {
  return toLocalDateTimeInput(new Date().toISOString());
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

function formatEveryMinutes(everyMs: number): string {
  const everyMinutes = everyMs / MINUTE_IN_MS;
  if (Number.isInteger(everyMinutes)) return String(everyMinutes);
  return everyMinutes.toFixed(2).replace(/\.?0+$/, '');
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

function jobToDraft(job: CronJob): JobDraft {
  const base = createEmptyDraft(job.workspacePath, job.sessionId);
  const draft: JobDraft = { ...base, name: job.name, text: job.payload.text, enabled: job.enabled };
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
    if (!draft.at.trim()) throw new Error('Please select a valid datetime.');
    return { kind: 'at', at: new Date(draft.at).toISOString() };
  }
  if (draft.scheduleKind === 'every') {
    const everyMinutes = Number(draft.everyMinutes);
    if (!Number.isFinite(everyMinutes) || everyMinutes <= 0) {
      throw new Error('Interval must be greater than 0 minutes.');
    }
    const anchorMs = draft.anchorMs.trim() ? new Date(draft.anchorMs).getTime() : undefined;
    return { kind: 'every', everyMs: Math.round(everyMinutes * MINUTE_IN_MS), anchorMs };
  }
  if (!draft.expr.trim()) throw new Error('Cron expression is required.');
  return { kind: 'cron', expr: draft.expr.trim(), tz: draft.tz.trim() || undefined };
}

function validateDraft(
  draft: JobDraft,
  t: (key: string, params?: Record<string, unknown>) => string,
): string | null {
  if (!draft.name.trim()) return t('nav.scheduledJobs.validation.nameRequired');
  if (!draft.text.trim()) return t('nav.scheduledJobs.validation.promptRequired');
  if (!draft.workspacePath.trim()) return t('nav.scheduledJobs.validation.workspaceRequired');
  if (!draft.sessionId.trim()) return t('nav.scheduledJobs.validation.sessionRequired');
  return null;
}

function getNextExecutionAtMs(job: CronJob): number | null {
  return job.state.pendingTriggerAtMs ?? job.state.retryAtMs ?? job.state.nextRunAtMs ?? null;
}

function formatScheduleSummary(
  schedule: CronSchedule,
  t: (key: string, params?: Record<string, unknown>) => string,
): string {
  switch (schedule.kind) {
    case 'at':
      return `${t('nav.scheduledJobs.scheduleKinds.at')}: ${formatTimestamp(new Date(schedule.at).getTime(), t)}`;
    case 'every':
      return t('nav.scheduledJobs.scheduleSummary.every', { everyMinutes: formatEveryMinutes(schedule.everyMs) });
    case 'cron':
      return schedule.tz
        ? t('nav.scheduledJobs.scheduleSummary.cronWithTz', { expr: schedule.expr, tz: schedule.tz })
        : t('nav.scheduledJobs.scheduleSummary.cron', { expr: schedule.expr });
    default:
      return '';
  }
}

function formatTimestamp(
  timestampMs: number | null | undefined,
  t: (key: string, params?: Record<string, unknown>) => string,
): string {
  if (!timestampMs || !Number.isFinite(timestampMs)) return t('nav.scheduledJobs.never');
  return new Intl.DateTimeFormat(undefined, {
    month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit',
  }).format(timestampMs);
}

function resolveSessionLabel(session: Session): string {
  return resolveSessionTitle(session, (key, options) => i18nService.t(key, options));
}

const AssistantScheduleView: React.FC<AssistantScheduleViewProps> = ({
  workspacePath,
  sessionId,
}) => {
  const { t } = useI18n('common');
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const [jobs, setJobs] = useState<CronJob[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [draft, setDraft] = useState<JobDraft>(() =>
    createEmptyDraft(workspacePath ?? '', sessionId ?? ''),
  );

  useEffect(() => {
    const unsubscribe = flowChatStore.subscribe((state) => setFlowChatState(state));
    return unsubscribe;
  }, []);

  const workspaceSessions = useMemo(() => {
    const wp = workspacePath?.trim() ?? '';
    if (!wp) return [] as Session[];
    return Array.from(flowChatState.sessions.values())
      .filter(s => (s.workspacePath || wp) === wp && !s.parentSessionId)
      .sort(compareSessionsForDisplay);
  }, [workspacePath, flowChatState.sessions]);

  const defaultSessionIdForWorkspace = useMemo(
    () => workspaceSessions[0]?.sessionId ?? '',
    [workspaceSessions],
  );

  const sortedJobs = useMemo(() => [...jobs].sort((a, b) => {
    if (a.enabled !== b.enabled) return a.enabled ? -1 : 1;
    const diff = b.configUpdatedAtMs - a.configUpdatedAtMs;
    return diff !== 0 ? diff : b.createdAtMs - a.createdAtMs;
  }), [jobs]);

  const loadJobs = useCallback(async () => {
    setLoading(true);
    try {
      const result = await cronAPI.listJobs({ workspacePath: workspacePath || undefined });
      setJobs(result);
      setSelectedJobId(current => {
        if (current && result.some(j => j.id === current)) return current;
        return null;
      });
    } catch (error) {
      log.error('Failed to load scheduled jobs', { error });
      notificationService.error(
        t('nav.scheduledJobs.messages.loadFailed', {
          error: error instanceof Error ? error.message : String(error),
        }),
      );
    } finally {
      setLoading(false);
    }
  }, [workspacePath, t]);

  useEffect(() => { void loadJobs(); }, [loadJobs]);

  useEffect(() => {
    setDraft(prev => ({
      ...prev,
      workspacePath: workspacePath ?? '',
      sessionId: sessionId || prev.sessionId || defaultSessionIdForWorkspace,
    }));
  }, [workspacePath, sessionId, defaultSessionIdForWorkspace]);

  const handleCreateNew = useCallback(() => {
    setSelectedJobId(null);
    setDraft(createEmptyDraft(workspacePath ?? '', sessionId || defaultSessionIdForWorkspace));
    setModalOpen(true);
  }, [workspacePath, sessionId, defaultSessionIdForWorkspace]);

  const handleEditJob = useCallback((job: CronJob) => {
    setSelectedJobId(job.id);
    setDraft(jobToDraft(job));
    setModalOpen(true);
  }, []);

  const handleCloseModal = useCallback(() => {
    setModalOpen(false);
  }, []);

  const handleDeleteJob = useCallback(async (job: CronJob) => {
    const confirmed = await confirmDanger(
      t('nav.scheduledJobs.deleteDialog.title', { name: job.name }),
      null,
    );
    if (!confirmed) return;
    try {
      await cronAPI.deleteJob(job.id);
      notificationService.success(t('nav.scheduledJobs.messages.deleteSuccess'));
      if (selectedJobId === job.id) { setSelectedJobId(null); setModalOpen(false); }
      await loadJobs();
    } catch (error) {
      log.error('Failed to delete scheduled job', { jobId: job.id, error });
      notificationService.error(
        t('nav.scheduledJobs.messages.deleteFailed', {
          error: error instanceof Error ? error.message : String(error),
        }),
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
        }),
      );
    }
  }, [loadJobs, t]);

  const handleSave = useCallback(async () => {
    const validationError = validateDraft(draft, t);
    if (validationError) { notificationService.error(validationError); return; }
    let schedule: CronSchedule;
    try { schedule = buildScheduleFromDraft(draft); } catch (error) {
      notificationService.error(error instanceof Error ? error.message : String(error));
      return;
    }
    setSaving(true);
    try {
      if (selectedJobId) {
        const request: UpdateCronJobRequest = {
          name: draft.name.trim(),
          payload: { text: draft.text.trim() },
          enabled: draft.enabled,
          schedule,
          workspacePath: draft.workspacePath.trim(),
          sessionId: draft.sessionId.trim(),
        };
        const updated = await cronAPI.updateJob(selectedJobId, request);
        setSelectedJobId(updated.id);
        setDraft(jobToDraft(updated));
        notificationService.success(t('nav.scheduledJobs.messages.updateSuccess'));
        setModalOpen(false);
      } else {
        const request: CreateCronJobRequest = {
          name: draft.name.trim(),
          payload: { text: draft.text.trim() },
          enabled: draft.enabled,
          schedule,
          workspacePath: draft.workspacePath.trim(),
          sessionId: draft.sessionId.trim(),
        };
        const created = await cronAPI.createJob(request);
        setSelectedJobId(created.id);
        setDraft(jobToDraft(created));
        notificationService.success(t('nav.scheduledJobs.messages.createSuccess'));
        setModalOpen(false);
      }
      await loadJobs();
    } catch (error) {
      log.error('Failed to save scheduled job', { error });
      notificationService.error(
        t('nav.scheduledJobs.messages.saveFailed', {
          error: error instanceof Error ? error.message : String(error),
        }),
      );
    } finally {
      setSaving(false);
    }
  }, [draft, loadJobs, selectedJobId, t]);

  const sessionOptions = useMemo(
    () => workspaceSessions.map(s => ({
      value: s.sessionId,
      label: resolveSessionLabel(s),
      description: s.title || s.sessionId,
    })),
    [workspaceSessions],
  );

  const canSave = Boolean(draft.workspacePath.trim() && draft.sessionId.trim());

  const modalTitle = selectedJobId
    ? t('nav.scheduledJobs.editor.editTitle')
    : t('nav.scheduledJobs.editor.createTitle');

  return (
    <div className="asv">
      {/* ── Header ── */}
      <div className="asv__head">
        <span className="asv__head-title">{t('nav.scheduledJobs.title')}</span>
        <Button
          type="button"
          size="small"
          variant="secondary"
          className="asv__new-job"
          onClick={handleCreateNew}
          disabled={!defaultSessionIdForWorkspace}
        >
          {t('nav.scheduledJobs.actions.newJob')}
        </Button>
      </div>

      {/* ── Job list ── */}
      {loading ? (
        <div className="asv__empty">
          <RefreshCw size={14} className="asv__spin" />
        </div>
      ) : sortedJobs.length === 0 ? (
        <div className="asv__empty">
          <p className="asv__empty-title">{t('nav.scheduledJobs.empty.title')}</p>
          <p className="asv__empty-text">{t('nav.scheduledJobs.empty.description')}</p>
        </div>
      ) : (
        <div className="asv__list">
          {sortedJobs.map(job => (
            <div
              key={job.id}
              className="asv__item"
              role="group"
              tabIndex={0}
              aria-label={`${job.name}, ${t('nav.scheduledJobs.actions.edit')}`}
              onClick={() => handleEditJob(job)}
              onKeyDown={e => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  handleEditJob(job);
                }
              }}
            >
              <div className="asv__item-body">
                <div className="asv__item-top">
                  <span className="asv__item-name">{job.name}</span>
                </div>
                <div className="asv__item-meta">
                  {formatScheduleSummary(job.schedule, t)}
                </div>
                <div className="asv__item-meta asv__item-meta--dim">
                  {t('nav.scheduledJobs.nextRunLabel')}: {formatTimestamp(getNextExecutionAtMs(job), t)}
                </div>
                {job.state.lastError ? (
                  <div className="asv__item-error">{job.state.lastError}</div>
                ) : null}
              </div>
              <div className="asv__item-actions">
                <div
                  className="asv__switch-wrap"
                  onClick={e => e.stopPropagation()}
                  role="presentation"
                >
                  <Switch
                    size="small"
                    checked={job.enabled}
                    onChange={e => {
                      void handleToggleEnabled(job, e.currentTarget.checked);
                    }}
                    aria-label={t('nav.scheduledJobs.actions.toggleEnabled')}
                  />
                </div>
                <IconButton
                  type="button"
                  size="xs"
                  variant="danger"
                  aria-label={t('nav.scheduledJobs.actions.delete')}
                  tooltip={t('nav.scheduledJobs.actions.delete')}
                  onClick={e => { e.stopPropagation(); void handleDeleteJob(job); }}
                >
                  <Trash2 size={13} />
                </IconButton>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* ── Edit / Create modal ── */}
      <Modal
        isOpen={modalOpen}
        onClose={handleCloseModal}
        title={modalTitle}
        size="medium"
        contentInset
      >
        {renderForm()}
      </Modal>
    </div>
  );

  function renderForm() {
    return (
      <div className="asv__form">
        {!canSave && (
          <p className="asv__warning">{t('nav.scheduledJobs.messages.sessionRequired')}</p>
        )}

        <div className="asv__form-row">
          <Input
            label={t('nav.scheduledJobs.fields.name')}
            value={draft.name}
            onChange={e => {
              const name = e.currentTarget.value;
              setDraft(c => ({ ...c, name }));
            }}
            placeholder={t('nav.scheduledJobs.placeholders.name')}
          />
        </div>

        <div className="asv__form-row asv__form-row--two">
          <Select
            label={t('nav.scheduledJobs.fields.scheduleKind')}
            value={draft.scheduleKind}
            options={[
              { value: 'at', label: t('nav.scheduledJobs.scheduleKinds.at') },
              { value: 'every', label: t('nav.scheduledJobs.scheduleKinds.every') },
              { value: 'cron', label: t('nav.scheduledJobs.scheduleKinds.cron') },
            ]}
            onChange={value => {
              setDraft(c => ({
                ...c,
                scheduleKind: value as ScheduleKind,
                at: (value as ScheduleKind) === 'at' && !c.at.trim() ? getCurrentLocalDateTimeInput() : c.at,
              }));
            }}
          />

          <div className="asv__field asv__field--switch">
            <span className="asv__field-label">{t('nav.scheduledJobs.fields.enabled')}</span>
            <Switch
              size="small"
              checked={draft.enabled}
              onChange={e => {
                const enabled = e.currentTarget.checked;
                setDraft(c => ({ ...c, enabled }));
              }}
              aria-label={t('nav.scheduledJobs.fields.enabled')}
            />
          </div>
        </div>

        {draft.scheduleKind === 'at' && (
          <div className="asv__form-row">
            <Input
              type="datetime-local"
              label={t('nav.scheduledJobs.fields.at')}
              value={draft.at}
              onChange={e => {
                const at = e.currentTarget.value;
                setDraft(c => ({ ...c, at }));
              }}
            />
          </div>
        )}

        {draft.scheduleKind === 'every' && (
          <div className="asv__form-row asv__form-row--two">
            <Input
              type="number"
              label={t('nav.scheduledJobs.fields.everyMs')}
              value={draft.everyMinutes}
              onChange={e => {
                const everyMinutes = e.currentTarget.value;
                setDraft(c => ({ ...c, everyMinutes }));
              }}
              placeholder="60"
            />
            <Input
              type="datetime-local"
              label={t('nav.scheduledJobs.fields.anchorMs')}
              value={draft.anchorMs}
              onChange={e => {
                const anchorMs = e.currentTarget.value;
                setDraft(c => ({ ...c, anchorMs }));
              }}
              placeholder={t('nav.scheduledJobs.placeholders.anchorMs')}
            />
          </div>
        )}

        {draft.scheduleKind === 'cron' && (
          <div className="asv__form-row asv__form-row--two">
            <Input
              label={t('nav.scheduledJobs.fields.cronExpr')}
              value={draft.expr}
              onChange={e => {
                const expr = e.currentTarget.value;
                setDraft(c => ({ ...c, expr }));
              }}
              placeholder="0 8 * * *"
            />
            <Input
              label={t('nav.scheduledJobs.fields.timezone')}
              value={draft.tz}
              onChange={e => {
                const tz = e.currentTarget.value;
                setDraft(c => ({ ...c, tz }));
              }}
              placeholder={t('nav.scheduledJobs.placeholders.timezone')}
            />
          </div>
        )}

        <div className="asv__form-row">
          <Select
            label={t('nav.scheduledJobs.fields.session')}
            options={sessionOptions}
            value={draft.sessionId}
            allowCustomValue
            searchable
            onChange={value => setDraft(c => ({ ...c, sessionId: String(value) }))}
            placeholder={t('nav.scheduledJobs.placeholders.session')}
          />
        </div>

        <div className="asv__form-row">
          <Textarea
            label={t('nav.scheduledJobs.fields.prompt')}
            value={draft.text}
            onChange={e => {
              const text = e.currentTarget.value;
              setDraft(c => ({ ...c, text }));
            }}
            autoResize
            showCount
            maxLength={4000}
            placeholder={t('nav.scheduledJobs.placeholders.prompt')}
          />
        </div>

        <div className="asv__form-actions">
          <Button
            variant="ghost"
            onClick={handleCloseModal}
          >
            {t('nav.scheduledJobs.actions.cancel')}
          </Button>
          <Button
            variant="primary"
            onClick={() => { void handleSave(); }}
            disabled={!canSave}
            isLoading={saving}
          >
            {selectedJobId
              ? t('nav.scheduledJobs.actions.save')
              : t('nav.scheduledJobs.actions.create')}
          </Button>
        </div>
      </div>
    );
  }
};

export default AssistantScheduleView;
