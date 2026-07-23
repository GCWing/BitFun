import { create } from 'zustand';
import type {
  BackgroundCommandOutputMetadata,
  BackgroundCommandOutputStatus,
} from '@/infrastructure/api/service-api/AgentAPI';

export const BACKGROUND_COMMAND_VISIBLE_AFTER_MS = 5_000;
const BACKGROUND_COMMAND_FINISHED_RETENTION_MS = 800;

export interface BackgroundCommandLifecycleEvent {
  agentSessionId?: string;
  agent_session_id?: string;
  execSessionId?: number;
  exec_session_id?: number;
  command?: string;
  workdir?: string;
  remote?: boolean;
  tty?: boolean;
  status?: BackgroundCommandOutputStatus;
  exitCode?: number | null;
  exit_code?: number | null;
  startedAt?: number;
  started_at?: number;
  endedAt?: number | null;
  ended_at?: number | null;
  timestamp?: number;
}

export interface BackgroundCommandActivity {
  execSessionKey: string;
  agentSessionId?: string;
  execSessionId: number;
  command: string;
  workdir?: string;
  remote: boolean;
  tty: boolean;
  status: BackgroundCommandOutputStatus;
  exitCode?: number;
  startedAtMs: number;
  endedAtMs?: number;
  visible: boolean;
}

interface BackgroundCommandActivityState {
  activities: Record<string, BackgroundCommandActivity>;
  applyLifecycleEvent: (event: BackgroundCommandLifecycleEvent) => void;
  hydrateActivities: (agentSessionId: string | undefined, activities: BackgroundCommandOutputMetadata[]) => void;
  revealActivity: (execSessionKey: string) => void;
  removeActivity: (execSessionKey: string) => void;
}

const revealTimers = new Map<string, number>();
const removalTimers = new Map<string, number>();

function execSessionKey(remote: boolean, execSessionId: number): string {
  return `${remote ? 'remote' : 'local'}:${execSessionId}`;
}

function secondsToMs(value: number | undefined | null): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value * 1000 : undefined;
}

function normalizeLifecycleEvent(event: BackgroundCommandLifecycleEvent): BackgroundCommandActivity | null {
  const execSessionId = event.execSessionId ?? event.exec_session_id;
  if (typeof execSessionId !== 'number' || !Number.isFinite(execSessionId)) {
    return null;
  }

  const remote = event.remote === true;
  const startedAtMs = secondsToMs(event.startedAt ?? event.started_at) ?? Date.now();
  const endedAtMs = secondsToMs(event.endedAt ?? event.ended_at);
  const exitCode = event.exitCode ?? event.exit_code ?? undefined;

  return {
    execSessionKey: execSessionKey(remote, execSessionId),
    agentSessionId: event.agentSessionId ?? event.agent_session_id,
    execSessionId,
    command: event.command || '',
    workdir: event.workdir,
    remote,
    tty: event.tty === true,
    status: event.status ?? 'running',
    exitCode: exitCode == null ? undefined : exitCode,
    startedAtMs,
    endedAtMs,
    visible: false,
  };
}

function activityFromMetadata(metadata: BackgroundCommandOutputMetadata): BackgroundCommandActivity | null {
  if (metadata.execSessionId == null) {
    return null;
  }

  const remote = metadata.remote === true;
  return {
    execSessionKey: execSessionKey(remote, metadata.execSessionId),
    agentSessionId: metadata.agentSessionId,
    execSessionId: metadata.execSessionId,
    command: metadata.command,
    workdir: metadata.workdir,
    remote,
    tty: metadata.tty,
    status: metadata.status,
    exitCode: metadata.exitCode,
    startedAtMs: metadata.startedAt * 1000,
    endedAtMs: metadata.endedAt == null ? undefined : metadata.endedAt * 1000,
    visible: false,
  };
}

function isTerminalStatus(status: BackgroundCommandOutputStatus): boolean {
  return status !== 'running';
}

function shouldBeVisible(activity: BackgroundCommandActivity, now = Date.now()): boolean {
  if (activity.visible) {
    return true;
  }
  const referenceTime = activity.status === 'running'
    ? now
    : activity.endedAtMs ?? now;
  return referenceTime - activity.startedAtMs >= BACKGROUND_COMMAND_VISIBLE_AFTER_MS;
}

function clearTimer(map: Map<string, number>, key: string): void {
  const timerId = map.get(key);
  if (timerId == null) {
    return;
  }
  window.clearTimeout(timerId);
  map.delete(key);
}

