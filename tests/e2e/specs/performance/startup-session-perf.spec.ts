import { $, browser, expect } from '@wdio/globals';
import * as crypto from 'crypto';
import * as fs from 'fs/promises';
import * as fsSync from 'fs';
import * as os from 'os';
import * as path from 'path';
import {
  readPerformanceNow,
  readStartupTraceSnapshot,
  summarizeApiCommandSegments,
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
const MAX_PROJECT_SLUG_LEN = 120;
const LONG_SESSION_VIEWPORT_MIN_COVERAGE_RATIO = 0.7;
const LONG_SESSION_VIEWPORT_MAX_BOTTOM_BLANK_PX = 64;
const LONG_SESSION_VIEWPORT_MAX_BLANK_GAP_PX = 64;
const LONG_SESSION_LATEST_VISIBLE_MAX_BOTTOM_BLANK_PX = 96;
const LONG_SESSION_LATEST_VISIBLE_MAX_BLANK_GAP_PX = 96;
const LONG_SESSION_INPUT_MIN_TOP_RATIO = 0.65;
const LONG_SESSION_INPUT_BOTTOM_TOLERANCE_PX = 96;
const LONG_SESSION_MAX_LATEST_TEXT_DELAY_AFTER_VISIBLE_MS = 120;

type LongSessionViewportState = {
  hasRoot: boolean;
  hasScroller: boolean;
  scrollTop: number | null;
  scrollHeight: number | null;
  clientHeight: number | null;
  latestTurnId: string | null;
  latestTop: number | null;
  latestBottom: number | null;
  latestRendered: boolean;
  latestModelRoundRendered: boolean;
  latestModelRoundVisible: boolean;
  latestModelRoundTextLength: number;
  latestContentVisible: boolean;
  historyPlaceholderVisible: boolean;
  scrollerTop: number | null;
  scrollerBottom: number | null;
  effectiveScrollerBottom: number | null;
  inputOverlayTop: number | null;
  inputOverlayBottom: number | null;
  inputOverlayHeight: number | null;
  latestVisible: boolean;
  visibleTurnIds: string[];
  visibleUserMessageCount: number;
  userMessageCount: number;
  visibleItemCount: number;
  visibleItemTypes: string[];
  visibleModelRoundCount: number;
  visibleExploreGroupCount: number;
  visibleTextLength: number;
  visibleItemHeightStats: {
    min: number | null;
    max: number | null;
    avg: number | null;
  };
  visibleItemSummaries: Array<{
    type: string | null;
    turnId: string | null;
    top: number;
    bottom: number;
    height: number;
    textLength: number;
  }>;
  coveredViewportPx: number;
  coverageRatio: number | null;
  topBlankPx: number | null;
  largestBlankGapPx: number | null;
  bottomBlankPx: number | null;
};

type LongSessionViewportTimelineSample = {
  atMs: number;
  sinceClickMs: number;
  hasRoot: boolean;
  hasScroller: boolean;
  latestRendered: boolean;
  latestModelRoundRendered: boolean;
  latestModelRoundVisible: boolean;
  latestModelRoundTextLength: number;
  latestContentVisible: boolean;
  historyPlaceholderVisible: boolean;
  latestVisible: boolean;
  latestTurnId: string | null;
  scrollTop: number | null;
  scrollHeight: number | null;
  clientHeight: number | null;
  visibleItemCount: number;
  visibleItemTypes: string[];
  visibleModelRoundCount: number;
  visibleTextLength: number;
  visibleItemSummaries: Array<{
    type: string | null;
    turnId: string | null;
    top: number;
    bottom: number;
    height: number;
    textLength: number;
    textContentLength: number;
    opacity: string | null;
  }>;
  renderedItemCount: number;
  renderedItemSummaries: Array<{
    type: string | null;
    turnId: string | null;
    top: number;
    bottom: number;
    height: number;
    textLength: number;
    textContentLength: number;
    opacity: string | null;
    visible: boolean;
  }>;
  coverageRatio: number | null;
  topBlankPx: number | null;
  largestBlankGapPx: number | null;
  bottomBlankPx: number | null;
  inputOverlayTop: number | null;
  inputOverlayBottom: number | null;
};

type LongSessionMainThreadTask = {
  startMs: number;
  sinceClickMs: number;
  durationMs: number;
  name: string;
  entryType: string;
};

type LongSessionViewportTimeline = {
  samples: LongSessionViewportTimelineSample[];
  mainThreadTasks: LongSessionMainThreadTask[];
};

type LongSessionViewportTimelineSummary = {
  firstScrollerAtMs: number | null;
  firstScrollerBlankAtMs: number | null;
  firstVisibleItemAtMs: number | null;
  firstHistoryPlaceholderAtMs: number | null;
  firstLatestVisibleAtMs: number | null;
  firstLatestContentVisibleAtMs: number | null;
  firstLatestTextVisibleAtMs: number | null;
  firstLatestVisibleTextlessAtMs: number | null;
  firstLatestContentVisibleTextlessAtMs: number | null;
  latestTextDelayAfterVisibleMs: number | null;
  latestTextDelayAfterContentVisibleMs: number | null;
  latestVisibleTextlessSampleCount: number;
  latestContentVisibleTextlessSampleCount: number;
  maxTextlessVisibleBlankGapPx: number | null;
  maxTextlessVisibleBottomBlankPx: number | null;
  preLatestTextVisibleBlankSampleCount: number;
  preLatestTextVisibleBlankWithoutPlaceholderSampleCount: number;
  maxPreLatestTextVisibleBlankGapPx: number | null;
  maxPreLatestTextVisibleBlankWithoutPlaceholderGapPx: number | null;
  maxPreLatestTextVisibleBottomBlankPx: number | null;
  postLatestTextVisibleBlankSampleCount: number;
  maxPostLatestTextVisibleBlankGapPx: number | null;
  maxPostLatestTextVisibleBottomBlankPx: number | null;
  postLatestTextVisibleLatestContentMissingSampleCount: number;
};

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

async function findSessionItem(sessionId: string): Promise<ReturnType<typeof $> | null> {
  let lastVisibleSessionIds: string[] = [];
  for (let attempt = 0; attempt < 6; attempt += 1) {
    const item = await $(`[data-testid="session-nav-item"][data-session-id="${sessionId}"]`);
    if (await item.isExisting()) {
      return item;
    }

    lastVisibleSessionIds = await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid="session-nav-item"]'))
        .map(element => element.getAttribute('data-session-id') || '')
        .filter(Boolean)
    );
    const showMore = await $('[data-testid="session-nav-show-more"]');
    if (!(await showMore.isExisting()) || !(await showMore.isEnabled())) {
      break;
    }

    const beforeCount = lastVisibleSessionIds.length;
    await showMore.click();
    await browser.waitUntil(async () => {
      const ids = await browser.execute(() =>
        Array.from(document.querySelectorAll('[data-testid="session-nav-item"]'))
          .map(element => element.getAttribute('data-session-id') || '')
          .filter(Boolean)
      );
      const toggle = await $('[data-testid="session-nav-show-more"]');
      const toggleReady = !(await toggle.isExisting()) || (await toggle.isEnabled());
      return ids.length !== beforeCount && toggleReady;
    }, { timeout: 3000, interval: 100 }).catch(() => undefined);

    const currentVisibleSessionIds = await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid="session-nav-item"]'))
        .map(element => element.getAttribute('data-session-id') || '')
        .filter(Boolean)
    );
    if (currentVisibleSessionIds.length <= beforeCount && attempt > 0) {
      lastVisibleSessionIds = currentVisibleSessionIds;
      break;
    }
  }
  console.log('[Perf] visible session ids while locating target', JSON.stringify({
    target: sessionId,
    visibleSessionIds: lastVisibleSessionIds.slice(0, 40),
    visibleSessionCount: lastVisibleSessionIds.length,
  }));
  return null;
}

