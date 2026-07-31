import { create } from 'zustand';
import {
  createJSONStorage,
  persist,
  type StateStorage,
} from 'zustand/middleware';
import type {
  DispatchApprovalPolicy,
  DispatchJobState,
  DispatchReachability,
  DispatchTarget,
  DispatchTargetRequest,
  DispatchWorkspaceDeliveryRequest,
  OutboundDispatchRecord,
} from './types';
import { isDispatchJobTerminal } from './types';

const MAX_APPLIED_EVENT_IDS = 2048;
const MAX_DISMISSED_JOB_IDS = 2048;
const fallbackStorageValues = new Map<string, string>();
const fallbackStorage: StateStorage = {
  getItem: (name) => fallbackStorageValues.get(name) ?? null,
  setItem: (name, value) => {
    fallbackStorageValues.set(name, value);
  },
  removeItem: (name) => {
    fallbackStorageValues.delete(name);
  },
};

export interface DispatchObserverJob {
  jobId: string;
  sessionId: string;
  targetRequest: DispatchTargetRequest;
  target: DispatchTarget;
  sourceWorkspacePath?: string;
  sourceWorkspaceId?: string;
  title: string;
  agentType: string;
  approvalPolicy: DispatchApprovalPolicy;
  workspaceDelivery: DispatchWorkspaceDeliveryRequest;
  model?: string;
  availableModels?: string[];
  defaultModel?: string;
  cursor: number;
  state: DispatchJobState;
  terminalDrained?: boolean;
  lastError?: string;
  pendingPermissions: Array<Record<string, unknown>>;
  eventLogComplete: boolean;
  historyTruncated: boolean;
  omittedEventCount: number;
  appliedEventIds: string[];
  createdAt: number;
  updatedAt: number;
}

export interface DispatchTransportState {
  reachability: DispatchReachability;
  lastTransportError?: string;
}

interface DispatchJobStoreState {
  jobs: Record<string, DispatchObserverJob>;
  /**
   * Live controller-to-target transport health. This is deliberately excluded
   * from persistence because only a current poll can establish reachability.
   */
  transportByJobId: Record<string, DispatchTransportState>;
  /** Local projection tombstones. The target job remains durable, but must not reopen in navigation. */
  dismissedJobIds: string[];
  registerJob: (job: DispatchObserverJob) => void;
  mergeOutboundRecords: (records: OutboundDispatchRecord[]) => void;
  updateProgress: (
    jobId: string,
    update: {
      cursor?: number;
      state?: DispatchJobState;
      lastError?: string;
      appliedEventIds?: string[];
      terminalDrained?: boolean;
      cursorReset?: boolean;
      pendingPermissions?: Array<Record<string, unknown>>;
      eventLogComplete?: boolean;
      historyTruncated?: boolean;
      omittedEventCount?: number;
    },
  ) => void;
  hasAppliedEvent: (jobId: string, eventId: string) => boolean;
  setTransportState: (
    jobId: string,
    reachability: DispatchReachability,
    lastTransportError?: string,
  ) => void;
  resetReplay: (jobId: string) => void;
  updateTitle: (jobId: string, title: string) => void;
  updateModel: (jobId: string, model: string) => void;
  updateApprovalPolicy: (jobId: string, policy: DispatchApprovalPolicy) => void;
  dismissJob: (jobId: string) => void;
  removeJob: (jobId: string) => void;
  clear: () => void;
}

function nextJobState(
  current: DispatchJobState,
  requested: DispatchJobState | undefined,
): DispatchJobState {
  if (!requested || isDispatchJobTerminal(current)) {
    return current;
  }
  return requested;
}

function requestFromTarget(target: DispatchTarget): DispatchTargetRequest {
  switch (target.kind) {
    case 'ssh':
      return {
        kind: 'ssh',
        connectionId: target.connectionId,
        workspacePath: target.workspacePath,
      };
    case 'device':
      return {
        kind: 'device',
        deviceId: target.deviceId,
        workspacePath: target.workspacePath,
      };
    default:
      return { kind: 'local' };
  }
}

