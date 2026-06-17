import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export type CronJobRunStatus = 'queued' | 'running' | 'ok' | 'error' | 'cancelled';
export type CronJobTargetKind = 'session' | 'workspace';

export type CronSchedule =
  | {
    kind: 'at';
    at: string;
  }
  | {
    kind: 'every';
    everyMs: number;
    anchorMs?: number | null;
  }
  | {
    kind: 'cron';
    expr: string;
    tz?: string | null;
  };

export interface CronJobPayload {
  text: string;
}

export interface CronWorkspaceRef {
  workspaceId?: string | null;
  workspacePath: string;
  remoteConnectionId?: string | null;
  remoteSshHost?: string | null;
}

export interface CronLaunchSpec {
  agentType: string;
  modelId?: string | null;
}

export type CronJobTarget =
  | {
    kind: 'session';
    sessionId: string;
    workspace: CronWorkspaceRef;
  }
  | {
    kind: 'workspace';
    workspace: CronWorkspaceRef;
    launch: CronLaunchSpec;
  };

export interface CronJobState {
  nextRunAtMs?: number | null;
  pendingTriggerAtMs?: number | null;
  retryAtMs?: number | null;
  lastTriggerAtMs?: number | null;
  lastEnqueuedAtMs?: number | null;
  lastRunStartedAtMs?: number | null;
  lastRunFinishedAtMs?: number | null;
  lastDurationMs?: number | null;
  lastRunStatus?: CronJobRunStatus | null;
  lastError?: string | null;
  activeTurnId?: string | null;
  consecutiveFailures: number;
  coalescedRunCount: number;
}

export interface CronJob {
  id: string;
  name: string;
  schedule: CronSchedule;
  payload: CronJobPayload;
  enabled: boolean;
  target: CronJobTarget;
  createdAtMs: number;
  configUpdatedAtMs: number;
  updatedAtMs: number;
  state: CronJobState;
}

export interface ListCronJobsRequest {
  workspacePath?: string;
  workspaceId?: string;
  remoteConnectionId?: string;
  sessionId?: string;
  targetKind?: CronJobTargetKind;
}

export interface CreateCronJobRequest {
  name: string;
  schedule: CronSchedule;
  payload: CronJobPayload;
  enabled?: boolean;
  target: CronJobTarget;
}

export interface UpdateCronJobRequest {
  name?: string;
  schedule?: CronSchedule;
  payload?: CronJobPayload;
  enabled?: boolean;
  target?: CronJobTarget;
}

export class CronAPI {
  async listJobs(request: ListCronJobsRequest = {}): Promise<CronJob[]> {
    try {
      return await api.invoke<CronJob[]>('list_cron_jobs', { request });
    } catch (error) {
      throw createTauriCommandError('list_cron_jobs', error, request);
    }
  }

  async createJob(request: CreateCronJobRequest): Promise<CronJob> {
    try {
      return await api.invoke<CronJob>('create_cron_job', { request });
    } catch (error) {
      throw createTauriCommandError('create_cron_job', error, request);
    }
  }

  async updateJob(jobId: string, changes: UpdateCronJobRequest): Promise<CronJob> {
    try {
      return await api.invoke<CronJob>('update_cron_job', {
        request: {
          jobId,
          ...changes,
        },
      });
    } catch (error) {
      throw createTauriCommandError('update_cron_job', error, { jobId, ...changes });
    }
  }

  async deleteJob(jobId: string): Promise<boolean> {
    try {
      return await api.invoke<boolean>('delete_cron_job', {
        request: { jobId },
      });
    } catch (error) {
      throw createTauriCommandError('delete_cron_job', error, { jobId });
    }
  }
}

export const cronAPI = new CronAPI();