async function ensurePerformanceWorkspace(startupPage: StartupPage): Promise<boolean> {
  const isBundledApp = await browser.execute(() => window.location.hostname === 'tauri.localhost');
  if (isBundledApp) {
    return true;
  }

  const targetWorkspace = process.env.E2E_TEST_WORKSPACE;
  if (!targetWorkspace) {
    return ensureWorkspaceOpen(startupPage);
  }

  const opened = await openWorkspace(targetWorkspace);
  if (!opened) {
    return ensureWorkspaceOpen(startupPage);
  }
  await ensureCodeSessionOpen();
  return true;
}

async function isSessionItemActive(item: ReturnType<typeof $>): Promise<boolean> {
  const className = await item.getAttribute('class') ?? '';
  return className.split(/\s+/).includes('is-active');
}

function projectRuntimeSlug(workspacePath: string): string {
  const canonical = fsSync.realpathSync(workspacePath);
  const slug = canonical
    .split('')
    .map(ch => /[a-zA-Z0-9]/.test(ch) ? ch.toLowerCase() : '-')
    .join('')
    .replace(/^-+|-+$/g, '') || 'workspace';

  if (slug.length <= MAX_PROJECT_SLUG_LEN) {
    return slug;
  }

  const suffix = crypto.createHash('sha256').update(canonical).digest('hex').slice(0, 12);
  const maxPrefixLen = MAX_PROJECT_SLUG_LEN - suffix.length - 1;
  return `${slug.slice(0, maxPrefixLen).replace(/-+$/g, '')}-${suffix}`;
}

type LongSessionMetadata = {
  turnCount?: unknown;
  customMetadata?: {
    fixtureScenario?: unknown;
  } | null;
};

async function readLongSessionMetadata(sessionId: string): Promise<LongSessionMetadata | null> {
  const bitfunHome = process.env.BITFUN_HOME || path.join(os.homedir(), '.bitfun');
  const workspaceCandidates = Array.from(new Set([
    process.env.E2E_TEST_WORKSPACE,
    path.resolve(process.cwd(), '..', '..'),
    process.cwd(),
  ].filter((workspacePath): workspacePath is string => Boolean(workspacePath))));

  for (const workspacePath of workspaceCandidates) {
    try {
      const metadataPath = path.join(
        bitfunHome,
        'projects',
        projectRuntimeSlug(workspacePath),
        'sessions',
        sessionId,
        'metadata.json',
      );
      return JSON.parse(await fs.readFile(metadataPath, 'utf8')) as LongSessionMetadata;
    } catch {
      // Try the next known E2E workspace candidate.
    }
  }

  return null;
}

async function readExpectedLatestTurnId(sessionId: string): Promise<string | null> {
  const metadata = await readLongSessionMetadata(sessionId);
  const turnCount = Number(metadata?.turnCount);
  if (!Number.isFinite(turnCount) || turnCount < 1) {
    return null;
  }
  return `${sessionId}-turn-${String(turnCount - 1).padStart(4, '0')}`;
}

async function readLongSessionFixtureScenario(sessionId: string): Promise<string | null> {
  const metadata = await readLongSessionMetadata(sessionId);
  const scenario = metadata?.customMetadata?.fixtureScenario;
  if (typeof scenario !== 'string' || scenario.length === 0) {
    return null;
  }
  return scenario;
}

function siblingSessionId(sessionId: string): string | null {
  const match = /^(.*-)(\d{3,})$/.exec(sessionId);
  if (!match) {
    return null;
  }
  return `${match[1]}${match[2] === '001' ? '000' : '001'}`;
}