function scheduleVisibility(
  activity: BackgroundCommandActivity,
  revealActivity: (execSessionKey: string) => void,
): void {
  clearTimer(revealTimers, activity.execSessionKey);
  if (activity.status !== 'running' || activity.visible) {
    return;
  }

  const delayMs = Math.max(0, activity.startedAtMs + BACKGROUND_COMMAND_VISIBLE_AFTER_MS - Date.now());
  const timerId = window.setTimeout(() => {
    revealTimers.delete(activity.execSessionKey);
    revealActivity(activity.execSessionKey);
  }, delayMs);
  revealTimers.set(activity.execSessionKey, timerId);
}

function scheduleRemoval(
  activity: BackgroundCommandActivity,
  removeActivity: (execSessionKey: string) => void,
): void {
  clearTimer(removalTimers, activity.execSessionKey);
  if (!isTerminalStatus(activity.status)) {
    return;
  }

  const timerId = window.setTimeout(() => {
    removalTimers.delete(activity.execSessionKey);
    removeActivity(activity.execSessionKey);
  }, BACKGROUND_COMMAND_FINISHED_RETENTION_MS);
  removalTimers.set(activity.execSessionKey, timerId);
}

export const useBackgroundCommandActivityStore = create<BackgroundCommandActivityState>((set, get) => ({
  activities: {},

  applyLifecycleEvent: (event) => {
    const normalized = normalizeLifecycleEvent(event);
    if (!normalized) {
      return;
    }

    set((state) => {
      const previous = state.activities[normalized.execSessionKey];
      const next: BackgroundCommandActivity = {
        ...normalized,
        command: normalized.command || previous?.command || '',
        workdir: normalized.workdir ?? previous?.workdir,
        agentSessionId: normalized.agentSessionId ?? previous?.agentSessionId,
        tty: normalized.tty || previous?.tty === true,
        visible: shouldBeVisible({
          ...normalized,
          visible: previous?.visible === true,
        }),
      };

      if (isTerminalStatus(next.status) && !next.visible) {
        const nextActivities = { ...state.activities };
        delete nextActivities[next.execSessionKey];
        return { activities: nextActivities };
      }

      return {
        activities: {
          ...state.activities,
          [next.execSessionKey]: next,
        },
      };
    });

    const current = get().activities[normalized.execSessionKey];
    if (!current) {
      clearTimer(revealTimers, normalized.execSessionKey);
      clearTimer(removalTimers, normalized.execSessionKey);
      return;
    }
    scheduleVisibility(current, get().revealActivity);
    scheduleRemoval(current, get().removeActivity);
  },

  hydrateActivities: (agentSessionId, metadataItems) => {
    const hydrated = metadataItems
      .map(activityFromMetadata)
      .filter((activity): activity is BackgroundCommandActivity => activity !== null)
      .filter(activity => !agentSessionId || activity.agentSessionId === agentSessionId);

    set((state) => {
      const nextActivities = { ...state.activities };
      for (const activity of hydrated) {
        const previous = nextActivities[activity.execSessionKey];
        const next = {
          ...activity,
          visible: shouldBeVisible({
            ...activity,
            visible: previous?.visible === true,
          }),
        };

        if (isTerminalStatus(next.status) && !next.visible) {
          delete nextActivities[next.execSessionKey];
          continue;
        }
        nextActivities[next.execSessionKey] = next;
      }
      return { activities: nextActivities };
    });

    for (const activity of hydrated) {
      const current = get().activities[activity.execSessionKey];
      if (!current) {
        continue;
      }
      scheduleVisibility(current, get().revealActivity);
      scheduleRemoval(current, get().removeActivity);
    }
  },

  revealActivity: (key) => {
    set((state) => {
      const activity = state.activities[key];
      if (!activity || activity.status !== 'running' || activity.visible) {
        return state;
      }
      return {
        activities: {
          ...state.activities,
          [key]: {
            ...activity,
            visible: true,
          },
        },
      };
    });
  },

  removeActivity: (key) => {
    clearTimer(revealTimers, key);
    clearTimer(removalTimers, key);
    set((state) => {
      if (!state.activities[key]) {
        return state;
      }
      const nextActivities = { ...state.activities };
      delete nextActivities[key];
      return { activities: nextActivities };
    });
  },
}));

export function visibleBackgroundCommandActivitiesForSession(
  activities: Record<string, BackgroundCommandActivity>,
  agentSessionId: string | undefined,
): BackgroundCommandActivity[] {
  return Object.values(activities)
    .filter(activity => activity.visible)
    .filter(activity => !agentSessionId || activity.agentSessionId === agentSessionId)
    .sort((a, b) => a.startedAtMs - b.startedAtMs || a.execSessionKey.localeCompare(b.execSessionKey));
}
