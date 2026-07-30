import { api } from '@/infrastructure/api/service-api/ApiClient';
import type {
  DispatchApprovalPolicy,
  DispatchCancelResponse,
  DispatchCliRelease,
  DispatchInstallPoll,
  DispatchInstallStart,
  DispatchJobListEntry,
  DispatchSshProbe,
  DispatchStatusResponse,
  DispatchSubmitResponse,
  DispatchTargetOption,
  DispatchTargetRequest,
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
};
