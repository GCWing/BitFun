/**
 * Pure draft <-> CronJob conversion helpers shared by every scheduled-job editor.
 *
 * These are deliberately free of React and i18n so both the per-workspace
 * `ScheduledJobsView` and the global Todos scene can build the same request
 * shapes from the same form state.
 */

import type {
  CronJob,
  CronJobTarget,
  CronJobTargetKind,
  CronSchedule,
  CronWorkspaceRef,
} from '@/infrastructure/api';
import { normalizePath } from '@/shared/utils/pathUtils';

export const MINUTE_IN_MS = 60_000;
export const DEFAULT_AGENT_TYPE = 'agentic';
export const ASSISTANT_WORKSPACE_AGENT_TYPE = 'Claw';

/** Emitted after any create/update/delete so other mounted views reload. */
export const SCHEDULED_JOBS_CHANGED_EVENT = 'bitfun:scheduled-jobs-changed';

export type ScheduleKind = CronSchedule['kind'];

export interface JobDraft {
  name: string;
  text: string;
  enabled: boolean;
  sessionId: string;
  agentType: string;
  scheduleKind: ScheduleKind;
  at: string;
  everyMinutes: string;
  anchorMs: string;
  expr: string;
  tz: string;
}

export interface JobDraftValidationErrors {
  name: boolean;
  sessionId: boolean;
  agentType: boolean;
  text: boolean;
  at: boolean;
  everyMinutes: boolean;
  cronExpr: boolean;
}

/** Shared reset value. Frozen because callers hold the same reference. */
export const EMPTY_VALIDATION_ERRORS: JobDraftValidationErrors = Object.freeze({
  name: false,
  sessionId: false,
  agentType: false,
  text: false,
  at: false,
  everyMinutes: false,
  cronExpr: false,
});

export function toLocalDateTimeInput(isoTimestamp: string): string {
  const date = new Date(isoTimestamp);
  const timezoneOffset = date.getTimezoneOffset();
  const localDate = new Date(date.getTime() - timezoneOffset * MINUTE_IN_MS);
  return localDate.toISOString().slice(0, 16);
}

export function getCurrentLocalDateTimeInput(): string {
  return toLocalDateTimeInput(new Date().toISOString());
}

export function timestampMsToLocalDateTimeInput(timestampMs: number): string {
  return toLocalDateTimeInput(new Date(timestampMs).toISOString());
}

export function isFutureLocalDateTimeInput(value: string, nowMs = Date.now()): boolean {
  const timestampMs = new Date(value).getTime();
  return Number.isFinite(timestampMs) && timestampMs > nowMs;
}

export function formatEveryMinutes(everyMs: number): string {
  const everyMinutes = everyMs / MINUTE_IN_MS;
  if (Number.isInteger(everyMinutes)) return String(everyMinutes);
  return everyMinutes.toFixed(2).replace(/\.?0+$/, '');
}

export function createEmptyDraft(
  defaultSessionId = '',
  defaultAgentType = DEFAULT_AGENT_TYPE,
): JobDraft {
  return {
    name: '',
    text: '',
    enabled: true,
    sessionId: defaultSessionId,
    agentType: defaultAgentType,
    scheduleKind: 'at',
    at: getCurrentLocalDateTimeInput(),
    everyMinutes: '60',
    anchorMs: '',
    expr: '0 8 * * *',
    tz: '',
  };
}

export function jobToDraft(job: CronJob, defaultAgentType: string): JobDraft {
  const base = createEmptyDraft('', defaultAgentType);
  const draft: JobDraft = {
    ...base,
    name: job.name,
    text: job.payload.text,
    enabled: job.enabled,
  };
  if (job.target.kind === 'session') {
    draft.sessionId = job.target.sessionId;
  } else {
    draft.agentType = job.target.launch.agentType || defaultAgentType;
  }
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

export function buildScheduleFromDraft(draft: JobDraft): CronSchedule {
  if (draft.scheduleKind === 'at') {
    return { kind: 'at', at: new Date(draft.at).toISOString() };
  }
  if (draft.scheduleKind === 'every') {
    const everyMinutes = Number(draft.everyMinutes);
    const anchorMs = draft.anchorMs.trim() ? new Date(draft.anchorMs).getTime() : undefined;
    return { kind: 'every', everyMs: Math.round(everyMinutes * MINUTE_IN_MS), anchorMs };
  }
  return { kind: 'cron', expr: draft.expr.trim(), tz: draft.tz.trim() || undefined };
}

export function buildWorkspaceRef(
  workspacePath?: string,
  workspaceId?: string,
  remoteConnectionId?: string | null,
  remoteSshHost?: string | null,
): CronWorkspaceRef | null {
  const normalizedWorkspacePath = normalizePath(workspacePath?.trim() ?? '');
  if (!normalizedWorkspacePath) {
    return null;
  }

  return {
    workspacePath: normalizedWorkspacePath,
    workspaceId: workspaceId?.trim() || undefined,
    remoteConnectionId: remoteConnectionId?.trim() || undefined,
    remoteSshHost: remoteSshHost?.trim() || undefined,
  };
}

export function buildTargetFromDraft(
  targetKind: CronJobTargetKind,
  draft: JobDraft,
  workspace: CronWorkspaceRef,
): CronJobTarget {
  if (targetKind === 'session') {
    return {
      kind: 'session',
      sessionId: draft.sessionId.trim(),
      workspace,
    };
  }

  return {
    kind: 'workspace',
    workspace,
    launch: {
      agentType: draft.agentType.trim() || DEFAULT_AGENT_TYPE,
    },
  };
}

export function validateDraft(
  targetKind: CronJobTargetKind,
  draft: JobDraft,
): JobDraftValidationErrors {
  const everyMinutes = Number(draft.everyMinutes);
  return {
    name: !draft.name.trim(),
    sessionId: targetKind === 'session' && !draft.sessionId.trim(),
    agentType: targetKind === 'workspace' && !draft.agentType.trim(),
    text: !draft.text.trim(),
    at: draft.scheduleKind === 'at' && !draft.at.trim(),
    everyMinutes:
      draft.scheduleKind === 'every'
      && (!draft.everyMinutes.trim() || !Number.isFinite(everyMinutes) || everyMinutes <= 0),
    cronExpr: draft.scheduleKind === 'cron' && !draft.expr.trim(),
  };
}

export function hasValidationErrors(errors: JobDraftValidationErrors): boolean {
  return (
    errors.name
    || errors.sessionId
    || errors.agentType
    || errors.text
    || errors.at
    || errors.everyMinutes
    || errors.cronExpr
  );
}

/** Timestamp the scheduler will actually act on next, or null when nothing is pending. */
export function getNextExecutionAtMs(job: CronJob): number | null {
  return job.state.pendingTriggerAtMs ?? job.state.retryAtMs ?? job.state.nextRunAtMs ?? null;
}

export function notifyScheduledJobsChanged(sourceId: string): void {
  window.dispatchEvent(new CustomEvent(SCHEDULED_JOBS_CHANGED_EVENT, {
    detail: { sourceId },
  }));
}
