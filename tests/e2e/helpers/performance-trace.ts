import { browser } from '@wdio/globals';

declare global {
  interface Window {
    __BITFUN_STARTUP_TRACE__?: {
      snapshot?: () => unknown;
    };
  }
}

export interface StartupTracePhase {
  traceId: string;
  phase: string;
  atMs: number;
  [key: string]: unknown;
}

export interface StartupTraceCommandAggregate {
  command: string;
  count: number;
  successCount: number;
  failureCount: number;
  cacheHitCount: number;
  cacheMissCount: number;
  cacheUnknownCount: number;
  remoteCount: number;
  totalDurationMs: number;
  maxDurationMs: number;
  requestBytes: number;
  responseBytes: number;
}

export interface StartupTraceSnapshot {
  traceId: string;
  phases: {
    count: number;
    events: StartupTracePhase[];
  };
  api: {
    totalCount: number;
    successCount: number;
    failureCount: number;
    cacheHitCount: number;
    cacheMissCount: number;
    cacheUnknownCount: number;
    remoteCount: number;
    requestBytes: number;
    responseBytes: number;
    payloadEstimateDurationMs: number;
    byCommand: StartupTraceCommandAggregate[];
  };
}

export type StartupPerfMilestones = {
  firstScriptEvalMs?: number;
  startApplicationStartMs?: number;
  beforeRenderDurationMs?: number;
  reactRenderScheduledMs?: number;
  appEffectMountedMs?: number;
  mainWindowShownMs?: number;
  interactiveShellReadyMs?: number;
  nonCriticalInitDoneMs?: number;
};

export type SessionOpenPerfMilestones = {
  hydrateStartMs?: number;
  restoreDurationMs?: number;
  convertDurationMs?: number;
  stateCommitDurationMs?: number;
  latestFrameSinceHydrateMs?: number;
  hydrateDurationMs?: number;
  fullHydrateDurationMs?: number;
  fullHydrateFrameSinceStartMs?: number;
  loadedTurnCount?: number;
  totalTurnCount?: number;
  isPartial?: boolean;
};

function numberField(value: unknown): number | undefined {
  return typeof value === 'number' ? value : undefined;
}

function booleanField(value: unknown): boolean | undefined {
  return typeof value === 'boolean' ? value : undefined;
}

export async function readStartupTraceSnapshot(): Promise<StartupTraceSnapshot> {
  const snapshot = await browser.execute(() => {
    const diagnostics = window.__BITFUN_STARTUP_TRACE__;
    return diagnostics?.snapshot?.() ?? null;
  });

  if (!snapshot) {
    throw new Error('Startup trace diagnostics are not available');
  }
  return snapshot as StartupTraceSnapshot;
}

export async function readPerformanceNow(): Promise<number> {
  return browser.execute(() => performance.now());
}

export async function waitForTracePhaseCount(
  phase: string,
  minCount: number,
  timeoutMs = 15000,
): Promise<StartupTraceSnapshot> {
  let latest = await readStartupTraceSnapshot();
  await browser.waitUntil(async () => {
    latest = await readStartupTraceSnapshot();
    return latest.phases.events.filter(event => event.phase === phase).length >= minCount;
  }, {
    timeout: timeoutMs,
    interval: 100,
    timeoutMsg: `Timed out waiting for startup trace phase '${phase}' count ${minCount}`,
  });
  return latest;
}

export function summarizeStartup(snapshot: StartupTraceSnapshot): StartupPerfMilestones {
  const phase = (name: string) => snapshot.phases.events.find(event => event.phase === name);
  const beforeRenderEnd = phase('before_render_end');
  return {
    firstScriptEvalMs: numberField(phase('first_script_eval')?.atMs),
    startApplicationStartMs: numberField(phase('start_application_start')?.atMs),
    beforeRenderDurationMs: numberField(beforeRenderEnd?.durationMs),
    reactRenderScheduledMs: numberField(phase('react_render_scheduled')?.atMs),
    appEffectMountedMs: numberField(phase('app_effect_mounted')?.atMs),
    mainWindowShownMs: numberField(phase('main_window_shown')?.atMs),
    interactiveShellReadyMs: numberField(phase('interactive_shell_ready')?.atMs),
    nonCriticalInitDoneMs: numberField(phase('non_critical_init_done')?.atMs),
  };
}

export function summarizeSessionOpen(
  events: StartupTracePhase[],
): SessionOpenPerfMilestones {
  const last = (name: string) => events.filter(event => event.phase === name).at(-1);
  const restoreEnd = last('historical_session_restore_end');
  const convertEnd = last('historical_session_convert_end');
  const stateCommitEnd = last('historical_session_state_commit_end');
  const latestFrame = last('historical_session_after_state_commit_frame');
  const hydrateEnd = last('historical_session_hydrate_end');
  const fullHydrateEnd = last('historical_session_full_hydrate_end');
  const fullHydrateFrame = last('historical_session_full_hydrate_after_state_commit_frame');

  return {
    hydrateStartMs: numberField(last('historical_session_hydrate_start')?.atMs),
    restoreDurationMs: numberField(restoreEnd?.durationMs),
    convertDurationMs: numberField(convertEnd?.durationMs),
    stateCommitDurationMs: numberField(stateCommitEnd?.durationMs),
    latestFrameSinceHydrateMs: numberField(latestFrame?.durationMs),
    hydrateDurationMs: numberField(hydrateEnd?.durationMs),
    fullHydrateDurationMs: numberField(fullHydrateEnd?.durationMs),
    fullHydrateFrameSinceStartMs: numberField(fullHydrateFrame?.durationMs),
    loadedTurnCount: numberField(restoreEnd?.loadedTurnCount),
    totalTurnCount: numberField(restoreEnd?.totalTurnCount ?? hydrateEnd?.totalTurnCount),
    isPartial: booleanField(restoreEnd?.isPartial ?? hydrateEnd?.isPartial),
  };
}
