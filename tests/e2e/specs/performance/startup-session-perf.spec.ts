import { $, browser, expect } from '@wdio/globals';
import * as fs from 'fs/promises';
import * as path from 'path';
import {
  readPerformanceNow,
  readStartupTraceSnapshot,
  summarizeSessionOpen,
  summarizeStartup,
  summarizeStartupBreakdown,
  waitForTracePhaseCount,
  type StartupTraceSnapshot,
} from '../../helpers/performance-trace';
import { StartupPage } from '../../page-objects/StartupPage';
import { ensureWorkspaceOpen } from '../../helpers/workspace-utils';
import { ensureCodeSessionOpen, openWorkspace } from '../../helpers/workspace-helper';

const DEFAULT_PERF_SESSION_ID = 'perf-long-session-000';

function reportDir(): string {
  return path.resolve(process.cwd(), 'reports', 'performance');
}

async function writeReport(name: string, data: unknown): Promise<void> {
  await fs.mkdir(reportDir(), { recursive: true });
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  await fs.writeFile(
    path.join(reportDir(), `${name}-${timestamp}.json`),
    `${JSON.stringify(data, null, 2)}\n`,
    'utf8',
  );
}

function countPhase(snapshot: StartupTraceSnapshot, phase: string): number {
  return snapshot.phases.events.filter(event => event.phase === phase).length;
}

function numericEnv(name: string): number | undefined {
  const raw = process.env[name];
  if (!raw) {
    return undefined;
  }
  const value = Number(raw);
  return Number.isFinite(value) ? value : undefined;
}

async function waitForOptionalPhaseCount(
  phase: string,
  minCount: number,
  timeoutMs: number,
): Promise<StartupTraceSnapshot> {
  try {
    return await waitForTracePhaseCount(phase, minCount, timeoutMs);
  } catch {
    return readStartupTraceSnapshot();
  }
}

async function findSessionItem(sessionId: string) {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const item = await $(`[data-testid="session-nav-item"][data-session-id="${sessionId}"]`);
    if (await item.isExisting()) {
      return item;
    }

    const showMore = await $('[data-testid="session-nav-show-more"]');
    if (!(await showMore.isExisting()) || !(await showMore.isEnabled())) {
      break;
    }
    await showMore.click();
    await browser.pause(500);
  }
  return null;
}

async function ensurePerformanceWorkspace(startupPage: StartupPage): Promise<boolean> {
  const targetWorkspace = process.env.E2E_TEST_WORKSPACE;
  if (!targetWorkspace) {
    return ensureWorkspaceOpen(startupPage);
  }

  const isBundledApp = await browser.execute(() => window.location.hostname === 'tauri.localhost');
  if (isBundledApp) {
    return true;
  }

  const opened = await openWorkspace(targetWorkspace);
  if (!opened) {
    return ensureWorkspaceOpen(startupPage);
  }
  await ensureCodeSessionOpen();
  return true;
}

async function isSessionItemActive(item: WebdriverIO.Element): Promise<boolean> {
  const className = await item.getAttribute('class');
  return className.split(/\s+/).includes('is-active');
}

function siblingSessionId(sessionId: string): string | null {
  const match = /^(.*-)(\d{3,})$/.exec(sessionId);
  if (!match) {
    return null;
  }
  return `${match[1]}${match[2] === '001' ? '000' : '001'}`;
}

async function switchAwayIfSessionIsActive(sessionId: string): Promise<void> {
  const item = await findSessionItem(sessionId);
  if (!item || !(await isSessionItemActive(item))) {
    return;
  }

  const alternateId = siblingSessionId(sessionId);
  const alternate = alternateId ? await findSessionItem(alternateId) : null;
  if (!alternate) {
    return;
  }

  const beforeSnapshot = await readStartupTraceSnapshot();
  const frameCountBefore = countPhase(
    beforeSnapshot,
    'historical_session_after_state_commit_frame',
  );
  await alternate.click();
  await waitForOptionalPhaseCount(
    'historical_session_after_state_commit_frame',
    frameCountBefore + 1,
    10000,
  );
}