async function switchAwayFromSession(sessionId: string): Promise<void> {
  const alternateId = siblingSessionId(sessionId);
  const alternate = alternateId ? await findSessionItem(alternateId) : null;
  if (!alternate) {
    return;
  }
  if (await isSessionItemActive(alternate)) {
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
  await browser.pause(50);
}

async function readLongSessionViewportState(expectedLatestTurnId?: string | null): Promise<LongSessionViewportState> {
  return browser.execute((targetTurnId) => {
    const root = document.querySelector<HTMLElement>(
      '.modern-flowchat-container__messages .virtual-message-list',
    );
    const scroller = root?.querySelector<HTMLElement>(
      '[data-virtuoso-scroller="true"], [data-virtuoso-scroller]',
    ) ?? null;
    const userMessages = Array.from(root?.querySelectorAll<HTMLElement>(
      '.virtual-item-wrapper[data-turn-id][data-item-type="user-message"]',
    ) ?? []);
    const renderedLatest = userMessages.length > 0 ? userMessages[userMessages.length - 1] : null;
    const latest = targetTurnId
      ? root?.querySelector<HTMLElement>(
        `.virtual-item-wrapper[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
      ) ?? null
      : renderedLatest;
    const latestModelRoundSegments = targetTurnId
      ? Array.from(root?.querySelectorAll<HTMLElement>(
        `.virtual-item-wrapper[data-turn-id="${targetTurnId}"][data-item-type="model-round"]`,
      ) ?? [])
      : [];
    const scrollerRect = scroller?.getBoundingClientRect() ?? null;
    const inputOverlay = document.querySelector<HTMLElement>('.bitfun-chat-input-drop-zone');
    const inputOverlayRect = inputOverlay?.getBoundingClientRect() ?? null;
    const historyPlaceholder = document.querySelector<HTMLElement>(
      '.modern-flowchat-container__messages .history-session-placeholder',
    );
    const historyPlaceholderRect = historyPlaceholder?.getBoundingClientRect() ?? null;
    const historyPlaceholderStyle = historyPlaceholder
      ? window.getComputedStyle(historyPlaceholder)
      : null;
    const historyPlaceholderVisible = Boolean(
      historyPlaceholder &&
      historyPlaceholderRect &&
      historyPlaceholderRect.width > 0 &&
      historyPlaceholderRect.height > 0 &&
      historyPlaceholderStyle?.visibility !== 'hidden' &&
      historyPlaceholderStyle?.display !== 'none' &&
      historyPlaceholderStyle?.opacity !== '0'
    );
    const effectiveScrollerBottom = scrollerRect
      ? Math.min(scrollerRect.bottom, inputOverlayRect?.top ?? scrollerRect.bottom)
      : null;
    const latestRect = latest?.getBoundingClientRect() ?? null;
    const isVisibleWithinScroller = (rect: DOMRect | null): boolean => Boolean(
      scrollerRect &&
      rect &&
      rect.bottom > scrollerRect.top &&
      rect.top < (effectiveScrollerBottom ?? scrollerRect.bottom)
    );
    const latestModelRoundVisibleSegments = latestModelRoundSegments
      .filter(element => isVisibleWithinScroller(element.getBoundingClientRect()));
    const latestModelRoundVisible = latestModelRoundVisibleSegments.length > 0;
    const latestModelRoundTextLength = latestModelRoundVisibleSegments
      .reduce((total, element) => total + (element.innerText?.length ?? 0), 0);
    const latestVisible = isVisibleWithinScroller(latestRect);
    const visibleUserMessages = scrollerRect
      ? userMessages.filter(element => {
        const rect = element.getBoundingClientRect();
        return rect.bottom > scrollerRect.top && rect.top < (effectiveScrollerBottom ?? scrollerRect.bottom);
      })
      : [];
    const visibleItems = scrollerRect
      ? Array.from(root?.querySelectorAll<HTMLElement>('.virtual-item-wrapper[data-turn-id]') ?? [])
        .map(element => {
          const rect = element.getBoundingClientRect();
          return {
            element,
            top: Math.max(rect.top, scrollerRect.top),
            bottom: Math.min(rect.bottom, effectiveScrollerBottom ?? scrollerRect.bottom),
            rawTop: rect.top,
            rawBottom: rect.bottom,
          };
        })
        .filter(({ top, bottom }) =>
          bottom > scrollerRect.top &&
          top < (effectiveScrollerBottom ?? scrollerRect.bottom) &&
          bottom > top
        )
        .sort((left, right) => left.top - right.top)
      : [];
    const visibleItemSummaries = scrollerRect
      ? visibleItems.map(({ element, rawTop, rawBottom }) => ({
        type: element.dataset.itemType ?? null,
        turnId: element.dataset.turnId ?? null,
        top: rawTop - scrollerRect.top,
        bottom: rawBottom - scrollerRect.top,
        height: Math.max(0, rawBottom - rawTop),
        textLength: element.innerText?.length ?? 0,
      }))
      : [];
    const visibleTextLength = visibleItemSummaries
      .reduce((total, item) => total + item.textLength, 0);
    const visibleItemHeights = visibleItemSummaries.map(item => item.height);
    const visibleItemHeightStats = visibleItemHeights.length > 0
      ? {
        min: Math.min(...visibleItemHeights),
        max: Math.max(...visibleItemHeights),
        avg: visibleItemHeights.reduce((sum, height) => sum + height, 0) / visibleItemHeights.length,
      }
      : { min: null, max: null, avg: null };

    let coveredViewportPx = 0;
    let topBlankPx: number | null = null;
    let largestBlankGapPx: number | null = null;
    let bottomBlankPx: number | null = null;
    if (scrollerRect) {
      let cursor = scrollerRect.top;
      let maxBottom = scrollerRect.top;
      visibleItems.forEach((item, index) => {
        if (item.top > cursor) {
          const gap = item.top - cursor;
          if (index === 0) {
            topBlankPx = gap;
          }
          largestBlankGapPx = Math.max(largestBlankGapPx ?? 0, gap);
        }
        const coveredStart = Math.max(cursor, item.top);
        if (item.bottom > coveredStart) {
          coveredViewportPx += item.bottom - coveredStart;
          cursor = Math.max(cursor, item.bottom);
          maxBottom = Math.max(maxBottom, item.bottom);
        }
      });
      if (visibleItems.length === 0) {
        topBlankPx = Math.max(0, (effectiveScrollerBottom ?? scrollerRect.bottom) - scrollerRect.top);
      } else if (topBlankPx === null) {
        topBlankPx = 0;
      }
      bottomBlankPx = Math.max(0, (effectiveScrollerBottom ?? scrollerRect.bottom) - maxBottom);
      largestBlankGapPx = Math.max(largestBlankGapPx ?? 0, bottomBlankPx);
    }
    const effectiveViewportHeight = scrollerRect && effectiveScrollerBottom !== null
      ? Math.max(0, effectiveScrollerBottom - scrollerRect.top)
      : null;

    return {
      hasRoot: Boolean(root),
      hasScroller: Boolean(scroller),
      scrollTop: scroller?.scrollTop ?? null,
      scrollHeight: scroller?.scrollHeight ?? null,
      clientHeight: scroller?.clientHeight ?? null,
      latestTurnId: latest?.dataset.turnId ?? targetTurnId ?? null,
      latestTop: latestRect?.top ?? null,
      latestBottom: latestRect?.bottom ?? null,
      latestRendered: Boolean(latest),
      latestModelRoundRendered: latestModelRoundSegments.length > 0,
      latestModelRoundVisible,
      latestModelRoundTextLength,
      latestContentVisible: latestVisible || latestModelRoundVisible,
      historyPlaceholderVisible,
      scrollerTop: scrollerRect?.top ?? null,
      scrollerBottom: scrollerRect?.bottom ?? null,
      effectiveScrollerBottom,
      inputOverlayTop: inputOverlayRect?.top ?? null,
      inputOverlayBottom: inputOverlayRect?.bottom ?? null,
      inputOverlayHeight: inputOverlayRect?.height ?? null,
      latestVisible,
      visibleTurnIds: visibleUserMessages
        .map(element => element.dataset.turnId)
        .filter((turnId): turnId is string => Boolean(turnId)),
      visibleUserMessageCount: visibleUserMessages.length,
      userMessageCount: userMessages.length,
      visibleItemCount: visibleItems.length,
      visibleItemTypes: visibleItems
        .map(({ element }) => element.dataset.itemType)
        .filter((itemType): itemType is string => Boolean(itemType)),
      visibleModelRoundCount: visibleItems
        .filter(({ element }) => element.dataset.itemType === 'model-round')
        .length,
      visibleExploreGroupCount: visibleItems
        .filter(({ element }) => element.dataset.itemType === 'explore-group')
        .length,
      visibleTextLength,
      visibleItemHeightStats,
      visibleItemSummaries,
      coveredViewportPx,
      coverageRatio: effectiveViewportHeight && effectiveViewportHeight > 0
        ? coveredViewportPx / effectiveViewportHeight
        : null,
      topBlankPx,
      largestBlankGapPx,
      bottomBlankPx,
    };
  }, expectedLatestTurnId ?? null);
}

async function startLongSessionViewportTimelineRecorder(
  expectedLatestTurnId: string,
  clickedAtMs: number,
  enableRenderProfile: boolean,
): Promise<void> {
  await browser.execute((targetTurnId, clickTime, shouldEnableRenderProfile) => {
    const globalWindow = window as typeof window & {
      __bitfunLongSessionViewportTimeline?: LongSessionViewportTimelineSample[];
      __bitfunLongSessionMainThreadTasks?: LongSessionMainThreadTask[];
      __bitfunLongSessionViewportTimelineTimer?: number;
      __bitfunLongSessionLongTaskObserver?: PerformanceObserver;
      __BITFUN_RENDER_PROFILE_ENABLED__?: boolean;
    };
    globalWindow.__BITFUN_RENDER_PROFILE_ENABLED__ = shouldEnableRenderProfile;
    if (globalWindow.__bitfunLongSessionViewportTimelineTimer !== undefined) {
      window.clearInterval(globalWindow.__bitfunLongSessionViewportTimelineTimer);
    }
    globalWindow.__bitfunLongSessionLongTaskObserver?.disconnect();
    const samples: LongSessionViewportTimelineSample[] = [];
    const mainThreadTasks: LongSessionMainThreadTask[] = [];
    try {
      if (PerformanceObserver.supportedEntryTypes.includes('longtask')) {
        const observer = new PerformanceObserver(list => {
          for (const entry of list.getEntries()) {
            mainThreadTasks.push({
              startMs: entry.startTime,
              sinceClickMs: entry.startTime - clickTime,
              durationMs: entry.duration,
              name: entry.name,
              entryType: entry.entryType,
            });
            if (mainThreadTasks.length > 120) {
              mainThreadTasks.shift();
            }
          }
        });
        observer.observe({ entryTypes: ['longtask'] });
        globalWindow.__bitfunLongSessionLongTaskObserver = observer;
      }
    } catch {
      globalWindow.__bitfunLongSessionLongTaskObserver = undefined;
    }
    const readSample = (): LongSessionViewportTimelineSample => {
      const root = document.querySelector<HTMLElement>(
        '.modern-flowchat-container__messages .virtual-message-list',
      );
      const scroller = root?.querySelector<HTMLElement>(
        '[data-virtuoso-scroller="true"], [data-virtuoso-scroller]',
      ) ?? null;
      const inputOverlay = document.querySelector<HTMLElement>('.bitfun-chat-input-drop-zone');
      const scrollerRect = scroller?.getBoundingClientRect() ?? null;
      const inputOverlayRect = inputOverlay?.getBoundingClientRect() ?? null;
      const historyPlaceholder = document.querySelector<HTMLElement>(
        '.modern-flowchat-container__messages .history-session-placeholder',
      );
      const historyPlaceholderRect = historyPlaceholder?.getBoundingClientRect() ?? null;
      const historyPlaceholderStyle = historyPlaceholder
        ? window.getComputedStyle(historyPlaceholder)
        : null;
      const historyPlaceholderVisible = Boolean(
        historyPlaceholder &&
        historyPlaceholderRect &&
        historyPlaceholderRect.width > 0 &&
        historyPlaceholderRect.height > 0 &&
        historyPlaceholderStyle?.visibility !== 'hidden' &&
        historyPlaceholderStyle?.display !== 'none' &&
        historyPlaceholderStyle?.opacity !== '0'
      );
      const effectiveScrollerBottom = scrollerRect
        ? Math.min(scrollerRect.bottom, inputOverlayRect?.top ?? scrollerRect.bottom)
        : null;
      const latest = targetTurnId
        ? root?.querySelector<HTMLElement>(
          `.virtual-item-wrapper[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
        ) ?? null
        : null;
      const latestModelRoundSegments = targetTurnId
        ? Array.from(root?.querySelectorAll<HTMLElement>(
          `.virtual-item-wrapper[data-turn-id="${targetTurnId}"][data-item-type="model-round"]`,
        ) ?? [])
        : [];
      const latestRect = latest?.getBoundingClientRect() ?? null;
      const isVisibleWithinScroller = (rect: DOMRect | null): boolean => Boolean(
        scrollerRect &&
        rect &&
        rect.bottom > scrollerRect.top &&
        rect.top < (effectiveScrollerBottom ?? scrollerRect.bottom)
      );
      const latestModelRoundVisibleSegments = latestModelRoundSegments
        .filter(element => isVisibleWithinScroller(element.getBoundingClientRect()));
      const latestModelRoundVisible = latestModelRoundVisibleSegments.length > 0;
      const latestModelRoundTextLength = latestModelRoundVisibleSegments
        .reduce((total, element) => total + (element.innerText?.length ?? 0), 0);
      const latestVisible = isVisibleWithinScroller(latestRect);
      const renderedItems = scrollerRect
        ? Array.from(root?.querySelectorAll<HTMLElement>('.virtual-item-wrapper[data-turn-id]') ?? [])
          .map(element => {
            const rect = element.getBoundingClientRect();
            const top = Math.max(rect.top, scrollerRect.top);
            const bottom = Math.min(rect.bottom, effectiveScrollerBottom ?? scrollerRect.bottom);
            const visible = (
              bottom > scrollerRect.top &&
              top < (effectiveScrollerBottom ?? scrollerRect.bottom) &&
              bottom > top
            );
            return {
              element,
              rect,
              top,
              bottom,
              visible,
            };
          })
          .sort((left, right) => left.rect.top - right.rect.top)
        : [];
      const visibleItems = renderedItems
        .filter(item => item.visible)
        .sort((left, right) => left.top - right.top);
      const visibleTextLength = visibleItems
        .reduce((total, { element }) => total + (element.innerText?.length ?? 0), 0);
      const summarizeItem = ({ element, rect, visible }: typeof renderedItems[number]) => ({
        type: element.dataset.itemType ?? null,
        turnId: element.dataset.turnId ?? null,
        top: scrollerRect ? rect.top - scrollerRect.top : 0,
        bottom: scrollerRect ? rect.bottom - scrollerRect.top : 0,
        height: Math.max(0, rect.bottom - rect.top),
        textLength: element.innerText?.length ?? 0,
        textContentLength: element.textContent?.length ?? 0,
        opacity: window.getComputedStyle(element).opacity ?? null,
        visible,
      });
      const visibleItemSummaries = visibleItems.map(item => {
        const summary = summarizeItem(item);
        return {
          type: summary.type,
          turnId: summary.turnId,
          top: summary.top,
          bottom: summary.bottom,
          height: summary.height,
          textLength: summary.textLength,
          textContentLength: summary.textContentLength,
          opacity: summary.opacity,
        };
      });
      const renderedItemSummaries = renderedItems
        .slice(0, 16)
        .map(summarizeItem);

      let coveredViewportPx = 0;
      let topBlankPx: number | null = null;
      let largestBlankGapPx: number | null = null;
      let bottomBlankPx: number | null = null;
      if (scrollerRect) {
        let cursor = scrollerRect.top;
        let maxBottom = scrollerRect.top;
        visibleItems.forEach((item, index) => {
          if (item.top > cursor) {
            const gap = item.top - cursor;
            if (index === 0) {
              topBlankPx = gap;
            }
            largestBlankGapPx = Math.max(largestBlankGapPx ?? 0, gap);
          }
          const coveredStart = Math.max(cursor, item.top);
          if (item.bottom > coveredStart) {
            coveredViewportPx += item.bottom - coveredStart;
            cursor = Math.max(cursor, item.bottom);
            maxBottom = Math.max(maxBottom, item.bottom);
          }
        });
        if (visibleItems.length === 0) {
          topBlankPx = Math.max(0, (effectiveScrollerBottom ?? scrollerRect.bottom) - scrollerRect.top);
        } else if (topBlankPx === null) {
          topBlankPx = 0;
        }
        bottomBlankPx = Math.max(0, (effectiveScrollerBottom ?? scrollerRect.bottom) - maxBottom);
        largestBlankGapPx = Math.max(largestBlankGapPx ?? 0, bottomBlankPx);
      }
      const effectiveViewportHeight = scrollerRect && effectiveScrollerBottom !== null
        ? Math.max(0, effectiveScrollerBottom - scrollerRect.top)
        : null;
      const atMs = performance.now();

      return {
        atMs,
        sinceClickMs: atMs - clickTime,
        hasRoot: Boolean(root),
        hasScroller: Boolean(scroller),
        latestRendered: Boolean(latest),
        latestModelRoundRendered: latestModelRoundSegments.length > 0,
        latestModelRoundVisible,
        latestModelRoundTextLength,
        latestContentVisible: latestVisible || latestModelRoundVisible,
        historyPlaceholderVisible,
        latestVisible,
        latestTurnId: latest?.dataset.turnId ?? targetTurnId ?? null,
        scrollTop: scroller?.scrollTop ?? null,
        scrollHeight: scroller?.scrollHeight ?? null,
        clientHeight: scroller?.clientHeight ?? null,
        visibleItemCount: visibleItems.length,
        visibleItemTypes: visibleItems
          .map(({ element }) => element.dataset.itemType)
          .filter((itemType): itemType is string => Boolean(itemType)),
        visibleModelRoundCount: visibleItems
          .filter(({ element }) => element.dataset.itemType === 'model-round')
          .length,
        visibleTextLength,
        visibleItemSummaries,
        renderedItemCount: renderedItems.length,
        renderedItemSummaries,
        coverageRatio: effectiveViewportHeight && effectiveViewportHeight > 0
          ? coveredViewportPx / effectiveViewportHeight
          : null,
        topBlankPx,
        largestBlankGapPx,
        bottomBlankPx,
        inputOverlayTop: inputOverlayRect?.top ?? null,
        inputOverlayBottom: inputOverlayRect?.bottom ?? null,
      };
    };

    const record = () => {
      samples.push(readSample());
      if (samples.length > 120) {
        samples.shift();
      }
    };

    globalWindow.__bitfunLongSessionViewportTimeline = samples;
    globalWindow.__bitfunLongSessionMainThreadTasks = mainThreadTasks;
    record();
    globalWindow.__bitfunLongSessionViewportTimelineTimer = window.setInterval(record, 50);
  }, expectedLatestTurnId, clickedAtMs, enableRenderProfile);
}

async function stopLongSessionViewportTimelineRecorder(): Promise<LongSessionViewportTimeline> {
  return browser.execute(() => {
    const globalWindow = window as typeof window & {
      __bitfunLongSessionViewportTimeline?: LongSessionViewportTimelineSample[];
      __bitfunLongSessionMainThreadTasks?: LongSessionMainThreadTask[];
      __bitfunLongSessionViewportTimelineTimer?: number;
      __bitfunLongSessionLongTaskObserver?: PerformanceObserver;
      __BITFUN_RENDER_PROFILE_ENABLED__?: boolean;
    };
    if (globalWindow.__bitfunLongSessionViewportTimelineTimer !== undefined) {
      window.clearInterval(globalWindow.__bitfunLongSessionViewportTimelineTimer);
      globalWindow.__bitfunLongSessionViewportTimelineTimer = undefined;
    }
    globalWindow.__bitfunLongSessionLongTaskObserver?.disconnect();
    globalWindow.__bitfunLongSessionLongTaskObserver = undefined;
    const samples = globalWindow.__bitfunLongSessionViewportTimeline ?? [];
    const mainThreadTasks = globalWindow.__bitfunLongSessionMainThreadTasks ?? [];
    globalWindow.__bitfunLongSessionViewportTimeline = undefined;
    globalWindow.__bitfunLongSessionMainThreadTasks = undefined;
    globalWindow.__BITFUN_RENDER_PROFILE_ENABLED__ = false;
    return { samples, mainThreadTasks };
  });
}

async function waitForLatestLongSessionTurnVisible(timeoutMs: number, expectedLatestTurnId?: string | null): Promise<{
  visibleAtMs: number;
  viewport: LongSessionViewportState;
}> {
  let viewport = await readLongSessionViewportState(expectedLatestTurnId);
  try {
    await browser.waitUntil(async () => {
      viewport = await readLongSessionViewportState(expectedLatestTurnId);
      return viewport.latestContentVisible;
    }, {
      timeout: timeoutMs,
      interval: 50,
      timeoutMsg: 'latest long-session content did not become visible',
    });
  } catch (error) {
    viewport = await readLongSessionViewportState(expectedLatestTurnId);
    const snapshot = await readStartupTraceSnapshot().catch(() => null);
    const relatedEvents = snapshot?.phases.events
      .filter(event =>
        event.phase.includes('latest_anchor') ||
        event.phase.includes('latest_end_anchor') ||
        event.phase.includes('turn_pin')
      )
      .slice(-30) ?? [];
    throw new Error(
      `${error instanceof Error ? error.message : String(error)}; ` +
      `viewport=${JSON.stringify(viewport)}; ` +
      `relatedEvents=${JSON.stringify(relatedEvents)}`,
    );
  }

  return {
    visibleAtMs: await readPerformanceNow(),
    viewport,
  };
}

function isLongSessionViewportUsable(viewport: LongSessionViewportState): boolean {
  const coverageRatio = viewport.coverageRatio ?? 0;
  const bottomBlankPx = viewport.bottomBlankPx ?? Number.POSITIVE_INFINITY;
  const largestBlankGapPx = viewport.largestBlankGapPx ?? Number.POSITIVE_INFINITY;
  return (
    viewport.latestContentVisible &&
    viewport.latestModelRoundVisible &&
    viewport.latestModelRoundTextLength > 0 &&
    coverageRatio >= LONG_SESSION_VIEWPORT_MIN_COVERAGE_RATIO &&
    bottomBlankPx <= LONG_SESSION_VIEWPORT_MAX_BOTTOM_BLANK_PX &&
    largestBlankGapPx <= LONG_SESSION_VIEWPORT_MAX_BLANK_GAP_PX
  );
}

function isLongSessionLatestVisibleViewportPositioned(viewport: LongSessionViewportState): boolean {
  const coverageRatio = viewport.coverageRatio ?? 0;
  const bottomBlankPx = viewport.bottomBlankPx ?? Number.POSITIVE_INFINITY;
  const largestBlankGapPx = viewport.largestBlankGapPx ?? Number.POSITIVE_INFINITY;
  return (
    viewport.latestContentVisible &&
    coverageRatio >= LONG_SESSION_VIEWPORT_MIN_COVERAGE_RATIO &&
    bottomBlankPx <= LONG_SESSION_LATEST_VISIBLE_MAX_BOTTOM_BLANK_PX &&
    largestBlankGapPx <= LONG_SESSION_LATEST_VISIBLE_MAX_BLANK_GAP_PX
  );
}

async function maybeSavePerfScreenshot(name: string): Promise<string | null> {
  if (process.env.BITFUN_E2E_PERF_SCREENSHOTS !== '1') {
    return null;
  }

  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  const screenshotsDir = path.resolve(process.cwd(), 'reports', 'screenshots');
  await fs.mkdir(screenshotsDir, { recursive: true });
  const screenshotPath = path.join(screenshotsDir, `${name}-${timestamp}.png`);
  await browser.saveScreenshot(screenshotPath);
  return screenshotPath;
}

function isLongSessionInputAnchoredNearBottom(viewport: LongSessionViewportState): boolean {
  if (
    viewport.scrollerTop === null ||
    viewport.scrollerBottom === null ||
    viewport.clientHeight === null ||
    viewport.inputOverlayTop === null ||
    viewport.inputOverlayBottom === null
  ) {
    return false;
  }

  const minTop = viewport.scrollerTop + viewport.clientHeight * LONG_SESSION_INPUT_MIN_TOP_RATIO;
  const bottomDistance = Math.abs(viewport.scrollerBottom - viewport.inputOverlayBottom);
  return (
    viewport.inputOverlayTop >= minTop &&
    bottomDistance <= LONG_SESSION_INPUT_BOTTOM_TOLERANCE_PX
  );
}

function summarizeLongSessionViewportTimeline(
  samples: LongSessionViewportTimelineSample[],
): LongSessionViewportTimelineSummary {
  const firstScroller = samples.find(sample => sample.hasScroller);
  const firstScrollerBlank = samples.find(sample =>
    sample.hasScroller &&
    sample.visibleItemCount === 0
  );
  const firstVisibleItem = samples.find(sample => sample.visibleItemCount > 0);
  const firstHistoryPlaceholder = samples.find(sample => sample.historyPlaceholderVisible);
  const firstLatestVisible = samples.find(sample => sample.latestVisible);
  const firstLatestContentVisible = samples.find(sample => sample.latestContentVisible);
  const firstLatestTextVisible = samples.find(sample =>
    sample.latestModelRoundVisible &&
    sample.latestModelRoundTextLength > 0
  );
  const latestVisibleTextlessSamples = samples.filter(sample =>
    sample.latestVisible &&
      sample.latestModelRoundVisible &&
      sample.latestModelRoundTextLength === 0
  );
  const firstLatestVisibleTextless = latestVisibleTextlessSamples[0];
  const latestContentVisibleTextlessSamples = samples.filter(sample =>
    sample.latestContentVisible &&
      sample.latestModelRoundVisible &&
      sample.latestModelRoundTextLength === 0
  );
  const firstLatestContentVisibleTextless = latestContentVisibleTextlessSamples[0];
  const textlessBlankGaps = latestVisibleTextlessSamples
    .map(sample => sample.largestBlankGapPx)
    .filter((value): value is number => typeof value === 'number');
  const textlessBottomBlanks = latestVisibleTextlessSamples
    .map(sample => sample.bottomBlankPx)
    .filter((value): value is number => typeof value === 'number');
  const preLatestTextVisibleBlankSamples = firstLatestTextVisible
    ? samples.filter(sample =>
      sample.sinceClickMs < firstLatestTextVisible.sinceClickMs &&
      sample.hasRoot &&
      sample.hasScroller &&
      sample.visibleItemCount === 0
    )
    : samples.filter(sample =>
      sample.hasRoot &&
      sample.hasScroller &&
      sample.visibleItemCount === 0
    );
  const preLatestTextVisibleBlankGaps = preLatestTextVisibleBlankSamples
    .map(sample => sample.largestBlankGapPx)
    .filter((value): value is number => typeof value === 'number');
  const preLatestTextVisibleBlankWithoutPlaceholderSamples = preLatestTextVisibleBlankSamples
    .filter(sample => !sample.historyPlaceholderVisible);
  const preLatestTextVisibleBlankWithoutPlaceholderGaps = preLatestTextVisibleBlankWithoutPlaceholderSamples
    .map(sample => sample.largestBlankGapPx)
    .filter((value): value is number => typeof value === 'number');
  const preLatestTextVisibleBottomBlanks = preLatestTextVisibleBlankSamples
    .map(sample => sample.bottomBlankPx)
    .filter((value): value is number => typeof value === 'number');
  const postLatestTextVisibleBlankSamples = firstLatestTextVisible
    ? samples.filter(sample =>
      sample.sinceClickMs > firstLatestTextVisible.sinceClickMs &&
      sample.hasRoot &&
      sample.hasScroller &&
      sample.visibleItemCount === 0
    )
    : [];
  const postLatestTextVisibleBlankGaps = postLatestTextVisibleBlankSamples
    .map(sample => sample.largestBlankGapPx)
    .filter((value): value is number => typeof value === 'number');
  const postLatestTextVisibleBottomBlanks = postLatestTextVisibleBlankSamples
    .map(sample => sample.bottomBlankPx)
    .filter((value): value is number => typeof value === 'number');
  const postLatestTextVisibleLatestContentMissingSamples = firstLatestTextVisible
    ? samples.filter(sample =>
      sample.sinceClickMs > firstLatestTextVisible.sinceClickMs &&
      sample.hasRoot &&
      sample.hasScroller &&
      !sample.latestContentVisible
    )
    : [];

  return {
    firstScrollerAtMs: firstScroller?.sinceClickMs ?? null,
    firstScrollerBlankAtMs: firstScrollerBlank?.sinceClickMs ?? null,
    firstVisibleItemAtMs: firstVisibleItem?.sinceClickMs ?? null,
    firstHistoryPlaceholderAtMs: firstHistoryPlaceholder?.sinceClickMs ?? null,
    firstLatestVisibleAtMs: firstLatestVisible?.sinceClickMs ?? null,
    firstLatestContentVisibleAtMs: firstLatestContentVisible?.sinceClickMs ?? null,
    firstLatestTextVisibleAtMs: firstLatestTextVisible?.sinceClickMs ?? null,
    firstLatestVisibleTextlessAtMs: firstLatestVisibleTextless?.sinceClickMs ?? null,
    firstLatestContentVisibleTextlessAtMs: firstLatestContentVisibleTextless?.sinceClickMs ?? null,
    latestTextDelayAfterVisibleMs: (
      firstLatestVisible && firstLatestTextVisible
        ? firstLatestTextVisible.sinceClickMs - firstLatestVisible.sinceClickMs
        : null
    ),
    latestTextDelayAfterContentVisibleMs: (
      firstLatestContentVisible && firstLatestTextVisible
        ? firstLatestTextVisible.sinceClickMs - firstLatestContentVisible.sinceClickMs
        : null
    ),
    latestVisibleTextlessSampleCount: latestVisibleTextlessSamples.length,
    latestContentVisibleTextlessSampleCount: latestContentVisibleTextlessSamples.length,
    maxTextlessVisibleBlankGapPx: textlessBlankGaps.length > 0
      ? Math.max(...textlessBlankGaps)
      : null,
    maxTextlessVisibleBottomBlankPx: textlessBottomBlanks.length > 0
      ? Math.max(...textlessBottomBlanks)
      : null,
    preLatestTextVisibleBlankSampleCount: preLatestTextVisibleBlankSamples.length,
    preLatestTextVisibleBlankWithoutPlaceholderSampleCount: preLatestTextVisibleBlankWithoutPlaceholderSamples.length,
    maxPreLatestTextVisibleBlankGapPx: preLatestTextVisibleBlankGaps.length > 0
      ? Math.max(...preLatestTextVisibleBlankGaps)
      : null,
    maxPreLatestTextVisibleBlankWithoutPlaceholderGapPx: preLatestTextVisibleBlankWithoutPlaceholderGaps.length > 0
      ? Math.max(...preLatestTextVisibleBlankWithoutPlaceholderGaps)
      : null,
    maxPreLatestTextVisibleBottomBlankPx: preLatestTextVisibleBottomBlanks.length > 0
      ? Math.max(...preLatestTextVisibleBottomBlanks)
      : null,
    postLatestTextVisibleBlankSampleCount: postLatestTextVisibleBlankSamples.length,
    maxPostLatestTextVisibleBlankGapPx: postLatestTextVisibleBlankGaps.length > 0
      ? Math.max(...postLatestTextVisibleBlankGaps)
      : null,
    maxPostLatestTextVisibleBottomBlankPx: postLatestTextVisibleBottomBlanks.length > 0
      ? Math.max(...postLatestTextVisibleBottomBlanks)
      : null,
    postLatestTextVisibleLatestContentMissingSampleCount: postLatestTextVisibleLatestContentMissingSamples.length,
  };
}

async function waitForLatestLongSessionViewportUsable(timeoutMs: number, expectedLatestTurnId?: string | null): Promise<{
  usableAtMs: number;
  viewport: LongSessionViewportState;
}> {
  let viewport = await readLongSessionViewportState(expectedLatestTurnId);
  try {
    await browser.waitUntil(async () => {
      viewport = await readLongSessionViewportState(expectedLatestTurnId);
      return isLongSessionViewportUsable(viewport);
    }, {
      timeout: timeoutMs,
      interval: 50,
      timeoutMsg: 'latest long-session viewport did not become usable',
    });
  } catch (error) {
    viewport = await readLongSessionViewportState(expectedLatestTurnId);
    const snapshot = await readStartupTraceSnapshot().catch(() => null);
    const relatedEvents = snapshot?.phases.events
      .filter(event =>
        event.phase.includes('latest_anchor') ||
        event.phase.includes('latest_end_anchor') ||
        event.phase.includes('turn_pin')
      )
      .slice(-30) ?? [];
    throw new Error(
      `${error instanceof Error ? error.message : String(error)}; ` +
      `viewport=${JSON.stringify(viewport)}; ` +
      `relatedEvents=${JSON.stringify(relatedEvents)}`,
    );
  }

  return {
    usableAtMs: await readPerformanceNow(),
    viewport,
  };
}

type LongSessionOpenMeasurement = {
  appMode: string;
  sessionId: string;
  fixtureScenario: string | null;
  expectedLatestTurnId: string | null;
  clickedAtMs: number;
  sessionOpen: ReturnType<typeof summarizeSessionOpen>;
  latestVisibleAtMs: number;
  clickToLatestVisibleMs: number;
  latestUsableAtMs: number;
  clickToLatestUsableMs: number;
  latestAnswerTextVisibleAtMs: number;
  clickToLatestAnswerTextVisibleMs: number;
  finalViewportCheckedAtMs: number;
  postHydrateUsableAtMs?: number;
  clickToPostHydrateUsableMs?: number;
  latestVisibleViewport: LongSessionViewportState;
  latestUsableViewport: LongSessionViewportState;
  latestAnswerTextVisibleViewport: LongSessionViewportState;
  viewport: LongSessionViewportState;
  viewportTimeline: LongSessionViewportTimelineSample[];
  viewportTimelineSummary: LongSessionViewportTimelineSummary;
  mainThreadTasks: LongSessionMainThreadTask[];
  screenshotPath: string | null;
  events: StartupTraceSnapshot['phases']['events'];
  apiSegments: ReturnType<typeof summarizeApiCommandSegments>;
  api: StartupTraceSnapshot['api'];
  native: StartupTraceSnapshot['native'];
};

type LongSessionOpenMeasurementOptions = {
  requireFrameTrace?: boolean;
};

async function collectLongSessionOpenMeasurement(
  sessionId: string,
  expectedLatestTurnId: string | null,
  options: LongSessionOpenMeasurementOptions = {},
): Promise<LongSessionOpenMeasurement | null> {
  await switchAwayFromSession(sessionId);

  const item = await findSessionItem(sessionId);
  if (!item) {
    return null;
  }
  if (!expectedLatestTurnId) {
    throw new Error(`Could not resolve expected latest turn id for session ${sessionId}`);
  }
  const fixtureScenario = await readLongSessionFixtureScenario(sessionId);

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
  const latestAnchorAttemptCountBefore = countPhase(
    beforeClickSnapshot,
    'historical_session_latest_anchor_attempt',
  );
  const requireFrameTrace = options.requireFrameTrace !== false;
  const afterFrameTimeoutMs = requireFrameTrace ? 20000 : 1000;
  const fullHydrateTimeoutMs = requireFrameTrace ? 10000 : 1000;
  const latestAnchorTimeoutMs = requireFrameTrace ? 5000 : 1000;
  const clickedAtMs = await readPerformanceNow();
  await startLongSessionViewportTimelineRecorder(
    expectedLatestTurnId,
    clickedAtMs,
    process.env.BITFUN_E2E_RENDER_PROFILE === '1',
  );

  await item.click();
  const latestVisiblePromise = waitForLatestLongSessionTurnVisible(5000, expectedLatestTurnId);
  const latestUsablePromise = waitForLatestLongSessionViewportUsable(5000, expectedLatestTurnId);

  const afterFrameSnapshot = requireFrameTrace
    ? await waitForTracePhaseCount(
      'historical_session_after_state_commit_frame',
      frameCountBefore + 1,
      afterFrameTimeoutMs,
    )
    : await waitForOptionalPhaseCount(
      'historical_session_after_state_commit_frame',
      frameCountBefore + 1,
      afterFrameTimeoutMs,
    );
  const afterFullSnapshot = await waitForOptionalPhaseCount(
    'historical_session_full_hydrate_end',
    fullHydrateCountBefore + 1,
    fullHydrateTimeoutMs,
  );
  const afterFullFrameSnapshot = await waitForOptionalPhaseCount(
    'historical_session_full_hydrate_after_state_commit_frame',
    fullHydrateFrameCountBefore + 1,
    fullHydrateTimeoutMs,
  );
  const afterAnchorSnapshot = await waitForOptionalPhaseCount(
    'historical_session_latest_anchor_attempt',
    latestAnchorAttemptCountBefore + 1,
    latestAnchorTimeoutMs,
  );
  const latestVisible = await latestVisiblePromise;
  const latestUsable = await latestUsablePromise;
  let finalViewport = await readLongSessionViewportState(expectedLatestTurnId);
  let finalViewportCheckedAtMs = await readPerformanceNow();
  if (!isLongSessionViewportUsable(finalViewport)) {
    const finalUsable = await waitForLatestLongSessionViewportUsable(
      3000,
      expectedLatestTurnId,
    );
    finalViewport = finalUsable.viewport;
    finalViewportCheckedAtMs = finalUsable.usableAtMs;
  }
  const viewportTimeline = await stopLongSessionViewportTimelineRecorder();
  const viewportTimelineSummary = summarizeLongSessionViewportTimeline(viewportTimeline.samples);
  const finalSnapshot = await readStartupTraceSnapshot()
    .catch(() => [
      afterFrameSnapshot,
      afterFullSnapshot,
      afterFullFrameSnapshot,
      afterAnchorSnapshot,
    ].reduce((latest, snapshot) =>
      snapshot.phases.events.length >= latest.phases.events.length ? snapshot : latest
    ));
  const sessionEvents = finalSnapshot.phases.events.filter(event =>
    event.atMs >= clickedAtMs &&
    (
      event.phase.startsWith('historical_session') ||
      event.phase.startsWith('flowchat_latest_end_anchor') ||
      event.phase === 'react_render_profile'
    )
  );
  const screenshotPath = await maybeSavePerfScreenshot(`long-session-${sessionId}`);

  return {
    appMode: process.env.BITFUN_E2E_APP_MODE ?? 'auto',
    sessionId,
    fixtureScenario,
    expectedLatestTurnId,
    clickedAtMs,
    sessionOpen: summarizeSessionOpen(sessionEvents, clickedAtMs),
    latestVisibleAtMs: latestVisible.visibleAtMs,
    clickToLatestVisibleMs: latestVisible.visibleAtMs - clickedAtMs,
    latestUsableAtMs: latestUsable.usableAtMs,
    clickToLatestUsableMs: latestUsable.usableAtMs - clickedAtMs,
    latestAnswerTextVisibleAtMs: latestUsable.usableAtMs,
    clickToLatestAnswerTextVisibleMs: latestUsable.usableAtMs - clickedAtMs,
    finalViewportCheckedAtMs,
    ...(requireFrameTrace
      ? {
        postHydrateUsableAtMs: finalViewportCheckedAtMs,
        clickToPostHydrateUsableMs: finalViewportCheckedAtMs - clickedAtMs,
      }
      : {}),
    latestVisibleViewport: latestVisible.viewport,
    latestUsableViewport: latestUsable.viewport,
    latestAnswerTextVisibleViewport: latestUsable.viewport,
    viewport: finalViewport,
    viewportTimeline: viewportTimeline.samples,
    viewportTimelineSummary,
    mainThreadTasks: viewportTimeline.mainThreadTasks,
    screenshotPath,
    events: sessionEvents,
    apiSegments: summarizeApiCommandSegments(finalSnapshot),
    api: finalSnapshot.api,
    native: finalSnapshot.native,
  };
}

function expectLongSessionMeasurementUsable(
  measurement: LongSessionOpenMeasurement,
  maxLatestFrameMs?: number,
  options: LongSessionOpenMeasurementOptions = {},
): void {
  expect(measurement.clickToLatestVisibleMs).toBeGreaterThan(0);
  expect(measurement.clickToLatestUsableMs).toBeGreaterThan(0);
  if (options.requireFrameTrace !== false) {
    expect(measurement.clickToPostHydrateUsableMs).toBeGreaterThan(0);
  }
  if (options.requireFrameTrace !== false) {
    expect(measurement.sessionOpen.hydrateDurationMs).toBeGreaterThan(0);
    expect(measurement.sessionOpen.latestFrameSinceHydrateMs).toBeGreaterThan(0);
    expect(measurement.sessionOpen.clickToLatestFrameMs).toBeGreaterThan(0);
  }
  expect(measurement.viewport.hasScroller).toBe(true);
  expect(measurement.viewport.latestContentVisible).toBe(true);
  expect(measurement.viewport.latestModelRoundVisible).toBe(true);
  expect(measurement.viewport.latestModelRoundTextLength).toBeGreaterThan(0);
  expect(measurement.viewport.latestTurnId).toBe(measurement.expectedLatestTurnId);
  expect(measurement.latestVisibleViewport.hasScroller).toBe(true);
  expect(measurement.latestVisibleViewport.latestContentVisible).toBe(true);
  expect(measurement.latestVisibleViewport.latestModelRoundVisible).toBe(true);
  expect(measurement.latestVisibleViewport.latestTurnId).toBe(measurement.expectedLatestTurnId);
  expect(isLongSessionLatestVisibleViewportPositioned(measurement.latestVisibleViewport)).toBe(true);
  expect(measurement.latestAnswerTextVisibleViewport.latestModelRoundVisible).toBe(true);
  expect(measurement.latestAnswerTextVisibleViewport.latestModelRoundTextLength).toBeGreaterThan(0);
  expect(isLongSessionViewportUsable(measurement.latestAnswerTextVisibleViewport)).toBe(true);
  if (measurement.viewportTimelineSummary.latestTextDelayAfterContentVisibleMs !== null) {
    expect(measurement.viewportTimelineSummary.latestTextDelayAfterContentVisibleMs)
      .toBeLessThanOrEqual(LONG_SESSION_MAX_LATEST_TEXT_DELAY_AFTER_VISIBLE_MS);
  }
  expect(measurement.viewportTimelineSummary.preLatestTextVisibleBlankWithoutPlaceholderSampleCount).toBe(0);
  expect(measurement.viewportTimelineSummary.postLatestTextVisibleBlankSampleCount).toBe(0);
  expect(measurement.viewportTimelineSummary.postLatestTextVisibleLatestContentMissingSampleCount).toBe(0);
  if (measurement.fixtureScenario === 'mixed-visible') {
    expect(measurement.latestVisibleViewport.visibleModelRoundCount).toBeGreaterThan(0);
  }
  expect(isLongSessionInputAnchoredNearBottom(measurement.latestVisibleViewport)).toBe(true);
  expect(isLongSessionViewportUsable(measurement.viewport)).toBe(true);
  expect(isLongSessionInputAnchoredNearBottom(measurement.viewport)).toBe(true);
  if (
    maxLatestFrameMs !== undefined &&
    measurement.sessionOpen.latestFrameSinceHydrateMs !== undefined
  ) {
    expect(measurement.sessionOpen.latestFrameSinceHydrateMs).toBeLessThanOrEqual(maxLatestFrameMs);
  }
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
    const apiSegments = summarizeApiCommandSegments(snapshot);
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
      apiSegments,
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
    const expectedLatestTurnId = await readExpectedLatestTurnId(sessionId);
    const measurement = await collectLongSessionOpenMeasurement(
      sessionId,
      expectedLatestTurnId,
      { requireFrameTrace: true },
    );
    if (!measurement) {
      if (expectedLatestTurnId) {
        throw new Error(`Session ${sessionId} exists on disk but was not reachable from the session navigation.`);
      }
      console.log(`[Perf] Session ${sessionId} not found; generate it before running this spec.`);
      this.skip();
      return;
    }
    const maxLatestFrameMs = numericEnv('BITFUN_E2E_PERF_MAX_SESSION_FRAME_MS');

    console.log('[Perf] long-session-first-open', JSON.stringify({
      appMode: measurement.appMode,
      sessionId,
      fixtureScenario: measurement.fixtureScenario,
      sessionOpen: measurement.sessionOpen,
    }));

    await writeReport('long-session-first-open', measurement);
    expectLongSessionMeasurementUsable(measurement, maxLatestFrameMs, { requireFrameTrace: true });
  });

  it('collects warm-reopen timing for a generated long session', async function () {
    const sessionId = process.env.BITFUN_E2E_PERF_SESSION_ID || DEFAULT_PERF_SESSION_ID;
    const expectedLatestTurnId = await readExpectedLatestTurnId(sessionId);
    const measurement = await collectLongSessionOpenMeasurement(
      sessionId,
      expectedLatestTurnId,
      { requireFrameTrace: false },
    );
    if (!measurement) {
      if (expectedLatestTurnId) {
        throw new Error(`Session ${sessionId} exists on disk but was not reachable from the session navigation.`);
      }
      console.log(`[Perf] Session ${sessionId} not found; generate it before running this spec.`);
      this.skip();
      return;
    }
    const maxLatestFrameMs = numericEnv('BITFUN_E2E_PERF_MAX_SESSION_FRAME_MS');

    console.log('[Perf] long-session-warm-reopen', JSON.stringify({
      appMode: measurement.appMode,
      sessionId,
      fixtureScenario: measurement.fixtureScenario,
      sessionOpen: measurement.sessionOpen,
    }));

    await writeReport('long-session-warm-reopen', measurement);
    expectLongSessionMeasurementUsable(measurement, maxLatestFrameMs, { requireFrameTrace: false });
  });
});
