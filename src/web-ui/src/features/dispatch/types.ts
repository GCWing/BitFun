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

export type DispatchApprovalPolicy = 'auto' | 'reject-and-report' | 'remote';

/**
 * How a dispatch reaches its target: a Git worktree of the controller's own
 * repository, checked out on the target at the same commit.
 *
 * Recorded on the session so the sync button knows where to fetch the target's
 * branch back into. It is resolved by the backend at submit time, not chosen in
 * the UI — the UI only decides `includeUncommitted`.
 */
export interface DispatchWorkspaceDelivery {
  sourceWorkspacePath: string;
  baselineWorktreeId: string;
  baseCommit: string;
  branch: string;
  remoteUrl?: string;
  includeUncommitted: boolean;
}
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
  kind: 'local' | 'ssh' | 'device';
  connectionId?: string;
  deviceId?: string;
  displayName: string;
  description?: string;
  online?: boolean;
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
  /** Set when no published binary can run on this target, with the reason. */
  prebuiltIncompatible?: string;
  /** Offered as the way forward when a prebuilt install cannot work. */
  sourceBuild?: DispatchSourceBuild;
}

export interface DispatchSyncedChange {
  /** Git name-status letter: `A`, `M`, `D`, and so on. */
  status: string;
  path: string;
}

/**
 * Outcome of one sync-back.
 *
 * `changed: false` means the target's worktree still matches `baseCommit` — no
 * bundle was built and nothing was fetched.
 */
export interface DispatchSyncResult {
  changed: boolean;
  branch: string;
  baseCommit: string;
  headCommit: string;
  commitCount: number;
  changes: DispatchSyncedChange[];
  /** True when the change list was capped; the fetched history is complete. */
  truncatedChanges: boolean;
  /** Controller worktree the branch was fast-forwarded into. */
  baselineWorktreePath?: string;
  syncedHeadCommit?: string;
}

export interface DispatchSourceBuild {
  supported: boolean;
  /** What the user must install or free up first. Empty when supported. */
  blockers: string[];
  cargoVersion?: string;
  gitRef: string;
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
  historyTruncated: boolean;
  eventLogComplete: boolean;
  omittedEventCount: number;
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
  agentType?: string;
  approvalPolicy?: DispatchApprovalPolicy;
  model?: string;
}

export interface OutboundDispatchRecord {
  jobId: string;
  target: DispatchTarget;
  sessionId: string;
  /** Controller workspace that owns the observer session. */
  sourceWorkspacePath?: string;
  sourceWorkspaceId?: string;
  /** Managed worktree this job branched from; absent on pre-Git records. */
  baselineWorktreeId?: string;
  baselineWorktreePath?: string;
  /** Stable main checkout that owns the managed-worktree registry. */
  baselineProjectWorkspacePath?: string;
  baseCommit?: string;
  branch?: string;
  remoteUrl?: string;
  /** Branch tip the last successful sync fetched. */
  syncedHeadCommit?: string;
  workspacePath: string;
  promptPreview: string;
  title?: string;
  agentType?: string;
  approvalPolicy?: DispatchApprovalPolicy;
  model?: string;
  lastCursor: number;
  lastState: DispatchJobState;
  createdAt: string;
  updatedAt: string;
}

/**
 * Bumped whenever the projection this cache stores changes shape or meaning.
 *
 * A mismatch discards the cache and replays the job from byte zero, so this is
 * the only thing standing between a projection change and a transcript rendered
 * by rules that no longer exist.
 */
export const DISPATCH_TRANSCRIPT_SCHEMA_VERSION = 4;

/**
 * The controller's UI cache for one observer projection.
 *
 * Cursor, turns, and completeness facts are one document on purpose: the cursor
 * is only meaningful next to the turns it produced, and rendering a truncated
 * transcript as a complete one is exactly what dispatch invariant 14 forbids.
 */
export interface DispatchTranscriptCache {
  schemaVersion: number;
  jobId: string;
  sessionId: string;
  cursor: number;
  dialogTurns: unknown[];
  appliedEventIds: string[];
  eventLogComplete: boolean;
  historyTruncated: boolean;
  omittedEventCount: number;
}

export interface DispatchSelection {
  request: Exclude<DispatchTargetRequest, { kind: 'local' }>;
  target: Exclude<DispatchTarget, { kind: 'local' }>;
  /** Fold the baseline worktree's uncommitted changes into the base commit. */
  includeUncommitted: boolean;
  /** Git revision resolved when creating the controller baseline worktree. */
  baseRef: string;
  approvalPolicy: DispatchApprovalPolicy;
  model?: string;
  availableModels?: string[];
  defaultModel?: string;
}

export function isNonLocalDispatchTarget(
  target: DispatchTargetRequest | DispatchTarget | undefined,
): target is Exclude<DispatchTargetRequest | DispatchTarget, { kind: 'local' }> {
  return !!target && target.kind !== 'local';
}

export function isDispatchJobTerminal(state: DispatchJobState | undefined): boolean {
  return state === 'succeeded' || state === 'failed' || state === 'cancelled';
}
