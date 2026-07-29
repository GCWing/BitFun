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
  model?: string;
  cursor: number;
  state: DispatchJobState;
  terminalDrained?: boolean;
  lastError?: string;
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
  mergeOutboundRecords: (
    records: OutboundDispatchRecord[],
    fallbackSourceWorkspacePath?: string,
  ) => void;
  updateProgress: (
    jobId: string,
    update: {
      cursor?: number;
      state?: DispatchJobState;
      lastError?: string;
      appliedEventIds?: string[];
      terminalDrained?: boolean;
      cursorReset?: boolean;
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

      mergeOutboundRecords: (records, fallbackSourceWorkspacePath) => {
        set(state => {
          const jobs = { ...state.jobs };
          for (const record of records) {
            if (state.dismissedJobIds.includes(record.jobId)) {
              continue;
            }
            const existing = jobs[record.jobId];
            if (existing) {
              const nextState = nextJobState(existing.state, record.lastState);
              const progressed =
                record.lastCursor > existing.cursor ||
                nextState !== existing.state;
              jobs[record.jobId] = {
                ...existing,
                target: record.target,
                targetRequest: requestFromTarget(record.target),
                cursor: Math.max(existing.cursor, record.lastCursor),
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
              sourceWorkspacePath: fallbackSourceWorkspacePath,
              title: record.promptPreview || record.sessionId.slice(0, 8),
              agentType: 'agentic',
              approvalPolicy: 'reject-and-report',
              cursor: record.lastCursor,
              state: record.lastState,
              terminalDrained: false,
              appliedEventIds: [],
              createdAt: Date.parse(record.createdAt) || Date.now(),
              updatedAt: Date.parse(record.updatedAt) || Date.now(),
            };
          }
          const transportByJobId = { ...state.transportByJobId };
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