export const useDispatchJobStore = create<DispatchJobStoreState>()(
  persist(
    (set, get) => ({
      jobs: {},
      transportByJobId: {},
      dismissedJobIds: [],

      registerJob: (job) => {
        set(state => {
          const transportByJobId = {
            ...state.transportByJobId,
            [job.jobId]: state.transportByJobId[job.jobId] ?? {
              reachability: 'unknown' as const,
            },
          };
          return {
            jobs: {
              ...state.jobs,
              [job.jobId]: {
                ...job,
                cursor: Math.max(0, job.cursor),
                appliedEventIds: job.appliedEventIds.slice(-MAX_APPLIED_EVENT_IDS),
              },
            },
            transportByJobId,
            dismissedJobIds: state.dismissedJobIds.filter(id => id !== job.jobId),
          };
        });
      },

      mergeOutboundRecords: (records) => {
        set(state => {
          const jobs = { ...state.jobs };
          const authoritativeJobIds = new Set(records.map(record => record.jobId));
          const prunedJobIds = new Set<string>();
          for (const [jobId, job] of Object.entries(jobs)) {
            if (
              !authoritativeJobIds.has(jobId)
              && job.state !== 'submitting'
              && job.state !== 'submission_unknown'
            ) {
              // The controller index is authoritative after acknowledgement.
              // Remove renderer cache left behind by retention, manual cleanup,
              // or an older build instead of restoring a ghost projection.
              delete jobs[jobId];
              prunedJobIds.add(jobId);
            }
          }
          for (const record of records) {
            if (state.dismissedJobIds.includes(record.jobId)) {
              continue;
            }
            const sourceWorkspacePath = record.sourceWorkspacePath?.trim() || undefined;
            if (!sourceWorkspacePath) {
              // A legacy/adopted record without controller-side ownership
              // cannot safely be projected into any workspace. In particular,
              // never assign it to whichever workspace happened to initialize
              // first after restart. Remove any previously inferred cache
              // entry so the old behavior migrates itself away.
              delete jobs[record.jobId];
              prunedJobIds.add(record.jobId);
              continue;
            }
            const existing = jobs[record.jobId];
            if (existing) {
              const nextState = nextJobState(existing.state, record.lastState);
              const progressed = nextState !== existing.state;
              jobs[record.jobId] = {
                ...existing,
                target: record.target,
                targetRequest: requestFromTarget(record.target),
                // The durable outbound record is the only authority allowed
                // to restore a projection after renderer restart.
                sourceWorkspacePath,
                sourceWorkspaceId:
                  record.sourceWorkspaceId || existing.sourceWorkspaceId,
                title: record.title || existing.title,
                agentType: record.agentType || existing.agentType,
                approvalPolicy: record.approvalPolicy || existing.approvalPolicy,
                model: record.model || existing.model,
                // `lastCursor` is controller-wide diagnostic progress. A
                // renderer cursor is per observer and must never jump because
                // another observer polled the same target job.
                cursor: existing.cursor,
                state: nextState,
                terminalDrained: progressed ? false : existing.terminalDrained,
                updatedAt: Math.max(existing.updatedAt, Date.parse(record.updatedAt) || 0),
              };
              continue;
            }
            jobs[record.jobId] = {
              jobId: record.jobId,
              sessionId: record.sessionId,
              targetRequest: requestFromTarget(record.target),
              target: record.target,
              sourceWorkspacePath,
              sourceWorkspaceId: record.sourceWorkspaceId,
              title: record.title || record.promptPreview || record.sessionId.slice(0, 8),
              agentType: record.agentType || 'agentic',
              approvalPolicy: record.approvalPolicy || 'reject-and-report',
              workspaceDelivery: { kind: 'existing' },
              model: record.model,
              // A newly reconstructed projection must replay its own
              // transcript instead of inheriting another observer's cursor.
              cursor: 0,
              state: record.lastState,
              terminalDrained: false,
              appliedEventIds: [],
              pendingPermissions: [],
              eventLogComplete: true,
              historyTruncated: false,
              omittedEventCount: 0,
              createdAt: Date.parse(record.createdAt) || Date.now(),
              updatedAt: Date.parse(record.updatedAt) || Date.now(),
            };
          }
          const transportByJobId = { ...state.transportByJobId };
          for (const jobId of prunedJobIds) {
            delete transportByJobId[jobId];
          }
          for (const jobId of Object.keys(jobs)) {
            transportByJobId[jobId] ??= { reachability: 'unknown' };
          }
          return { jobs, transportByJobId };
        });
      },

      updateProgress: (jobId, update) => {
        set(state => {
          const current = state.jobs[jobId];
          if (!current) return state;
          const eventIds = update.appliedEventIds
            ? Array.from(new Set([...current.appliedEventIds, ...update.appliedEventIds]))
                .slice(-MAX_APPLIED_EVENT_IDS)
            : current.appliedEventIds;
          const nextCursor = update.cursorReset
            ? Math.max(0, update.cursor ?? 0)
            : Math.max(current.cursor, update.cursor ?? current.cursor);
          const nextState = nextJobState(current.state, update.state);
          const progressed = nextCursor > current.cursor || nextState !== current.state;
          return {
            jobs: {
              ...state.jobs,
              [jobId]: {
                ...current,
                cursor: nextCursor,
                state: nextState,
                terminalDrained: update.terminalDrained ?? (
                  progressed ? false : current.terminalDrained
                ),
                lastError: update.lastError,
                appliedEventIds: eventIds,
                pendingPermissions:
                  update.pendingPermissions ?? current.pendingPermissions ?? [],
                eventLogComplete:
                  update.eventLogComplete ?? current.eventLogComplete ?? true,
                historyTruncated:
                  update.historyTruncated ?? current.historyTruncated ?? false,
                omittedEventCount:
                  update.omittedEventCount ?? current.omittedEventCount ?? 0,
                updatedAt: Date.now(),
              },
            },
          };
        });
      },

      hasAppliedEvent: (jobId, eventId) =>
        get().jobs[jobId]?.appliedEventIds.includes(eventId) ?? false,

      setTransportState: (jobId, reachability, lastTransportError) => {
        set(state => {
          if (!state.jobs[jobId]) return state;
          const current = state.transportByJobId[jobId];
          const normalizedError = lastTransportError?.trim() || undefined;
          if (
            current?.reachability === reachability &&
            current.lastTransportError === normalizedError
          ) {
            return state;
          }
          return {
            transportByJobId: {
              ...state.transportByJobId,
              [jobId]: {
                reachability,
                lastTransportError: normalizedError,
              },
            },
          };
        });
      },

      resetReplay: (jobId) => {
        set(state => {
          const current = state.jobs[jobId];
          if (!current) return state;
          return {
            jobs: {
              ...state.jobs,
              [jobId]: {
                ...current,
                cursor: 0,
                terminalDrained: false,
                appliedEventIds: [],
                updatedAt: Date.now(),
              },
            },
          };
        });
      },

      updateTitle: (jobId, title) => {
        set(state => {
          const current = state.jobs[jobId];
          const normalizedTitle = title.trim();
          if (!current || !normalizedTitle || current.title === normalizedTitle) {
            return state;
          }
          return {
            jobs: {
              ...state.jobs,
              [jobId]: {
                ...current,
                title: normalizedTitle,
                updatedAt: Date.now(),
              },
            },
          };
        });
      },

      updateModel: (jobId, model) => {
        set(state => {
          const current = state.jobs[jobId];
          const normalizedModel = model.trim();
          if (!current || !normalizedModel || current.model === normalizedModel) {
            return state;
          }
          return {
            jobs: {
              ...state.jobs,
              [jobId]: {
                ...current,
                model: normalizedModel,
                updatedAt: Date.now(),
              },
            },
          };
        });
      },

      updateApprovalPolicy: (jobId, approvalPolicy) => {
        set(state => {
          const current = state.jobs[jobId];
          if (!current || current.approvalPolicy === approvalPolicy) {
            return state;
          }
          return {
            jobs: {
              ...state.jobs,
              [jobId]: {
                ...current,
                approvalPolicy,
                updatedAt: Date.now(),
              },
            },
          };
        });
      },

      dismissJob: (jobId) => {
        set(state => {
          const jobs = { ...state.jobs };
          const transportByJobId = { ...state.transportByJobId };
          delete jobs[jobId];
          delete transportByJobId[jobId];
          return {
            jobs,
            transportByJobId,
            dismissedJobIds: Array.from(new Set([...state.dismissedJobIds, jobId]))
              .slice(-MAX_DISMISSED_JOB_IDS),
          };
        });
      },

      removeJob: (jobId) => {
        set(state => {
          if (!(jobId in state.jobs)) return state;
          const jobs = { ...state.jobs };
          const transportByJobId = { ...state.transportByJobId };
          delete jobs[jobId];
          delete transportByJobId[jobId];
          return { jobs, transportByJobId };
        });
      },

      clear: () => set({ jobs: {}, transportByJobId: {}, dismissedJobIds: [] }),
    }),
    {
      name: 'bitfun-dispatch-jobs-v1',
      version: 1,
      storage: createJSONStorage(() => (
        typeof localStorage === 'undefined' ? fallbackStorage : localStorage
      )),
      partialize: state => ({
        jobs: state.jobs,
        dismissedJobIds: state.dismissedJobIds,
      }),
    },
  ),
);

export const dispatchJobStore = useDispatchJobStore;
