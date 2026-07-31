import { api } from '@/infrastructure/api/service-api/ApiClient';
import type {
  DispatchApprovalPolicy,
  DispatchCancelResponse,
  DispatchCliRelease,
  DispatchInstallPoll,
  DispatchInstallStart,
  DispatchJobListEntry,
  DispatchResultApplyOutcome,
  DispatchResultBundle,
  DispatchSshProbe,
  DispatchStatusResponse,
  DispatchSubmitResponse,
  DispatchTargetOption,
  DispatchTargetRequest,
  DispatchTranscriptCache,
  DispatchWorkspaceDeliveryRequest,
  OutboundDispatchRecord,
} from './types';

export const dispatchApi = {
  async listTargets(): Promise<DispatchTargetOption[]> {
    return api.invoke<DispatchTargetOption[]>('dispatch_list_targets', {
      request: {},
    });
  },

  async probeTarget(target: DispatchTargetRequest): Promise<DispatchSshProbe> {
    return api.invoke<DispatchSshProbe>('dispatch_probe_target', {
      request: { target },
    });
  },

  async installCliStart(
    connectionId: string,
    release: DispatchCliRelease,
  ): Promise<DispatchInstallStart> {
    return api.invoke<DispatchInstallStart>('dispatch_install_cli_start', {
      request: { connectionId, release },
    });
  },

  /** Build the CLI from source on the target, for hosts no binary fits. */
  async installCliSourceStart(connectionId: string): Promise<DispatchInstallStart> {
    return api.invoke<DispatchInstallStart>('dispatch_install_cli_source_start', {
      request: { connectionId },
    });
  },

  async installCliPoll(connectionId: string, cursor: number): Promise<DispatchInstallPoll> {
    return api.invoke<DispatchInstallPoll>('dispatch_install_cli_poll', {
      request: { connectionId, cursor },
    });
  },

  async installCliCancel(connectionId: string): Promise<void> {
    return api.invoke<void>('dispatch_install_cli_cancel', {
      request: { connectionId },
    });
  },

  /**
   * Download what a finished snapshot job changed on its target.
   *
   * Fetch and report only: the bundle lands in the controller's staging area
   * and nothing reaches the local workspace until the user reviews the diff
   * and explicitly applies it.
   */
  async pullResult(jobId: string): Promise<DispatchResultBundle> {
    return api.invoke<DispatchResultBundle>('dispatch_pull_result', {
      request: { jobId },
    });
  },

  /**
   * Apply a pulled bundle to a local workspace.
   *
   * Aborts without writing when a path changed on both sides, unless
   * `overwriteConflicts` says to take the target's version.
   */
  async applyResult(
    jobId: string,
    workspacePath: string,
    overwriteConflicts: boolean,
  ): Promise<DispatchResultApplyOutcome> {
    return api.invoke<DispatchResultApplyOutcome>('dispatch_apply_result', {
      request: { jobId, workspacePath, overwriteConflicts },
    });
  },

  async syncModelConfig(connectionId: string): Promise<void> {
    return api.invoke<void>('dispatch_sync_model_config', {
      request: { connectionId },
    });
  },

  async submit(request: {
    target: DispatchTargetRequest;
    workspaceDelivery: DispatchWorkspaceDeliveryRequest;
    jobId: string;
    sessionId: string;
    agentType: string;
    prompt: string;
    approvalPolicy: DispatchApprovalPolicy;
    model?: string;
    title?: string;
    sourceWorkspacePath?: string;
    sourceWorkspaceId?: string;
  }): Promise<DispatchSubmitResponse> {
    return api.invoke<DispatchSubmitResponse>('dispatch_submit', {
      request,
    });
  },

  async status(jobId: string, cursor: number): Promise<DispatchStatusResponse> {
    return api.invoke<DispatchStatusResponse>('dispatch_status', {
      request: { jobId, cursor },
    });
  },

  async cancel(jobId: string): Promise<DispatchCancelResponse> {
    return api.invoke<DispatchCancelResponse>('dispatch_cancel', {
      request: { jobId },
    });
  },

  async answerPermission(
    jobId: string,
    requestId: string,
    reply: 'once' | 'always' | 'reject',
    feedback?: string,
  ): Promise<{ resolved: boolean }> {
    return api.invoke<{ resolved: boolean }>('dispatch_answer', {
      request: {
        jobId,
        requestId,
        reply,
        ...(feedback?.trim() ? { feedback: feedback.trim() } : {}),
      },
    });
  },

  async append(
    jobId: string,
    content: string,
    displayContent?: string,
    messageId: string = globalThis.crypto?.randomUUID?.()
      ?? `dispatch-message-${Date.now()}-${Math.random().toString(36).slice(2)}`,
  ): Promise<{ accepted: boolean; messageId: string }> {
    return api.invoke<{ accepted: boolean; messageId: string }>('dispatch_append', {
      request: {
        jobId,
        messageId,
        content,
        ...(displayContent?.trim() ? { displayContent } : {}),
      },
    });
  },

  async listJobs(): Promise<OutboundDispatchRecord[]> {
    return api.invoke<OutboundDispatchRecord[]>('dispatch_list_jobs', {
      request: {},
    });
  },

  async listTargetJobs(target: DispatchTargetRequest): Promise<DispatchJobListEntry[]> {
    return api.invoke<DispatchJobListEntry[]>('dispatch_list_jobs', {
      request: { target },
    });
  },

  /**
   * Read this controller's cached observer transcript for a job.
   *
   * Controller-local only. Returns `null` when nothing is cached, which sends
   * the observer back to replaying the target's event log from byte zero.
   */
  async loadTranscript(jobId: string): Promise<DispatchTranscriptCache | null> {
    return api.invoke<DispatchTranscriptCache | null>('dispatch_load_transcript', {
      request: { jobId },
    });
  },

  /**
   * Persist this controller's observer transcript for a job, or pass `null` to
   * erase it.
   *
   * Resolves `false` when the transcript is above the controller's cache
   * ceiling; the previous entry is kept and observing continues unchanged.
   */
  async saveTranscript(
    jobId: string,
    transcript: DispatchTranscriptCache | null,
  ): Promise<boolean> {
    return api.invoke<boolean>('dispatch_save_transcript', {
      request: { jobId, transcript },
    });
  },
};