describe('Performance telemetry', () => {
  const startupPage = new StartupPage();

  before(async () => {
    await waitForTracePhaseCount('interactive_shell_ready', 1, 30000);
    await ensurePerformanceWorkspace(startupPage);
  });

  it('collects startup timing from the current build', async () => {
    const snapshot = await readStartupTraceSnapshot();
    const startup = summarizeStartup(snapshot);
    const breakdown = summarizeStartupBreakdown(snapshot);
    const maxInteractiveMs = numericEnv('BITFUN_E2E_PERF_MAX_INTERACTIVE_MS');

    console.log('[Perf] startup', JSON.stringify({
      appMode: process.env.BITFUN_E2E_APP_MODE ?? 'auto',
      traceId: snapshot.traceId,
      startup,
      breakdown,
      api: snapshot.api,
      native: snapshot.native,
    }));
    await writeReport('startup', {
      appMode: process.env.BITFUN_E2E_APP_MODE ?? 'auto',
      traceId: snapshot.traceId,
      startup,
      breakdown,
      api: snapshot.api,
      native: snapshot.native,
      phases: snapshot.phases.events,
    });

    expect(startup.firstScriptEvalMs).toBeGreaterThan(0);
    expect(startup.interactiveShellReadyMs).toBeGreaterThan(0);
    if (maxInteractiveMs !== undefined) {
      expect(startup.interactiveShellReadyMs).toBeLessThanOrEqual(maxInteractiveMs);
    }
  });

  it('collects first-open timing for a generated long session', async function () {
    const sessionId = process.env.BITFUN_E2E_PERF_SESSION_ID || DEFAULT_PERF_SESSION_ID;
    await switchAwayIfSessionIsActive(sessionId);

    const item = await findSessionItem(sessionId);
    if (!item) {
      console.log(`[Perf] Session ${sessionId} not found; generate it before running this spec.`);
      this.skip();
      return;
    }

    const beforeClickSnapshot = await readStartupTraceSnapshot();
    const frameCountBefore = countPhase(
      beforeClickSnapshot,
      'historical_session_after_state_commit_frame',
    );
    const fullHydrateCountBefore = countPhase(
      beforeClickSnapshot,
      'historical_session_full_hydrate_end',
    );
    const fullHydrateFrameCountBefore = countPhase(
      beforeClickSnapshot,
      'historical_session_full_hydrate_after_state_commit_frame',
    );
    const clickedAtMs = await readPerformanceNow();

    await item.click();

    const afterFrameSnapshot = await waitForTracePhaseCount(
      'historical_session_after_state_commit_frame',
      frameCountBefore + 1,
      20000,
    );
    const afterFullSnapshot = await waitForOptionalPhaseCount(
      'historical_session_full_hydrate_end',
      fullHydrateCountBefore + 1,
      10000,
    );
    const afterFullFrameSnapshot = await waitForOptionalPhaseCount(
      'historical_session_full_hydrate_after_state_commit_frame',
      fullHydrateFrameCountBefore + 1,
      10000,
    );
    const finalSnapshot = [
      afterFrameSnapshot,
      afterFullSnapshot,
      afterFullFrameSnapshot,
    ].reduce((latest, snapshot) =>
      snapshot.phases.events.length >= latest.phases.events.length ? snapshot : latest
    );
    const sessionEvents = finalSnapshot.phases.events.filter(event =>
      event.atMs >= clickedAtMs &&
      event.phase.startsWith('historical_session')
    );
    const sessionOpen = summarizeSessionOpen(sessionEvents, clickedAtMs);
    const maxLatestFrameMs = numericEnv('BITFUN_E2E_PERF_MAX_SESSION_FRAME_MS');

    console.log('[Perf] long-session-first-open', JSON.stringify({
      appMode: process.env.BITFUN_E2E_APP_MODE ?? 'auto',
      sessionId,
      sessionOpen,
    }));
    await writeReport('long-session-first-open', {
      appMode: process.env.BITFUN_E2E_APP_MODE ?? 'auto',
      sessionId,
      clickedAtMs,
      sessionOpen,
      events: sessionEvents,
      api: finalSnapshot.api,
    });

    expect(sessionOpen.hydrateDurationMs).toBeGreaterThan(0);
    expect(sessionOpen.latestFrameSinceHydrateMs).toBeGreaterThan(0);
    expect(sessionOpen.clickToLatestFrameMs).toBeGreaterThan(0);
    if (maxLatestFrameMs !== undefined) {
      expect(sessionOpen.latestFrameSinceHydrateMs).toBeLessThanOrEqual(maxLatestFrameMs);
    }
  });
});
