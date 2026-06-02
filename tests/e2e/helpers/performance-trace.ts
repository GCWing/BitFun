import { browser } from '@wdio/globals';

declare global {
  interface Window {
    __BITFUN_STARTUP_TRACE__?: {
      snapshot?: () => unknown;
    };
    __TAURI__?: {
      core?: {
        invoke?: (command: string, args?: unknown) => Promise<unknown>;
      };
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

export interface StartupTraceApiCallRecord {
  traceId: string;
  type: 'tauri' | 'http';
  command: string;
  target?: string;
  startedAtMs?: number;
  endedAtMs?: number;
  durationMs: number;
  outcome: 'success' | 'failure';
  cacheOutcome: 'hit' | 'miss' | 'unknown';
  requestBytes: number;
  responseBytes: number;
  remote: boolean;
  payloadEstimateDurationMs?: number;
  requestPayloadEstimateDurationMs?: number;
  responsePayloadEstimateDurationMs?: number;
  adapterInitDurationMs?: number;
  transportDurationMs?: number;
  invokeDurationMs?: number;
  activeRequestsAtStart?: number;
  activeRequestsAtEnd?: number;
  maxConcurrentRequests?: number;
}

export interface StartupTraceNativeEvent {
  traceId: string;
  phase: string;
  atMs: number;
  sinceProcessStartMs: number;
  category?: string;
  step?: string;
  command?: string;
  target?: string;
  durationMs?: number;
}

export interface StartupTraceNativeSnapshot {
  traceId: string;
  events: StartupTraceNativeEvent[];
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
    calls: StartupTraceApiCallRecord[];
  };
  native?: StartupTraceNativeSnapshot;
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

export type StartupPerfBreakdown = {
  native: {
    preTauriLastStepEndMs?: number;
    tauriSetupDurationMs?: number;
    tauriSetupUntilMainWindowCreatedDurationMs?: number;
    createMainWindowDurationMs?: number;
    prepareThemeDurationMs?: number;
    webviewBuildDurationMs?: number;
    windowsMaximizeDurationMs?: number;
    windowsShowAfterMaximizeWaitMs?: number;
    showWindowDurationMs?: number;
    focusWindowDurationMs?: number;
  };
  frontend: {
    firstScriptToRenderScheduledMs?: number;
    renderScheduledToAppEffectMs?: number;
    mainWindowShownToInteractiveMs?: number;
    interactiveToNonCriticalDoneMs?: number;
    loadI18nProviderDurationMs?: number;
    beforeRenderSteps: Record<string, number>;
  };
  workspace: {
    initializeDurationMs?: number;
    providerInitializeDurationMs?: number;
    providerManagerInitializeDurationMs?: number;
    steps: Record<string, number>;
  };
  tauriCommand: {
    initializeGlobalState?: {
      frontendDurationMs?: number;
      backendDurationMs?: number;
      estimatedQueueOrBridgeMs?: number;
      steps: Record<string, number>;
    };
    beforeInteractive: {
      matchedCount: number;
      totalFrontendDurationMs: number;
      totalBackendDurationMs: number;
      totalEstimatedQueueOrBridgeMs: number;
      byCommand: Array<{
        command: string;
        count: number;
        frontendDurationMs: number;
        backendDurationMs: number;
        estimatedQueueOrBridgeMs: number;
      }>;
      slowCalls: Array<{
        command: string;
        target?: string;
        startedAtMs?: number;
        frontendDurationMs: number;
        backendDurationMs?: number;
        estimatedQueueOrBridgeMs?: number;
        transportDurationMs?: number;
        invokeDurationMs?: number;
        activeRequestsAtStart?: number;
        maxConcurrentRequests?: number;
      }>;
    };
  };
  apiBeforeInteractive: {
    count: number;
    totalDurationMs: number;
    maxDurationMs?: number;
    byCommand: StartupTraceCommandAggregate[];
    slowCalls: Array<{
      command: string;
      target?: string;
      startedAtMs?: number;
      durationMs: number;
      outcome: 'success' | 'failure';
      remote: boolean;
    }>;
  };
};

export interface StartupApiCommandSegment {
  command: string;
  target?: string;
  startedAtMs?: number;
  frontendDurationMs: number;
  backendDurationMs?: number;
  estimatedQueueOrBridgeMs?: number;
  transportDurationMs?: number;
  invokeDurationMs?: number;
  activeRequestsAtStart?: number;
  activeRequestsAtEnd?: number;
  maxConcurrentRequests?: number;
  requestBytes: number;
  responseBytes: number;
  remote: boolean;
}

export type SessionOpenPerfMilestones = {
  clickToHydrateStartMs?: number;
  clickToLatestFrameMs?: number;
  clickToHydrateEndMs?: number;
  clickToFullHydrateEndMs?: number;
  clickToFullHydrateFrameMs?: number;
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
  restoreTiming?: unknown;
  fullHydrateRestoreTiming?: unknown;
};

function numberField(value: unknown): number | undefined {
  return typeof value === 'number' ? value : undefined;
}

function booleanField(value: unknown): boolean | undefined {
  return typeof value === 'boolean' ? value : undefined;
}

async function readNativeStartupTraceSnapshot(): Promise<StartupTraceNativeSnapshot | undefined> {
  const snapshot = await browser.executeAsync((done: (value: unknown) => void) => {
    const invoke = window.__TAURI__?.core?.invoke;
    if (typeof invoke !== 'function') {
      done(null);
      return;
    }
    invoke('get_startup_native_trace')
      .then(done)
      .catch(() => done(null));
  });

  if (!snapshot || typeof snapshot !== 'object') {
    return undefined;
  }
  return snapshot as StartupTraceNativeSnapshot;
}

export async function readStartupTraceSnapshot(): Promise<StartupTraceSnapshot> {
  const snapshot = await browser.execute(() => {
    const diagnostics = window.__BITFUN_STARTUP_TRACE__;
    return diagnostics?.snapshot?.() ?? null;
  });

  if (!snapshot) {
    throw new Error('Startup trace diagnostics are not available');
  }
  const native = await readNativeStartupTraceSnapshot();
  return {
    ...(snapshot as StartupTraceSnapshot),
    ...(native ? { native } : {}),
  };
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

function round(value: number | undefined): number | undefined {
  return value === undefined ? undefined : Math.round(value * 10) / 10;
}

function aggregateApiCalls(calls: StartupTraceApiCallRecord[]): StartupTraceCommandAggregate[] {
  const byCommand = new Map<string, StartupTraceCommandAggregate>();
  for (const call of calls) {
    const existing = byCommand.get(call.command) ?? {
      command: call.command,
      count: 0,
      successCount: 0,
      failureCount: 0,
      cacheHitCount: 0,
      cacheMissCount: 0,
      cacheUnknownCount: 0,
      remoteCount: 0,
      totalDurationMs: 0,
      maxDurationMs: 0,
      requestBytes: 0,
      responseBytes: 0,
    };
    existing.count += 1;
    existing.successCount += call.outcome === 'success' ? 1 : 0;
    existing.failureCount += call.outcome === 'failure' ? 1 : 0;
    existing.cacheHitCount += call.cacheOutcome === 'hit' ? 1 : 0;
    existing.cacheMissCount += call.cacheOutcome === 'miss' ? 1 : 0;
    existing.cacheUnknownCount += call.cacheOutcome === 'unknown' ? 1 : 0;
    existing.remoteCount += call.remote ? 1 : 0;
    existing.totalDurationMs = round((existing.totalDurationMs ?? 0) + call.durationMs) ?? 0;
    existing.maxDurationMs = Math.max(existing.maxDurationMs, call.durationMs);
    existing.requestBytes += call.requestBytes;
    existing.responseBytes += call.responseBytes;
    byCommand.set(call.command, existing);
  }

  return Array.from(byCommand.values())
    .sort((left, right) => right.totalDurationMs - left.totalDurationMs);
}

export function summarizeApiCommandSegments(
  snapshot: StartupTraceSnapshot,
  frontendCalls: StartupTraceApiCallRecord[] = snapshot.api.calls ?? [],
): StartupApiCommandSegment[] {
  return matchBackendCommandSegments(frontendCalls, snapshot.native?.events ?? []);
}

function matchBackendCommandSegments(
  frontendCalls: StartupTraceApiCallRecord[],
  nativeEvents: StartupTraceNativeEvent[],
): StartupApiCommandSegment[] {
  const backendEvents = nativeEvents
    .filter(event =>
      event.category === 'tauri_command' &&
      typeof event.command === 'string' &&
      typeof event.durationMs === 'number'
    )
    .map(event => ({ ...event, consumed: false }));
  return frontendCalls.map(call => {
    const backend = backendEvents.find(event =>
      !event.consumed &&
      event.command === call.command &&
      (event.target === call.target || event.target === undefined || call.target === undefined)
    );
    if (backend) {
      backend.consumed = true;
    }
    const backendDurationMs = numberField(backend?.durationMs);
    return {
      command: call.command,
      target: call.target,
      startedAtMs: round(call.startedAtMs),
      frontendDurationMs: round(call.durationMs) ?? 0,
      backendDurationMs: round(backendDurationMs),
      estimatedQueueOrBridgeMs: round(
        backendDurationMs === undefined ? undefined : call.durationMs - backendDurationMs
      ),
      transportDurationMs: round(call.transportDurationMs),
      invokeDurationMs: round(call.invokeDurationMs),
      activeRequestsAtStart: numberField(call.activeRequestsAtStart),
      activeRequestsAtEnd: numberField(call.activeRequestsAtEnd),
      maxConcurrentRequests: numberField(call.maxConcurrentRequests),
      requestBytes: call.requestBytes,
      responseBytes: call.responseBytes,
      remote: call.remote,
    };
  });
}

function summarizeBackendCommandOverlap(
  frontendCalls: StartupTraceApiCallRecord[],
  nativeEvents: StartupTraceNativeEvent[],
) {
  const matched = matchBackendCommandSegments(frontendCalls, nativeEvents);
  const byCommand = new Map<string, {
    command: string;
    count: number;
    frontendDurationMs: number;
    backendDurationMs: number;
    estimatedQueueOrBridgeMs: number;
  }>();
  for (const call of matched) {
    const existing = byCommand.get(call.command) ?? {
      command: call.command,
      count: 0,
      frontendDurationMs: 0,
      backendDurationMs: 0,
      estimatedQueueOrBridgeMs: 0,
    };
    existing.count += 1;
    existing.frontendDurationMs = round((existing.frontendDurationMs ?? 0) + call.frontendDurationMs) ?? 0;
    existing.backendDurationMs = round((existing.backendDurationMs ?? 0) + (call.backendDurationMs ?? 0)) ?? 0;
    existing.estimatedQueueOrBridgeMs = round(
      (existing.estimatedQueueOrBridgeMs ?? 0) + (call.estimatedQueueOrBridgeMs ?? 0)
    ) ?? 0;
    byCommand.set(call.command, existing);
  }

  return {
    matchedCount: matched.filter(call => call.backendDurationMs !== undefined).length,
    totalFrontendDurationMs: round(
      matched.reduce((total, call) => total + call.frontendDurationMs, 0)
    ) ?? 0,
    totalBackendDurationMs: round(
      matched.reduce((total, call) => total + (call.backendDurationMs ?? 0), 0)
    ) ?? 0,
    totalEstimatedQueueOrBridgeMs: round(
      matched.reduce((total, call) => total + (call.estimatedQueueOrBridgeMs ?? 0), 0)
    ) ?? 0,
    byCommand: Array.from(byCommand.values())
      .sort((left, right) => right.frontendDurationMs - left.frontendDurationMs),
    slowCalls: [...matched]
      .sort((left, right) => right.frontendDurationMs - left.frontendDurationMs)
      .slice(0, 10),
  };
}

export function summarizeStartupBreakdown(snapshot: StartupTraceSnapshot): StartupPerfBreakdown {
  const first = (name: string) => snapshot.phases.events.find(event => event.phase === name);
  const last = (name: string) => snapshot.phases.events.filter(event => event.phase === name).at(-1);
  const nativeStep = (step: string) =>
    snapshot.native?.events.find(event => event.phase === 'native_step_end' && event.step === step);
  const nativeCommandStep = (step: string) =>
    snapshot.native?.events.find(event => event.category === 'tauri_command' && event.step === step);
  const frontendStepDuration = (phase: string, step: string) =>
    snapshot.phases.events.find(event => event.phase === phase && event.step === step)?.durationMs;
  const workspaceStepDuration = (step: string) =>
    snapshot.phases.events.find(event =>
      event.phase === 'workspace_startup_step_end' &&
      event.step === step
    )?.durationMs;
  const numeric = (value: unknown): number | undefined =>
    typeof value === 'number' ? value : undefined;

  const firstScript = first('first_script_eval')?.atMs;
  const renderScheduled = first('react_render_scheduled')?.atMs;
  const appEffectMounted = first('app_effect_mounted')?.atMs;
  const mainWindowShown = first('main_window_shown')?.atMs;
  const interactive = first('interactive_shell_ready')?.atMs;
  const nonCriticalDone = first('non_critical_init_done')?.atMs;
  const apiCalls = snapshot.api.calls ?? [];
  const initializeGlobalStateApiCall = apiCalls.find(call => call.command === 'initialize_global_state');
  const initializeGlobalStateBackendDuration = nativeCommandStep('initialize_global_state.total')?.durationMs;
  const initializeGlobalStateStepPrefix = 'initialize_global_state.';
  const initializeGlobalStateBackendSteps = Object.fromEntries(
    (snapshot.native?.events ?? [])
      .filter(event =>
        event.category === 'tauri_command' &&
        typeof event.step === 'string' &&
        event.step.startsWith(initializeGlobalStateStepPrefix) &&
        event.step !== 'initialize_global_state.total' &&
        typeof event.durationMs === 'number'
      )
      .map(event => [
        event.step!.slice(initializeGlobalStateStepPrefix.length),
        event.durationMs as number,
      ])
  );
  const apiBeforeInteractive = apiCalls.filter(call =>
    typeof call.startedAtMs === 'number' &&
    (interactive === undefined || call.startedAtMs <= interactive)
  );
  const apiBeforeInteractiveByCommand = aggregateApiCalls(apiBeforeInteractive);
  const backendBeforeInteractive = summarizeBackendCommandOverlap(
    apiBeforeInteractive,
    snapshot.native?.events ?? [],
  );
  const slowApiCalls = [...apiBeforeInteractive]
    .sort((left, right) => right.durationMs - left.durationMs)
    .slice(0, 10)
    .map(call => ({
      command: call.command,
      target: call.target,
      startedAtMs: round(call.startedAtMs),
      durationMs: round(call.durationMs) ?? 0,
      outcome: call.outcome,
      remote: call.remote,
    }));

  return {
    native: {
      preTauriLastStepEndMs: snapshot.native?.events
        .filter(event => event.category === 'native_pre_tauri')
        .at(-1)?.atMs,
      tauriSetupDurationMs: nativeStep('tauri_setup')?.durationMs,
      tauriSetupUntilMainWindowCreatedDurationMs:
        nativeStep('tauri_setup_until_main_window_created')?.durationMs,
      createMainWindowDurationMs: nativeStep('create_main_window')?.durationMs,
      prepareThemeDurationMs: nativeStep('prepare_theme')?.durationMs,
      webviewBuildDurationMs: nativeStep('webview_build')?.durationMs,
      windowsMaximizeDurationMs: nativeStep('windows_maximize')?.durationMs,
      windowsShowAfterMaximizeWaitMs: nativeStep('windows_show_after_maximize_wait')?.durationMs,
      showWindowDurationMs: nativeStep('show_window')?.durationMs,
      focusWindowDurationMs: nativeStep('focus_window')?.durationMs,
    },
    frontend: {
      firstScriptToRenderScheduledMs: round(
        firstScript !== undefined && renderScheduled !== undefined
          ? renderScheduled - firstScript
          : undefined
      ),
      renderScheduledToAppEffectMs: round(
        renderScheduled !== undefined && appEffectMounted !== undefined
          ? appEffectMounted - renderScheduled
          : undefined
      ),
      mainWindowShownToInteractiveMs: round(
        mainWindowShown !== undefined && interactive !== undefined
          ? interactive - mainWindowShown
          : undefined
      ),
      interactiveToNonCriticalDoneMs: round(
        interactive !== undefined && nonCriticalDone !== undefined
          ? nonCriticalDone - interactive
          : undefined
      ),
      loadI18nProviderDurationMs: numeric(frontendStepDuration('startup_step_end', 'load_i18n_provider')),
      beforeRenderSteps: {
        initLogger: numeric(frontendStepDuration('before_render_step_end', 'init_logger')) ?? 0,
        initializeFrontendLogLevelSync:
          numeric(frontendStepDuration('before_render_step_end', 'initialize_frontend_log_level_sync')) ?? 0,
        themeServiceInitialize:
          numeric(frontendStepDuration('before_render_step_end', 'theme_service_initialize')) ?? 0,
      },
    },
    workspace: {
      initializeDurationMs: numeric(last('workspace_initialize_end')?.durationMs),
      providerInitializeDurationMs: numeric(last('workspace_provider_initialize_end')?.durationMs),
      providerManagerInitializeDurationMs:
        numeric(last('workspace_provider_manager_initialize_end')?.durationMs),
      steps: {
        ensureIdentityListener: numeric(workspaceStepDuration('ensure_identity_listener')) ?? 0,
        initializeGlobalState: numeric(workspaceStepDuration('initialize_global_state')) ?? 0,
        cleanupInvalidWorkspaces: numeric(workspaceStepDuration('cleanup_invalid_workspaces')) ?? 0,
        fetchWorkspaceState: numeric(workspaceStepDuration('fetch_workspace_state')) ?? 0,
        updateWorkspaceState: numeric(workspaceStepDuration('update_workspace_state')) ?? 0,
      },
    },
    tauriCommand: {
      initializeGlobalState: {
        frontendDurationMs: round(initializeGlobalStateApiCall?.durationMs),
        backendDurationMs: round(initializeGlobalStateBackendDuration),
        estimatedQueueOrBridgeMs: round(
          initializeGlobalStateApiCall?.durationMs !== undefined &&
            initializeGlobalStateBackendDuration !== undefined
            ? initializeGlobalStateApiCall.durationMs - initializeGlobalStateBackendDuration
            : undefined
        ),
        steps: initializeGlobalStateBackendSteps,
      },
      beforeInteractive: backendBeforeInteractive,
    },
    apiBeforeInteractive: {
      count: apiBeforeInteractive.length,
      totalDurationMs: round(apiBeforeInteractive.reduce((total, call) => total + call.durationMs, 0)) ?? 0,
      maxDurationMs: apiBeforeInteractive.length > 0
        ? round(Math.max(...apiBeforeInteractive.map(call => call.durationMs)))
        : undefined,
      byCommand: apiBeforeInteractiveByCommand,
      slowCalls: slowApiCalls,
    },
  };
}

export function summarizeSessionOpen(
  events: StartupTracePhase[],
  clickedAtMs?: number,
): SessionOpenPerfMilestones {
  const last = (name: string) => events.filter(event => event.phase === name).at(-1);
  const sinceClick = (event: StartupTracePhase | undefined): number | undefined =>
    clickedAtMs !== undefined && typeof event?.atMs === 'number'
      ? round(event.atMs - clickedAtMs)
      : undefined;
  const hydrateStart = last('historical_session_hydrate_start');
  const restoreEnd = last('historical_session_restore_end');
  const convertEnd = last('historical_session_convert_end');
  const stateCommitEnd = last('historical_session_state_commit_end');
  const latestFrame = last('historical_session_after_state_commit_frame');
  const hydrateEnd = last('historical_session_hydrate_end');
  const fullHydrateEnd = last('historical_session_full_hydrate_end');
  const fullHydrateFrame = last('historical_session_full_hydrate_after_state_commit_frame');

  return {
    clickToHydrateStartMs: sinceClick(hydrateStart),
    clickToLatestFrameMs: sinceClick(latestFrame),
    clickToHydrateEndMs: sinceClick(hydrateEnd),
    clickToFullHydrateEndMs: sinceClick(fullHydrateEnd),
    clickToFullHydrateFrameMs: sinceClick(fullHydrateFrame),
    hydrateStartMs: numberField(hydrateStart?.atMs),
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
    restoreTiming: restoreEnd?.restoreTiming,
    fullHydrateRestoreTiming: fullHydrateEnd?.restoreTiming,
  };
}
