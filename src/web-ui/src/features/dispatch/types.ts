export type DispatchTargetRequest =
  | { kind: 'local' }
  | { kind: 'ssh'; connectionId: string; workspacePath: string }
  | { kind: 'device'; deviceId: string; workspacePath: string };

export type DispatchTarget =
  | { kind: 'local' }
  | {
      kind: 'ssh';
      connectionId: string;
      workspacePath: string;
      displayName: string;
    }
  | {
      kind: 'device';
      deviceId: string;
      workspacePath: string;
      displayName: string;
    };

export type DispatchApprovalPolicy = 'auto' | 'reject-and-report';
export type DispatchReachability = 'unknown' | 'reachable' | 'unreachable';
export type DispatchJobState =
  | 'submitting'
  | 'submission_unknown'
  | 'queued'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'cancelled';

export interface DispatchTargetOption {
  kind: 'local' | 'ssh';
  connectionId?: string;
  displayName: string;
  description?: string;
  defaultWorkspace?: string;
}

export interface DispatchCliRelease {
  version: string;
  target: string;
  url: string;
  sha256: string;
}

export interface DispatchWorkspaceProbe {
  path: string;
  exists: boolean;
  isDirectory: boolean;
  isGitRepository: boolean;
  branch?: string;
  dirty?: boolean;
  ahead?: number;
  behind?: number;
}

export interface DispatchProtocolProbe {
  protocolVersion: number;
  cliVersion: string;
  os: string;
  arch: string;
  capabilities: string[];
  modelConfigured: boolean;
  availableModels: string[];
  defaultModel?: string;
  modelDiagnostic?: string;
  workspace?: DispatchWorkspaceProbe;
}

export interface DispatchSshProbe {
  cliInstalled: boolean;
  cliPath?: string;
  os: string;
  arch: string;
  installSupported: boolean;
  installError?: string;
  protocolError?: string;
  release?: DispatchCliRelease;
  protocol?: DispatchProtocolProbe;
}

export interface DispatchInstallStart {
  scriptPath: string;
  version: string;
  target: string;
  url: string;
  sha256: string;
}

export interface DispatchInstallPoll {
  cursor: number;
  output: string;
  status: 'running' | 'succeeded' | 'failed';
}

export type DispatchEvent =
  | {
      type: 'audit';
      timestamp: string;
      action: string;
      details: Record<string, unknown>;
    }
  | {
      type: 'jobState';
      timestamp: string;
      state: Exclude<DispatchJobState, 'submitting'>;
      message?: string;
    }
  | {
      type: 'agentEvent';
      timestamp: string;
      event: DispatchAgentEventEnvelope | Record<string, unknown>;
      frontendEventName?: string;
      frontendPayload?: Record<string, unknown>;
    }
  | {
      type: 'permissionRejected';
      timestamp: string;
      request: Record<string, unknown>;
      reason: string;
    };

export interface DispatchAgentEventEnvelope {
  id?: string;
  event?: Record<string, unknown>;
  priority?: string | number;
  timestamp?: unknown;
  frontendEventName?: string;
  frontendPayload?: Record<string, unknown>;
}

export interface DispatchSubmitResponse {
  accepted: boolean;
  jobId: string;
  sessionId: string;
  state: Exclude<DispatchJobState, 'submitting'>;
}

export interface DispatchStatusResponse {
  state: Exclude<DispatchJobState, 'submitting'>;
  cursor: number;
  events: DispatchEvent[];
  pendingPermissions: Array<Record<string, unknown>>;
  cursorReset: boolean;
  lastError?: string;
}

export interface DispatchCancelResponse {
  cancelled: boolean;
}

export interface DispatchJobListEntry {
  jobId: string;
  sessionId: string;
  state: Exclude<DispatchJobState, 'submitting'>;
  startedAt?: string;
  workspacePath: string;
  title: string;
}

export interface OutboundDispatchRecord {
  jobId: string;
  target: DispatchTarget;
  sessionId: string;
  workspacePath: string;
  promptPreview: string;
  lastCursor: number;
  lastState: DispatchJobState;
  createdAt: string;
  updatedAt: string;
}

export interface DispatchSelection {
  request: Extract<DispatchTargetRequest, { kind: 'ssh' }>;
  target: Extract<DispatchTarget, { kind: 'ssh' }>;
  approvalPolicy: DispatchApprovalPolicy;
  model?: string;
}

export function isNonLocalDispatchTarget(
  target: DispatchTargetRequest | DispatchTarget | undefined,
): target is Exclude<DispatchTargetRequest | DispatchTarget, { kind: 'local' }> {
  return !!target && target.kind !== 'local';
}

export function isDispatchJobTerminal(state: DispatchJobState | undefined): boolean {
  return state === 'succeeded' || state === 'failed' || state === 'cancelled';
}
