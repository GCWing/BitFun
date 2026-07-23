import { $, browser, expect } from '@wdio/globals';
import * as crypto from 'crypto';
import * as fs from 'fs/promises';
import * as fsSync from 'fs';
import * as os from 'os';
import * as path from 'path';
import { performance as nodePerformance } from 'node:perf_hooks';
import { fileURLToPath } from 'url';
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
import { readStartupResourceTimingSummary } from '../../helpers/performance-resource-timing';
import { StartupPage } from '../../page-objects/StartupPage';
import { ensureWorkspaceOpen } from '../../helpers/workspace-utils';
import { openWorkspace } from '../../helpers/workspace-helper';

const DEFAULT_PERF_SESSION_ID = 'perf-long-session-000';
const MAX_PROJECT_SLUG_LEN = 120;
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const LONG_SESSION_VIEWPORT_MIN_COVERAGE_RATIO = 0.7;
const LONG_SESSION_VIEWPORT_MAX_BOTTOM_BLANK_PX = 64;
const LONG_SESSION_VIEWPORT_MAX_BLANK_GAP_PX = 64;
const LONG_SESSION_LATEST_VISIBLE_MAX_BOTTOM_BLANK_PX = 96;
const LONG_SESSION_LATEST_VISIBLE_MAX_BLANK_GAP_PX = 96;
const LONG_SESSION_LATEST_TAIL_BOTTOM_TOLERANCE_PX = 96;
const LONG_SESSION_PHYSICAL_BOTTOM_TOLERANCE_PX = 4;
const LONG_SESSION_RESIZE_BOTTOM_SETTLE_MAX_MS = 120;
const LONG_SESSION_INPUT_MIN_TOP_RATIO = 0.65;
const LONG_SESSION_INPUT_BOTTOM_TOLERANCE_PX = 96;
const LONG_SESSION_MAX_LATEST_TEXT_DELAY_AFTER_VISIBLE_MS = 120;
const SESSION_NAV_ITEM_SELECTOR =
  '[data-testid="session-nav-item"], [data-testid="nav-session-item"]';
const SESSION_NAV_TOGGLE_SELECTOR =
  '[data-testid="session-nav-show-more"], [data-testid="nav-session-list-toggle"]';

type LongSessionPostVisibleInteraction =
  | 'first-scroll'
  | 'scroll-down'
  | 'resize-window'
  | 'resize-window-width';

function defaultBitfunHome(): string {
  if (process.env.BITFUN_E2E_HOME) {
    return process.env.BITFUN_E2E_HOME;
  }
  if (process.env.BITFUN_E2E_USE_REAL_PROFILE === '1') {
    return process.env.BITFUN_HOME || path.join(os.homedir(), '.bitfun');
  }
  return path.resolve(__dirname, '..', '..', '.bitfun', 'runtime', 'home');
}

type LongSessionWindowRect = {
  x?: number;
  y?: number;
  width: number;
  height: number;
};

type LongSessionPostVisibleInteractionResult = {
  type: LongSessionPostVisibleInteraction;
  beforeScrollTop: number;
  afterScrollTop: number;
  maxScrollTop: number;
  deltaY: number;
  beforeClientHeight?: number;
  afterClientHeight?: number;
  beforeWindowRect?: LongSessionWindowRect;
  afterWindowRect?: LongSessionWindowRect;
};

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
  historyPlaceholderCoversMessages: boolean;
  latestContentVisuallyVisible: boolean;
  scrollerTop: number | null;
  scrollerBottom: number | null;
  effectiveScrollerBottom: number | null;
  inputOverlayTop: number | null;
  inputOverlayBottom: number | null;
  inputOverlayHeight: number | null;
  staticInitialHistoryWindowed: boolean | null;
  staticInitialHistorySpacerHeight: number | null;
  staticItemsTop: number | null;
  staticItemsBottom: number | null;
  footerTop: number | null;
  footerHeight: number | null;
  lastRenderedItemTop: number | null;
  lastRenderedItemBottom: number | null;
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

type LongSessionViewportUsabilityOptions = {
  requireLatestModelRound?: boolean;
};

type LongSessionViewportTimelineSample = {
  atMs: number;
  sinceClickMs: number;
  historyOpenIntentAtMs: number | null;
  historyOpenIntentSessionId: string | null;
  sinceHistoryOpenIntentMs: number | null;
  activeSessionId: string | null;
  pendingHistoryOpenSessionId: string | null;
  hasRoot: boolean;
  hasScroller: boolean;
  latestRendered: boolean;
  latestModelRoundRendered: boolean;
  latestModelRoundVisible: boolean;
  latestModelRoundTextLength: number;
  latestContentVisible: boolean;
  historyPlaceholderVisible: boolean;
  historyPlaceholderCoversMessages: boolean;
  latestContentVisuallyVisible: boolean;
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

type LongSessionLayoutShiftEvent = {
  startMs: number;
  sinceClickMs: number;
  value: number;
  hadRecentInput: boolean;
  sources: Array<{
    node: string | null;
    previousRect: LongSessionRectSummary | null;
    currentRect: LongSessionRectSummary | null;
  }>;
};

type LongSessionRectSummary = {
  top: number;
  bottom: number;
  left: number;
  right: number;
  width: number;
  height: number;
};

type LongSessionLoadingSurface = {
  selector: string;
  node: string | null;
  visible: boolean;
  textLength: number;
  rect: LongSessionRectSummary | null;
};

type LongSessionSurfacePoint = {
  label: string;
  x: number;
  y: number;
  category: string;
  topNode: string | null;
  topElementId: string | null;
  itemType: string | null;
  turnId: string | null;
  latestTurnHit: boolean;
  effectiveOpacity: number;
  textLength: number;
  modelRoundGroupCount: number | null;
  modelRoundRenderedGroupCount: number | null;
  modelRoundVisibleGroupStart: number | null;
  modelRoundVisibleGroupEnd: number | null;
  modelRoundRenderAll: string | null;
  modelRoundHasDeferredEarlier: string | null;
  modelRoundHasDeferredLater: string | null;
  backgroundChain: string[];
};

type LongSessionVirtualItemSummary = {
  elementId: string;
  virtualIndex: number | null;
  itemType: string | null;
  turnId: string | null;
  textLength: number;
  modelRoundGroupCount: number | null;
  modelRoundRenderedGroupCount: number | null;
  modelRoundVisibleGroupStart: number | null;
  modelRoundVisibleGroupEnd: number | null;
  modelRoundRenderAll: string | null;
  modelRoundHasDeferredEarlier: string | null;
  modelRoundHasDeferredLater: string | null;
  codeBlockWrapperCount: number;
  codeBlockFallbackCount: number;
  highlightedCodeBlockCount: number;
  tableCount: number;
  completedToolTransitionCount: number;
  completedToolTransitionHeight: number;
  completedToolTransitionMaxHeights: string[];
  completedToolTransitionAnimations: string[];
  completedToolTransitionOpacities: string[];
  rect: LongSessionRectSummary | null;
};

type LongSessionDomMutationEvent = {
  atMs: number;
  sinceClickMs: number;
  reason: string;
  activeSessionId: string | null;
  historyState: string | null;
  contextRestoreState: string | null;
  isPartial: string | null;
  dialogTurnCount: number | null;
  virtualItemCount: number | null;
  showHistoryPlaceholder: string | null;
  showHistoryTransitionOverlay: string | null;
  showHistoryLoadingLayer: string | null;
  showHistoryOpenIntentOverlay: string | null;
  hasPendingHistoryCompletion: string | null;
  hasDeferredHistoryProjection: string | null;
  latestTurnId: string | null;
  historyInitialContentReady: string | null;
  pendingHistoryOpenSessionId: string | null;
  placeholderCount: number;
  overlayCount: number;
  historyInitialPreviewCount: number;
  historyInitialPreviewVisible: boolean;
  historyProjectionHandoffCount: number;
  historyProjectionHandoffVisible: boolean;
  virtualListCount: number;
  virtualItemDomCount: number;
  visibleLoadingSurfaceCount: number;
  visibleTextLength: number;
  loadingSurfaces: LongSessionLoadingSurface[];
  overlayElementIds: string[];
  placeholderElementIds: string[];
  virtualListElementIds: string[];
  virtualItemElementIds: string[];
  virtualItemSummaries: LongSessionVirtualItemSummary[];
  surfaceSignature: string;
  surfacePoints: LongSessionSurfacePoint[];
  messagesRect: LongSessionRectSummary | null;
  scrollerRect: LongSessionRectSummary | null;
  overlayRect: LongSessionRectSummary | null;
  placeholderRect: LongSessionRectSummary | null;
  messagesBackground: string | null;
  overlayBackground: string | null;
  containerBackground: string | null;
  bodyBackground: string | null;
  htmlBackground: string | null;
};

type LongSessionVisualStateEvent = {
  atMs: number;
  sinceClickMs: number;
  historyOpenIntentSessionId: string | null;
  sinceHistoryOpenIntentMs: number | null;
  frame: number;
  reason: string;
  signature: string;
  activeSessionId: string | null;
  historyState: string | null;
  contextRestoreState: string | null;
  isPartial: string | null;
  dialogTurnCount: number | null;
  virtualItemCount: number | null;
  showHistoryPlaceholder: string | null;
  showHistoryTransitionOverlay: string | null;
  showHistoryLoadingLayer: string | null;
  showHistoryOpenIntentOverlay: string | null;
  hasPendingHistoryCompletion: string | null;
  hasDeferredHistoryProjection: string | null;
  latestTurnId: string | null;
  historyInitialContentReady: string | null;
  pendingHistoryOpenSessionId: string | null;
  placeholderCount: number;
  overlayCount: number;
  historyInitialPreviewCount: number;
  historyInitialPreviewVisible: boolean;
  historyProjectionHandoffCount: number;
  historyProjectionHandoffVisible: boolean;
  virtualListCount: number;
  virtualItemDomCount: number;
  visibleLoadingSurfaceCount: number;
  virtualListElementIds: string[];
  virtualItemElementIds: string[];
  virtualItemSummaries: LongSessionVirtualItemSummary[];
  completedToolTransitionCount: number;
  completedToolTransitionHeight: number;
  completedToolTransitionSignature: string;
  visibleTextLength: number;
  loadingSurfaces: LongSessionLoadingSurface[];
  latestContentVisuallyVisible: boolean;
  latestModelRoundTextLength: number;
  surfaceSignature: string;
  surfacePoints: LongSessionSurfacePoint[];
  scrollerExists: boolean;
  scrollTop: number | null;
  scrollHeight: number | null;
  clientHeight: number | null;
  footerHeight: number | null;
  footerStyleHeight: string | null;
  footerStyleMinHeight: string | null;
  inputOverlayHeight: number | null;
  messagesRect: LongSessionRectSummary | null;
  scrollerRect: LongSessionRectSummary | null;
  overlayRect: LongSessionRectSummary | null;
  placeholderRect: LongSessionRectSummary | null;
  inputOverlayRect: LongSessionRectSummary | null;
  messagesBackground: string | null;
  overlayBackground: string | null;
  containerBackground: string | null;
  bodyBackground: string | null;
  htmlBackground: string | null;
};

type LongSessionViewportTimeline = {
  samples: LongSessionViewportTimelineSample[];
  mainThreadTasks: LongSessionMainThreadTask[];
  mutationEvents: LongSessionDomMutationEvent[];
  visualStateEvents: LongSessionVisualStateEvent[];
  layoutShiftEvents: LongSessionLayoutShiftEvent[];
};

type LongSessionViewportTimelineSummary = {
  firstScrollerAtMs: number | null;
  firstScrollerBlankAtMs: number | null;
  firstVisibleItemAtMs: number | null;
  firstHistoryPlaceholderAtMs: number | null;
  firstLatestVisibleAtMs: number | null;
  firstLatestContentVisibleAtMs: number | null;
  firstLatestContentVisuallyVisibleAtMs: number | null;
  firstLatestTextVisibleAtMs: number | null;
  firstLatestVisibleTextlessAtMs: number | null;
  firstLatestContentVisibleTextlessAtMs: number | null;
  latestTextDelayAfterVisibleMs: number | null;
  latestTextDelayAfterContentVisibleMs: number | null;
  latestTextDelayAfterContentVisuallyVisibleMs: number | null;
  latestVisibleTextlessSampleCount: number;
  latestContentVisibleTextlessSampleCount: number;
  maxTextlessVisibleBlankGapPx: number | null;
  maxTextlessVisibleBottomBlankPx: number | null;
  preLatestTextVisibleBlankSampleCount: number;
  preLatestTextVisibleBlankWithoutPlaceholderSampleCount: number;
  preLatestTextVisibleUncoveredAfterIntentSampleCount: number;
  firstPreLatestTextVisibleUncoveredAfterIntentAtMs: number | null;
  maxPreLatestTextVisibleBlankGapPx: number | null;
  maxPreLatestTextVisibleBlankWithoutPlaceholderGapPx: number | null;
  maxPreLatestTextVisibleBottomBlankPx: number | null;
  postLatestTextVisibleBlankSampleCount: number;
  postLatestTextVisibleCoveredSampleCount: number;
  maxPostLatestTextVisibleBlankGapPx: number | null;
  maxPostLatestTextVisibleBottomBlankPx: number | null;
  postLatestTextVisibleLatestContentMissingSampleCount: number;
};

type LongSessionVisualStateSummary = {
  visualStateEventCount: number;
  mutationEventCount: number;
  firstVisualStateAtMs: number | null;
  firstLoadingLayerAtMs: number | null;
  lastLoadingLayerAtMs: number | null;
  historyInitialPreviewVisibleAtEnd: boolean;
  historyProjectionHandoffVisibleAtEnd: boolean;
  historyInitialPreviewActivationAfterActiveSessionCount: number;
  loadingLayerToggleCount: number;
  overlayCountToggleCount: number;
  placeholderCountToggleCount: number;
  overlayElementIdChangeCount: number;
  placeholderElementIdChangeCount: number;
  virtualListElementIdChangeCount: number;
  virtualItemElementIdChangeCount: number;
  backgroundChangeCount: number;
  scrollerScrollJumpCount: number;
  scrollerSizeChangeCount: number;
  layoutShiftCount: number;
  layoutShiftScore: number;
  postLatestTextVisibleVisualChangeCount: number;
  postLatestTextVisibleLoadingEventCount: number;
  postFirstVisibleItemVisualChangeCount: number;
  postFirstVisibleItemLoadingEventCount: number;
  postLatestTextVisibleBackgroundChangeCount: number;
  postLatestTextVisibleScrollJumpCount: number;
  postLatestTextVisibleVirtualItemElementChangeCount: number;
  postLatestTextVisibleSurfaceChangeCount: number;
  postLatestTextVisibleLoadingSurfacePointEventCount: number;
  postLatestTextVisibleBlankSurfacePointEventCount: number;
  postLatestTextVisibleTransparentSurfacePointEventCount: number;
  postLatestTextVisibleLayoutShiftCount: number;
  postLatestTextVisibleLayoutShiftScore: number;
  firstUserInteractionAtMs: number | null;
  postUserInteractionScrollJumpCount: number;
  postUserInteractionScrollerCollapseCount: number;
  postUserInteractionBlankSurfacePointEventCount: number;
  openIntentBlankSurfacePointEventCount: number;
  openIntentBlankSurfaceHoldCount: number;
  maxOpenIntentBlankSurfaceHoldMs: number | null;
  postOpenIntentNonTargetContentEventCount: number;
  postOpenIntentNonTargetContentHoldCount: number;
  maxPostOpenIntentNonTargetContentHoldMs: number | null;
  lastPostOpenIntentNonTargetContentAtMs: number | null;
  loadingTransitions: Array<{
    sinceClickMs: number;
    reason: string;
    showHistoryLoadingLayer: string | null;
    overlayCount: number;
    placeholderCount: number;
    visibleLoadingSurfaceCount: number;
    loadingSurfaces: LongSessionLoadingSurface[];
    virtualItemDomCount: number;
    latestContentVisuallyVisible: boolean;
  }>;
  backgroundTransitions: Array<{
    sinceClickMs: number;
    reason: string;
    from: string;
    to: string;
  }>;
  historyInitialPreviewTransitions: Array<{
    sinceClickMs: number;
    reason: string;
    from: boolean;
    to: boolean;
  }>;
  overlayElementTransitions: Array<{
    sinceClickMs: number;
    reason: string;
    from: string[];
    to: string[];
  }>;
  virtualItemElementTransitions: Array<{
    sinceClickMs: number;
    reason: string;
    from: string[];
    to: string[];
  }>;
  surfaceTransitions: Array<{
    sinceClickMs: number;
    reason: string;
    from: string;
    to: string;
    surfacePoints: LongSessionSurfacePoint[];
  }>;
  postOpenIntentNonTargetContentEvents: Array<{
    sinceClickMs: number;
    historyOpenIntentSessionId: string | null;
    sinceHistoryOpenIntentMs: number | null;
    reason: string;
    activeSessionId: string | null;
    pendingHistoryOpenSessionId: string | null;
    virtualItemDomCount: number;
    visibleTextLength: number;
    surfacePoints: LongSessionSurfacePoint[];
  }>;
  postOpenIntentNonTargetContentHolds: Array<{
    sinceClickMs: number;
    historyOpenIntentSessionId: string | null;
    sinceHistoryOpenIntentMs: number | null;
    untilHistoryOpenIntentMs: number | null;
    holdAfterGraceMs: number;
    reason: string;
    activeSessionId: string | null;
    virtualItemDomCount: number;
    visibleTextLength: number;
  }>;
  scrollTransitions: Array<{
    sinceClickMs: number;
    reason: string;
    fromScrollTop: number | null;
    toScrollTop: number | null;
    fromScrollHeight: number | null;
    toScrollHeight: number | null;
    fromClientHeight: number | null;
    toClientHeight: number | null;
    fromFooterHeight: number | null;
    toFooterHeight: number | null;
    fromFooterStyleHeight: string | null;
    toFooterStyleHeight: string | null;
  }>;
  postUserInteractionScrollTransitions: Array<{
    sinceClickMs: number;
    reason: string;
    fromScrollTop: number | null;
    toScrollTop: number | null;
    fromScrollHeight: number | null;
    toScrollHeight: number | null;
    fromClientHeight: number | null;
    toClientHeight: number | null;
    fromFooterHeight: number | null;
    toFooterHeight: number | null;
    fromFooterStyleHeight: string | null;
    toFooterStyleHeight: string | null;
  }>;
  layoutShiftEvents: LongSessionLayoutShiftEvent[];
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

function traceEventSessionId(event: StartupTraceSnapshot['phases']['events'][number]): string | null {
  return typeof event.sessionId === 'string' ? event.sessionId : null;
}

function traceEventMatchesSessionPhaseSince(
  event: StartupTraceSnapshot['phases']['events'][number],
  phase: string,
  sessionId: string,
  sinceMs: number,
): boolean {
  return (
    event.phase === phase &&
    event.atMs >= sinceMs &&
    traceEventSessionId(event) === sessionId
  );
}

function numericEnv(name: string): number | undefined {
  const raw = process.env[name];
  if (!raw) {
    return undefined;
  }
  const value = Number(raw);
  return Number.isFinite(value) ? value : undefined;
}

async function assertReleaseFastPerfRuntime(): Promise<{
  runtimeUrl: string;
  runtimeHostname: string;
}> {
  const runtime = await browser.execute(() => ({
    runtimeUrl: window.location.href,
    runtimeHostname: window.location.hostname,
  }));
  const appMode = (process.env.BITFUN_E2E_APP_MODE ?? '').toLowerCase();
  const allowDevServer = process.env.BITFUN_E2E_ALLOW_RELEASE_FAST_DEV_SERVER === '1';
  const isDevServerRuntime =
    runtime.runtimeHostname === 'localhost' || runtime.runtimeHostname === '127.0.0.1';

  if (appMode === 'release-fast' && isDevServerRuntime && !allowDevServer) {
    throw new Error(
      `release-fast perf run loaded a dev-server URL: ${runtime.runtimeUrl}. ` +
        'Build with pnpm run desktop:build:release-fast, or set ' +
        'BITFUN_E2E_ALLOW_RELEASE_FAST_DEV_SERVER=1 only for explicit dev-server diagnostics.',
    );
  }

  return runtime;
}

const DEFAULT_POST_VISIBLE_OBSERVE_MS = 3000;

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

async function waitForTracePhaseForSessionSince(
  phase: string,
  sessionId: string,
  sinceMs: number,
  timeoutMs: number,
): Promise<StartupTraceSnapshot> {
  let latest = await readStartupTraceSnapshot();
  await browser.waitUntil(async () => {
    latest = await readStartupTraceSnapshot();
    return latest.phases.events.some(event =>
      traceEventMatchesSessionPhaseSince(event, phase, sessionId, sinceMs)
    );
  }, {
    timeout: timeoutMs,
    interval: 100,
    timeoutMsg:
      `Timed out waiting for startup trace phase '${phase}' for session '${sessionId}' after ${sinceMs}`,
  });
  return latest;
}

async function waitForOptionalTracePhaseForSessionSince(
  phase: string,
  sessionId: string,
  sinceMs: number,
  timeoutMs: number,
): Promise<StartupTraceSnapshot> {
  try {
    return await waitForTracePhaseForSessionSince(phase, sessionId, sinceMs, timeoutMs);
  } catch {
    return readStartupTraceSnapshot();
  }
}

function escapeCssAttributeValue(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
}

async function findSessionItem(sessionId: string): Promise<ReturnType<typeof $> | null> {
  const targetSelector =
    `[data-testid="session-nav-item"][data-session-id="${escapeCssAttributeValue(sessionId)}"], ` +
    `[data-testid="nav-session-item"][data-session-id="${escapeCssAttributeValue(sessionId)}"]`;
  const readVisibleSessionIds = async (): Promise<string[]> =>
    browser.execute((selector) =>
      Array.from(document.querySelectorAll(selector))
        .map(element => element.getAttribute('data-session-id') || '')
        .filter(Boolean),
    SESSION_NAV_ITEM_SELECTOR);

  const findTarget = async (): Promise<ReturnType<typeof $> | null> => {
    const item = await $(targetSelector);
    return await item.isExisting() ? item : null;
  };

  const findExpandableToggles = async (): Promise<Array<ReturnType<typeof $>>> => {
    const toggles = await browser.$$(SESSION_NAV_TOGGLE_SELECTOR);
    const expandable: Array<ReturnType<typeof $>> = [];
    for (const toggle of toggles) {
      if (
        !(await toggle.isExisting()) ||
        !(await toggle.isDisplayed()) ||
        !(await toggle.isEnabled())
      ) {
        continue;
      }

      const action = await toggle.getAttribute('data-session-nav-toggle-action').catch(() => null);
      if (action === 'show-less') {
        continue;
      }
      expandable.push(toggle);
    }
    return expandable;
  };

  let lastVisibleSessionIds: string[] = [];
  for (let attempt = 0; attempt < 12; attempt += 1) {
    const existing = await findTarget();
    if (existing) {
      return existing;
    }

    lastVisibleSessionIds = await readVisibleSessionIds();
    const toggles = await findExpandableToggles();
    if (toggles.length === 0) {
      break;
    }

    let clickedAny = false;
    for (let toggleIndex = 0; toggleIndex < toggles.length; toggleIndex += 1) {
      const item = await findTarget();
      if (item) {
        return item;
      }

      const currentToggles = await findExpandableToggles();
      const toggle = currentToggles[toggleIndex];
      if (!toggle) {
        break;
      }

      if (
        !(await toggle.isExisting()) ||
        !(await toggle.isDisplayed()) ||
        !(await toggle.isEnabled())
      ) {
        continue;
      }

      const action = await toggle.getAttribute('data-session-nav-toggle-action').catch(() => null);
      if (action === 'show-less') {
        continue;
      }

      const beforeCount = lastVisibleSessionIds.length;
      clickedAny = true;
      await toggle.click();
      await browser.waitUntil(async () => {
        if (await findTarget()) {
          return true;
        }
        const ids = await readVisibleSessionIds();
        const nextToggles = await findExpandableToggles();
        return ids.length !== beforeCount || nextToggles.length !== toggles.length;
      }, { timeout: 3000, interval: 100 }).catch(() => undefined);
      lastVisibleSessionIds = await readVisibleSessionIds();
    }

    if (!clickedAny) {
      break;
    }
  }
  if (process.env.BITFUN_E2E_PERF_VERBOSE_REPORT === '1') {
    console.log('[Perf] visible session ids while locating target', JSON.stringify({
      target: sessionId,
      visibleSessionIds: lastVisibleSessionIds.slice(0, 40),
      visibleSessionCount: lastVisibleSessionIds.length,
    }));
  }
  return null;
}

async function ensurePerformanceWorkspace(startupPage: StartupPage): Promise<boolean> {
  const targetWorkspace = process.env.E2E_TEST_WORKSPACE;
  if (!targetWorkspace) {
    const isBundledApp = await browser.execute(() => window.location.hostname === 'tauri.localhost');
    if (isBundledApp) {
      return true;
    }
    return ensureWorkspaceOpen(startupPage);
  }

  const opened = await openWorkspace(targetWorkspace, { requireWorkspaceLabel: true });
  if (!opened) {
    throw new Error(`Performance workspace did not become active: ${targetWorkspace}`);
  }
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
  const metadataPath = await findLongSessionMetadataPath(sessionId);
  if (!metadataPath) {
    return null;
  }

  try {
    return JSON.parse(await fs.readFile(metadataPath, 'utf8')) as LongSessionMetadata;
  } catch {
    return null;
  }
}

async function findLongSessionMetadataPath(sessionId: string): Promise<string | null> {
  const bitfunHome = defaultBitfunHome();
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
      await fs.access(metadataPath);
      return metadataPath;
    } catch {
      // Try the next known E2E workspace candidate.
    }
  }

  try {
    const projectsDir = path.join(bitfunHome, 'projects');
    const projectEntries = await fs.readdir(projectsDir, { withFileTypes: true });
    for (const entry of projectEntries) {
      if (!entry.isDirectory()) {
        continue;
      }
      const metadataPath = path.join(projectsDir, entry.name, 'sessions', sessionId, 'metadata.json');
      try {
        await fs.access(metadataPath);
        return metadataPath;
      } catch {
        // Try the next persisted project.
      }
    }
  } catch {
    // No global project fallback is available.
  }

  return null;
}

async function readExpectedLatestTurnId(sessionId: string): Promise<string | null> {
  const metadata = await readLongSessionMetadata(sessionId);
  const turnCount = Number(metadata?.turnCount);
  if (!Number.isFinite(turnCount) || turnCount < 1) {
    return null;
  }
  const metadataPath = await findLongSessionMetadataPath(sessionId);
  if (metadataPath) {
    const latestTurnPath = path.join(
      path.dirname(metadataPath),
      'turns',
      `turn-${String(turnCount - 1).padStart(4, '0')}.json`,
    );
    try {
      const latestTurn = JSON.parse(await fs.readFile(latestTurnPath, 'utf8')) as { turnId?: unknown };
      if (typeof latestTurn.turnId === 'string' && latestTurn.turnId.length > 0) {
        return latestTurn.turnId;
      }
    } catch {
      // Fall back to the deterministic fixture id below.
    }
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
  if (!alternateId) {
    return;
  }
  const alternate = await findSessionItem(alternateId);
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
  const clickedAtMs = await readPerformanceNow();
  await alternate.click();
  const afterFrameSnapshot = await waitForOptionalTracePhaseForSessionSince(
    'historical_session_after_state_commit_frame',
    alternateId,
    clickedAtMs,
    10000,
  );
  if (countPhase(afterFrameSnapshot, 'historical_session_after_state_commit_frame') <= frameCountBefore) {
    await waitForOptionalPhaseCount(
      'historical_session_after_state_commit_frame',
      frameCountBefore + 1,
      1000,
    );
  }
  await browser.pause(50);
}

async function readLongSessionViewportState(expectedLatestTurnId?: string | null): Promise<LongSessionViewportState> {
  return browser.execute((targetTurnId) => {
    const effectiveOpacityFor = (element: HTMLElement | null): number => {
      if (!element) {
        return 0;
      }

      let opacity = 1;
      let current: HTMLElement | null = element;
      while (current) {
        const style = window.getComputedStyle(current);
        if (style.display === 'none' || style.visibility === 'hidden') {
          return 0;
        }
        const styleOpacity = Number(style.opacity);
        if (Number.isFinite(styleOpacity)) {
          opacity *= styleOpacity;
        }
        if (opacity <= 0.01) {
          return 0;
        }
        current = current.parentElement;
      }
      return opacity;
    };
    const isElementEffectivelyVisible = (element: HTMLElement | null): boolean => (
      effectiveOpacityFor(element) > 0.01
    );
    const root = document.querySelector<HTMLElement>(
      '.modern-flowchat-container__messages .virtual-message-list',
    );
    const scroller = root?.querySelector<HTMLElement>(
      '[data-virtuoso-scroller="true"], [data-virtuoso-scroller]',
    ) ?? null;
    const staticScroller = root?.querySelector<HTMLElement>('.virtual-message-list__static-scroller') ?? null;
    const staticItems = root?.querySelector<HTMLElement>('.virtual-message-list__static-items') ?? null;
    const staticInitialHistorySpacer = root?.querySelector<HTMLElement>(
      '.virtual-message-list__initial-history-spacer',
    ) ?? null;
    const footer = root?.querySelector<HTMLElement>('.message-list-footer') ?? null;
    const userMessages = Array.from(root?.querySelectorAll<HTMLElement>(
      '.virtual-item-wrapper[data-turn-id][data-item-type="user-message"]',
    ) ?? []);
    const renderedItems = Array.from(root?.querySelectorAll<HTMLElement>(
      '.virtual-item-wrapper[data-turn-id]',
    ) ?? []);
    const renderedLatest = userMessages.length > 0 ? userMessages[userMessages.length - 1] : null;
    const lastRenderedItem = renderedItems.length > 0 ? renderedItems[renderedItems.length - 1] : null;
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
      isElementEffectivelyVisible(historyPlaceholder)
    );
    const effectiveScrollerBottom = scrollerRect
      ? Math.min(scrollerRect.bottom, inputOverlayRect?.top ?? scrollerRect.bottom)
      : null;
    const historyPlaceholderCoversMessages = Boolean(
      historyPlaceholderVisible &&
      historyPlaceholderRect &&
      (
        !scrollerRect ||
        (
          historyPlaceholderRect.top <= scrollerRect.top + 4 &&
          historyPlaceholderRect.bottom >= (effectiveScrollerBottom ?? scrollerRect.bottom) - 4 &&
          historyPlaceholderRect.left <= scrollerRect.left + 4 &&
          historyPlaceholderRect.right >= scrollerRect.right - 4
        )
      )
    );
    const latestRect = latest?.getBoundingClientRect() ?? null;
    const isVisibleWithinScroller = (rect: DOMRect | null, element?: HTMLElement | null): boolean => Boolean(
      scrollerRect &&
      rect &&
      (!element || isElementEffectivelyVisible(element)) &&
      rect.bottom > scrollerRect.top &&
      rect.top < (effectiveScrollerBottom ?? scrollerRect.bottom)
    );
    const latestModelRoundVisibleSegments = latestModelRoundSegments
      .filter(element => isVisibleWithinScroller(element.getBoundingClientRect(), element));
    const latestModelRoundVisible = latestModelRoundVisibleSegments.length > 0;
    const latestModelRoundTextLength = latestModelRoundVisibleSegments
      .reduce((total, element) => total + (element.innerText?.length ?? 0), 0);
    const latestVisible = isVisibleWithinScroller(latestRect, latest);
    const staticItemsRect = staticItems?.getBoundingClientRect() ?? null;
    const footerRect = footer?.getBoundingClientRect() ?? null;
    const lastRenderedItemRect = lastRenderedItem?.getBoundingClientRect() ?? null;
    const latestContentVisible = latestVisible || latestModelRoundVisible;
    const latestContentVisuallyVisible =
      latestContentVisible && !historyPlaceholderCoversMessages;
    const visibleUserMessages = scrollerRect
      ? userMessages.filter(element => {
        const rect = element.getBoundingClientRect();
        return (
          isElementEffectivelyVisible(element) &&
          rect.bottom > scrollerRect.top &&
          rect.top < (effectiveScrollerBottom ?? scrollerRect.bottom)
        );
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
        .filter(({ element, top, bottom }) =>
          isElementEffectivelyVisible(element) &&
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
      latestContentVisible,
      historyPlaceholderVisible,
      historyPlaceholderCoversMessages,
      latestContentVisuallyVisible,
      scrollerTop: scrollerRect?.top ?? null,
      scrollerBottom: scrollerRect?.bottom ?? null,
      effectiveScrollerBottom,
      inputOverlayTop: inputOverlayRect?.top ?? null,
      inputOverlayBottom: inputOverlayRect?.bottom ?? null,
      inputOverlayHeight: inputOverlayRect?.height ?? null,
      staticInitialHistoryWindowed: staticScroller
        ? staticScroller.getAttribute('data-initial-history-render-windowed') === 'true'
        : null,
      staticInitialHistorySpacerHeight: staticInitialHistorySpacer?.offsetHeight ?? null,
      staticItemsTop: staticItemsRect?.top ?? null,
      staticItemsBottom: staticItemsRect?.bottom ?? null,
      footerTop: footerRect?.top ?? null,
      footerHeight: footerRect?.height ?? null,
      lastRenderedItemTop: lastRenderedItemRect?.top ?? null,
      lastRenderedItemBottom: lastRenderedItemRect?.bottom ?? null,
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
      __bitfunLongSessionMutationEvents?: LongSessionDomMutationEvent[];
      __bitfunLongSessionVisualStateEvents?: LongSessionVisualStateEvent[];
      __bitfunLongSessionLayoutShiftEvents?: LongSessionLayoutShiftEvent[];
      __bitfunLongSessionViewportTimelineTimer?: number;
      __bitfunLongSessionVisualFrameRequest?: number;
      __bitfunLongSessionVisualProbeTimers?: number[];
      __bitfunLongSessionLongTaskObserver?: PerformanceObserver;
      __bitfunLongSessionLayoutShiftObserver?: PerformanceObserver;
      __bitfunLongSessionMutationObserver?: MutationObserver;
      __bitfunLongSessionOpenIntentAt?: number;
      __bitfunLongSessionOpenIntentSessionId?: string | null;
      __bitfunLongSessionOpenIntentHandler?: EventListener;
      __bitfunLongSessionUserInteractionHandler?: EventListener;
      __BITFUN_RENDER_PROFILE_ENABLED__?: boolean;
    };
    globalWindow.__BITFUN_RENDER_PROFILE_ENABLED__ = shouldEnableRenderProfile;
    if (globalWindow.__bitfunLongSessionViewportTimelineTimer !== undefined) {
      window.clearInterval(globalWindow.__bitfunLongSessionViewportTimelineTimer);
    }
    if (globalWindow.__bitfunLongSessionVisualFrameRequest !== undefined) {
      window.cancelAnimationFrame(globalWindow.__bitfunLongSessionVisualFrameRequest);
    }
    for (const timerId of globalWindow.__bitfunLongSessionVisualProbeTimers ?? []) {
      window.clearTimeout(timerId);
    }
    globalWindow.__bitfunLongSessionVisualProbeTimers = [];
    globalWindow.__bitfunLongSessionLongTaskObserver?.disconnect();
    globalWindow.__bitfunLongSessionLayoutShiftObserver?.disconnect();
    globalWindow.__bitfunLongSessionMutationObserver?.disconnect();
    if (globalWindow.__bitfunLongSessionOpenIntentHandler) {
      window.removeEventListener(
        'flowchat:history-session-open-intent',
        globalWindow.__bitfunLongSessionOpenIntentHandler,
      );
    }
    if (globalWindow.__bitfunLongSessionUserInteractionHandler) {
      window.removeEventListener(
        'bitfun:e2e-long-session-user-interaction',
        globalWindow.__bitfunLongSessionUserInteractionHandler,
      );
    }
    globalWindow.__bitfunLongSessionOpenIntentAt = undefined;
    globalWindow.__bitfunLongSessionOpenIntentSessionId = undefined;
    const handleOpenIntent: EventListener = event => {
      const detail =
        event instanceof CustomEvent && typeof event.detail?.sessionId === 'string'
          ? event.detail
          : null;
      globalWindow.__bitfunLongSessionOpenIntentAt = performance.now();
      globalWindow.__bitfunLongSessionOpenIntentSessionId = detail?.sessionId ?? null;
      recordMutationEvent('history-open-intent');
      recordVisualStateEvent('history-open-intent', true);
      for (const delayMs of [100, 250, 500, 1_000]) {
        const timerId = window.setTimeout(
          () => recordVisualStateEvent(`history-open-intent-probe-${delayMs}ms`, true),
          delayMs,
        );
        globalWindow.__bitfunLongSessionVisualProbeTimers?.push(timerId);
      }
    };
    globalWindow.__bitfunLongSessionOpenIntentHandler = handleOpenIntent;
    window.addEventListener('flowchat:history-session-open-intent', handleOpenIntent);
    const handleUserInteraction: EventListener = event => {
      const interactionType =
        event instanceof CustomEvent && typeof event.detail?.type === 'string'
          ? event.detail.type
          : 'unknown';
      recordVisualStateEvent(`user-interaction-${interactionType}`, true);
      for (const delayMs of [16, 50, 100, 250, 500]) {
        window.setTimeout(
          () => recordVisualStateEvent(`user-interaction-${interactionType}-probe-${delayMs}ms`, true),
          delayMs,
        );
      }
    };
    globalWindow.__bitfunLongSessionUserInteractionHandler = handleUserInteraction;
    window.addEventListener('bitfun:e2e-long-session-user-interaction', handleUserInteraction);
    const samples: LongSessionViewportTimelineSample[] = [];
    const mainThreadTasks: LongSessionMainThreadTask[] = [];
    const mutationEvents: LongSessionDomMutationEvent[] = [];
    const visualStateEvents: LongSessionVisualStateEvent[] = [];
    const layoutShiftEvents: LongSessionLayoutShiftEvent[] = [];
    const elementIds = new WeakMap<Element, string>();
    let nextElementId = 1;
    let visualFrame = 0;
    let previousVisualSignature: string | null = null;

    const toNumberOrNull = (value: string | null | undefined): number | null => {
      if (value === null || value === undefined || value === '') {
        return null;
      }
      const numeric = Number(value);
      return Number.isFinite(numeric) ? numeric : null;
    };

    const rectSummary = (rect: DOMRect | null | undefined): LongSessionRectSummary | null => {
      if (!rect) {
        return null;
      }
      const round = (value: number) => Math.round(value * 10) / 10;
      return {
        top: round(rect.top),
        bottom: round(rect.bottom),
        left: round(rect.left),
        right: round(rect.right),
        width: round(rect.width),
        height: round(rect.height),
      };
    };

    const getElementId = (element: Element, prefix: string): string => {
      const existing = elementIds.get(element);
      if (existing) {
        return existing;
      }
      const id = `${prefix}-${nextElementId}`;
      nextElementId += 1;
      elementIds.set(element, id);
      return id;
    };

    const getBackground = (element: Element | null | undefined): string | null => {
      if (!element) {
        return null;
      }
      return window.getComputedStyle(element).backgroundColor || null;
    };

    const effectiveOpacityFor = (element: HTMLElement | null): number => {
      if (!element) {
        return 0;
      }

      let opacity = 1;
      let current: HTMLElement | null = element;
      while (current) {
        const style = window.getComputedStyle(current);
        if (style.display === 'none' || style.visibility === 'hidden') {
          return 0;
        }
        const styleOpacity = Number(style.opacity);
        if (Number.isFinite(styleOpacity)) {
          opacity *= styleOpacity;
        }
        if (opacity <= 0.01) {
          return 0;
        }
        current = current.parentElement;
      }
      return Math.round(opacity * 1000) / 1000;
    };

    const isElementEffectivelyVisible = (element: HTMLElement | null): boolean =>
      effectiveOpacityFor(element) > 0.01;

    const describeNode = (node: Node | null | undefined): string | null => {
      if (!(node instanceof HTMLElement)) {
        return node?.nodeName ?? null;
      }
      const testId = node.getAttribute('data-testid');
      const turnId = node.getAttribute('data-turn-id');
      const itemType = node.getAttribute('data-item-type');
      const id = node.id ? `#${node.id}` : '';
      const className = Array.from(node.classList).slice(0, 4).map(name => `.${name}`).join('');
      const data = [
        testId ? `[data-testid="${testId}"]` : '',
        turnId ? `[data-turn-id="${turnId}"]` : '',
        itemType ? `[data-item-type="${itemType}"]` : '',
      ].join('');
      return `${node.tagName.toLowerCase()}${id}${className}${data}` || node.tagName.toLowerCase();
    };

    const LOADING_SURFACE_SELECTORS = [
      '.modern-flowchat-container__history-overlay',
      '.history-session-placeholder',
      '.bitfun-scene-viewport__lazy-fallback',
      '.bitfun-assistant-scene__loading',
      '[role="status"][aria-busy="true"]',
    ];
    const OBSERVED_MUTATION_SELECTOR = [
      '.modern-flowchat-container__messages',
      '.modern-flowchat-container__history-open-intent-shield',
      '.modern-flowchat-container__history-overlay',
      '.history-session-placeholder',
      '.virtual-message-list',
      '.virtual-message-list__projection-handoff-overlay',
      '.virtual-item-wrapper',
      ...LOADING_SURFACE_SELECTORS,
    ].join(', ');

    const readLoadingSurfaces = (): LongSessionLoadingSurface[] => {
      const seen = new Set<Element>();
      const surfaces: LongSessionLoadingSurface[] = [];
      for (const selector of LOADING_SURFACE_SELECTORS) {
        for (const element of Array.from(document.querySelectorAll<HTMLElement>(selector))) {
          if (seen.has(element)) {
            continue;
          }
          seen.add(element);
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          const visible = (
            rect.width > 0 &&
            rect.height > 0 &&
            style.visibility !== 'hidden' &&
            style.display !== 'none' &&
            isElementEffectivelyVisible(element)
          );
          surfaces.push({
            selector,
            node: describeNode(element),
            visible,
            textLength: element.innerText?.length ?? 0,
            rect: rectSummary(rect),
          });
        }
      }
      return surfaces.slice(0, 24);
    };

    const visibleTextLengthFor = (elements: HTMLElement[]): number => elements.reduce((total, element) => {
      const rect = element.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) {
        return total;
      }
      const style = window.getComputedStyle(element);
      if (style.visibility === 'hidden' || style.display === 'none' || !isElementEffectivelyVisible(element)) {
        return total;
      }
      return total + (element.innerText?.length ?? 0);
    }, 0);

    const readModelRoundMetrics = (element: HTMLElement | null) => {
      const modelRound = element?.querySelector<HTMLElement>('.model-round-item') ??
        element?.closest<HTMLElement>('.model-round-item') ??
        null;
      return {
        modelRoundGroupCount: toNumberOrNull(modelRound?.dataset.modelRoundGroupCount),
        modelRoundRenderedGroupCount: toNumberOrNull(modelRound?.dataset.modelRoundRenderedGroupCount),
        modelRoundVisibleGroupStart: toNumberOrNull(modelRound?.dataset.modelRoundVisibleGroupStart),
        modelRoundVisibleGroupEnd: toNumberOrNull(modelRound?.dataset.modelRoundVisibleGroupEnd),
        modelRoundRenderAll: modelRound?.dataset.modelRoundRenderAll || null,
        modelRoundHasDeferredEarlier: modelRound?.dataset.modelRoundHasDeferredEarlier || null,
        modelRoundHasDeferredLater: modelRound?.dataset.modelRoundHasDeferredLater || null,
      };
    };

    const summarizeVirtualItems = (elements: HTMLElement[]): LongSessionVirtualItemSummary[] =>
      elements.slice(0, 160).map(element => {
        const modelRoundMetrics = readModelRoundMetrics(element);
        const completedToolTransitions = Array.from(
          element.querySelectorAll<HTMLElement>(
            '.flowchat-flow-item--tool-transition.flowchat-flow-item--tool-completed',
          ),
        );
        const completedToolTransitionStyles = completedToolTransitions.map(tool => window.getComputedStyle(tool));
        const uniqueStyleValues = (values: string[]): string[] => Array.from(new Set(values)).slice(0, 8);
        return {
          elementId: getElementId(element, 'virtual-item'),
          virtualIndex: toNumberOrNull(element.dataset.virtualIndex),
          itemType: element.dataset.itemType || null,
          turnId: element.dataset.turnId || null,
          textLength: element.innerText?.length ?? 0,
          ...modelRoundMetrics,
          codeBlockWrapperCount: element.querySelectorAll('.code-block-wrapper').length,
          codeBlockFallbackCount: element.querySelectorAll('.code-block-fallback').length,
          highlightedCodeBlockCount: element.querySelectorAll('.code-block-body pre:not(.code-block-fallback)').length,
          tableCount: element.querySelectorAll('.table-wrapper table').length,
          completedToolTransitionCount: completedToolTransitions.length,
          completedToolTransitionHeight: completedToolTransitions.reduce(
            (total, tool) => total + tool.getBoundingClientRect().height,
            0,
          ),
          completedToolTransitionMaxHeights: uniqueStyleValues(
            completedToolTransitionStyles.map(style => style.maxHeight),
          ),
          completedToolTransitionAnimations: uniqueStyleValues(
            completedToolTransitionStyles.map(style => `${style.animationName}:${style.animationDuration}:${style.animationPlayState}`),
          ),
          completedToolTransitionOpacities: uniqueStyleValues(
            completedToolTransitionStyles.map(style => style.opacity),
          ),
          rect: rectSummary(element.getBoundingClientRect()),
        };
      });

    const getElementBackgroundChain = (elements: Element[]): string[] =>
      elements
        .filter((element): element is HTMLElement => element instanceof HTMLElement)
        .slice(0, 6)
        .map(element => {
          const style = window.getComputedStyle(element);
          return [
            describeNode(element),
            style.backgroundColor || 'none',
            style.opacity || '1',
            style.visibility || 'visible',
            style.display || 'block',
          ].join('@');
        });

    const isLoadingSurfaceElement = (element: HTMLElement): boolean =>
      Boolean(
        element.closest('.modern-flowchat-container__history-overlay') ||
        element.closest('.history-session-placeholder') ||
        element.closest('.bitfun-scene-viewport__lazy-fallback') ||
        element.closest('.bitfun-assistant-scene__loading') ||
        element.closest('[role="status"][aria-busy="true"]'),
      );

    const classifySurfaceElement = (
      element: HTMLElement | null,
      latestTurnId: string,
    ): Pick<LongSessionSurfacePoint, 'category' | 'itemType' | 'turnId' | 'latestTurnHit' | 'effectiveOpacity' | 'textLength' | 'modelRoundGroupCount' | 'modelRoundRenderedGroupCount' | 'modelRoundVisibleGroupStart' | 'modelRoundVisibleGroupEnd' | 'modelRoundRenderAll' | 'modelRoundHasDeferredEarlier' | 'modelRoundHasDeferredLater'> => {
      if (!element) {
        return {
          category: 'missing',
          itemType: null,
          turnId: null,
          latestTurnHit: false,
          effectiveOpacity: 0,
          textLength: 0,
          ...readModelRoundMetrics(null),
        };
      }

      const virtualItem = element.closest<HTMLElement>('.virtual-item-wrapper[data-turn-id]');
      const effectiveOpacity = effectiveOpacityFor(element);
      const virtualItemEffectiveOpacity = virtualItem ? effectiveOpacityFor(virtualItem) : effectiveOpacity;
      const modelRoundMetrics = readModelRoundMetrics(virtualItem ?? element);
      if (isLoadingSurfaceElement(element)) {
        return {
          category: element.closest('.modern-flowchat-container__history-overlay')
            ? 'history-overlay'
            : 'loading-surface',
          itemType: virtualItem?.dataset.itemType || null,
          turnId: virtualItem?.dataset.turnId || null,
          latestTurnHit: virtualItem?.dataset.turnId === latestTurnId,
          effectiveOpacity,
          textLength: virtualItem?.innerText?.length ?? element.innerText?.length ?? 0,
          ...modelRoundMetrics,
        };
      }

      const historyOpenIntentShield = element.closest<HTMLElement>('.modern-flowchat-container__history-open-intent-shield');
      if (historyOpenIntentShield) {
        const shieldBeforeStyle = window.getComputedStyle(historyOpenIntentShield, '::before');
        const hasContinuitySurface =
          shieldBeforeStyle.content !== 'none' &&
          shieldBeforeStyle.display !== 'none' &&
          shieldBeforeStyle.backgroundImage !== 'none' &&
          historyOpenIntentShield.getBoundingClientRect().height > 0;
        return {
          category: hasContinuitySurface
            ? 'history-open-intent-transition-surface'
            : 'history-open-intent-shield',
          itemType: null,
          turnId: null,
          latestTurnHit: false,
          effectiveOpacity,
          textLength: element.innerText?.length ?? 0,
          ...readModelRoundMetrics(null),
        };
      }

      if (virtualItem) {
        return {
          category: virtualItemEffectiveOpacity > 0.01
            ? `virtual-item:${virtualItem.dataset.itemType || 'unknown'}`
            : `transparent-virtual-item:${virtualItem.dataset.itemType || 'unknown'}`,
          itemType: virtualItem.dataset.itemType || null,
          turnId: virtualItem.dataset.turnId || null,
          latestTurnHit: virtualItem.dataset.turnId === latestTurnId,
          effectiveOpacity: virtualItemEffectiveOpacity,
          textLength: virtualItem.innerText?.length ?? 0,
          ...modelRoundMetrics,
        };
      }

      if (element.closest('.bitfun-chat-input-drop-zone')) {
        return {
          category: 'input-overlay',
          itemType: null,
          turnId: null,
          latestTurnHit: false,
          effectiveOpacity,
          textLength: element.innerText?.length ?? 0,
          ...readModelRoundMetrics(null),
        };
      }

      if (element.closest('[data-virtuoso-scroller], .virtual-message-list')) {
        return {
          category: 'list-blank',
          itemType: null,
          turnId: null,
          latestTurnHit: false,
          effectiveOpacity,
          textLength: element.innerText?.length ?? 0,
          ...readModelRoundMetrics(null),
        };
      }

      if (element.closest('.modern-flowchat-container__messages')) {
        return {
          category: 'messages-blank',
          itemType: null,
          turnId: null,
          latestTurnHit: false,
          effectiveOpacity,
          textLength: element.innerText?.length ?? 0,
          ...readModelRoundMetrics(null),
        };
      }

      if (element.closest('.modern-flowchat-container')) {
        return {
          category: 'flowchat-shell',
          itemType: null,
          turnId: null,
          latestTurnHit: false,
          effectiveOpacity,
          textLength: element.innerText?.length ?? 0,
          ...readModelRoundMetrics(null),
        };
      }

      return {
        category: 'other',
        itemType: null,
        turnId: null,
        latestTurnHit: false,
        effectiveOpacity,
        textLength: element.innerText?.length ?? 0,
        ...readModelRoundMetrics(null),
      };
    };

    const readSurfacePoints = (
      messages: HTMLElement | null,
      scroller: HTMLElement | null,
      inputOverlay: HTMLElement | null,
      latestTurnId: string,
    ): LongSessionSurfacePoint[] => {
      const messagesRect = messages?.getBoundingClientRect() ?? null;
      if (!messagesRect || messagesRect.width <= 0 || messagesRect.height <= 0) {
        return [];
      }

      const inputOverlayTop = inputOverlay?.getBoundingClientRect().top ?? messagesRect.bottom;
      const sampleTop = messagesRect.top;
      const sampleBottom = Math.max(
        sampleTop,
        Math.min(messagesRect.bottom, inputOverlayTop),
      );
      const sampleHeight = sampleBottom - sampleTop;
      if (sampleHeight <= 0) {
        return [];
      }

      const pointSpecs = [
        ['top-left', 0.25, 0.16],
        ['top-center', 0.5, 0.16],
        ['top-right', 0.75, 0.16],
        ['middle-left', 0.25, 0.48],
        ['middle-center', 0.5, 0.48],
        ['middle-right', 0.75, 0.48],
        ['bottom-left', 0.25, 0.82],
        ['bottom-center', 0.5, 0.82],
        ['bottom-right', 0.75, 0.82],
      ] as const;

      const findPaintedHandoffElementAtPoint = (x: number, y: number): HTMLElement | null => {
        const handoffs = Array.from(
          messages.querySelectorAll<HTMLElement>('.virtual-message-list__projection-handoff-overlay'),
        );
        for (let handoffIndex = handoffs.length - 1; handoffIndex >= 0; handoffIndex -= 1) {
          const handoff = handoffs[handoffIndex];
          const handoffRect = handoff.getBoundingClientRect();
          const handoffStyle = window.getComputedStyle(handoff);
          const handoffVisible = (
            handoffRect.width > 0 &&
            handoffRect.height > 0 &&
            handoffStyle.visibility !== 'hidden' &&
            handoffStyle.display !== 'none' &&
            isElementEffectivelyVisible(handoff)
          );
          if (
            !handoffVisible ||
            x < handoffRect.left ||
            x > handoffRect.right ||
            y < handoffRect.top ||
            y > handoffRect.bottom
          ) {
            continue;
          }

          const handoffItems = Array.from(
            handoff.querySelectorAll<HTMLElement>('.virtual-item-wrapper[data-turn-id]'),
          );
          for (let itemIndex = handoffItems.length - 1; itemIndex >= 0; itemIndex -= 1) {
            const item = handoffItems[itemIndex];
            const itemRect = item.getBoundingClientRect();
            if (
              itemRect.width > 0 &&
              itemRect.height > 0 &&
              x >= itemRect.left &&
              x <= itemRect.right &&
              y >= itemRect.top &&
              y <= itemRect.bottom &&
              isElementEffectivelyVisible(item)
            ) {
              return item;
            }
          }

          return handoff;
        }

        return null;
      };

      return pointSpecs.map(([label, xRatio, yRatio]) => {
        const x = Math.round(messagesRect.left + messagesRect.width * xRatio);
        const y = Math.round(sampleTop + sampleHeight * yRatio);
        const elements = document.elementsFromPoint(x, y);
        const paintedHandoffElement = findPaintedHandoffElementAtPoint(x, y);
        const topElement =
          paintedHandoffElement ??
          elements.find((element): element is HTMLElement => element instanceof HTMLElement) ??
          null;
        const classified = classifySurfaceElement(topElement, latestTurnId);
        return {
          label,
          x,
          y,
          ...classified,
          topNode: describeNode(topElement),
          topElementId: topElement ? getElementId(topElement, 'surface') : null,
          backgroundChain: getElementBackgroundChain(
            paintedHandoffElement ? [paintedHandoffElement, ...elements] : elements,
          ),
        };
      });
    };

    const surfaceSignatureFor = (surfacePoints: LongSessionSurfacePoint[]): string =>
      surfacePoints
        .map(point => [
          point.label,
          point.category,
          point.topElementId,
          point.itemType,
          point.latestTurnHit ? 'latest' : point.turnId,
          point.effectiveOpacity,
          Math.min(point.textLength, 9999),
          point.modelRoundGroupCount,
          point.modelRoundRenderedGroupCount,
          point.modelRoundVisibleGroupStart,
          point.modelRoundVisibleGroupEnd,
          point.modelRoundRenderAll,
          point.modelRoundHasDeferredEarlier,
          point.modelRoundHasDeferredLater,
          point.backgroundChain[0] ?? '',
        ].join(':'))
        .join('|');

    const readMutationEvent = (reason: string): LongSessionDomMutationEvent => {
      const atMs = performance.now();
      const messages = document.querySelector<HTMLElement>('.modern-flowchat-container__messages');
      const container = document.querySelector<HTMLElement>('.modern-flowchat-container');
      const inputOverlay = document.querySelector<HTMLElement>('.bitfun-chat-input-drop-zone');
      const placeholders = Array.from(
        messages?.querySelectorAll<HTMLElement>('.history-session-placeholder') ?? []
      );
      const overlays = Array.from(
        messages?.querySelectorAll<HTMLElement>('.modern-flowchat-container__history-overlay') ?? []
      );
      const historyInitialPreviews = Array.from(
        messages?.querySelectorAll<HTMLElement>('.virtual-message-list__history-initial-preview') ?? []
      );
      const historyProjectionHandoffs = Array.from(
        messages?.querySelectorAll<HTMLElement>('.virtual-message-list__projection-handoff-overlay') ?? []
      );
      const virtualLists = Array.from(
        messages?.querySelectorAll<HTMLElement>('.virtual-message-list') ?? []
      );
      const scroller = messages?.querySelector<HTMLElement>(
        '[data-virtuoso-scroller="true"], [data-virtuoso-scroller]',
      ) ?? null;
      const virtualItems = Array.from(
        scroller?.querySelectorAll<HTMLElement>('.virtual-item-wrapper[data-turn-id]') ?? []
      );
      const visibleTextLength = visibleTextLengthFor(virtualItems);
      const loadingSurfaces = readLoadingSurfaces();
      const visibleLoadingSurfaceCount = loadingSurfaces.filter(surface => surface.visible).length;
      const surfacePoints = readSurfacePoints(messages ?? null, scroller, inputOverlay, targetTurnId);
      const surfaceSignature = surfaceSignatureFor(surfacePoints);
      const historyInitialPreviewVisible = historyInitialPreviews.some(element => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.visibility !== 'hidden' &&
          style.display !== 'none' &&
          isElementEffectivelyVisible(element)
        );
      });
      const historyProjectionHandoffVisible = historyProjectionHandoffs.some(element => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.visibility !== 'hidden' &&
          style.display !== 'none' &&
          isElementEffectivelyVisible(element)
        );
      });

      return {
        atMs,
        sinceClickMs: atMs - clickTime,
        reason,
        activeSessionId: messages?.dataset.activeSessionId || null,
        historyState: messages?.dataset.historyState || null,
        contextRestoreState: messages?.dataset.contextRestoreState || null,
        isPartial: messages?.dataset.isPartial || null,
        dialogTurnCount: toNumberOrNull(messages?.dataset.dialogTurnCount),
        virtualItemCount: toNumberOrNull(messages?.dataset.virtualItemCount),
        showHistoryPlaceholder: messages?.dataset.showHistoryPlaceholder || null,
        showHistoryTransitionOverlay: messages?.dataset.showHistoryTransitionOverlay || null,
        showHistoryLoadingLayer: messages?.dataset.showHistoryLoadingLayer || null,
        showHistoryOpenIntentOverlay: messages?.dataset.showHistoryOpenIntentOverlay || null,
        hasPendingHistoryCompletion: messages?.dataset.hasPendingHistoryCompletion || null,
        hasDeferredHistoryProjection: messages?.dataset.hasDeferredHistoryProjection || null,
        latestTurnId: messages?.dataset.latestTurnId || null,
        historyInitialContentReady: messages?.dataset.historyInitialContentReady || null,
        pendingHistoryOpenSessionId: messages?.dataset.pendingHistoryOpenSessionId || null,
        placeholderCount: placeholders.length,
        overlayCount: overlays.length,
        historyInitialPreviewCount: historyInitialPreviews.length,
        historyInitialPreviewVisible,
        historyProjectionHandoffCount: historyProjectionHandoffs.length,
        historyProjectionHandoffVisible,
        virtualListCount: virtualLists.length,
        virtualItemDomCount: virtualItems.length,
        visibleLoadingSurfaceCount,
        visibleTextLength,
        loadingSurfaces,
        overlayElementIds: overlays.map(element => getElementId(element, 'overlay')),
        placeholderElementIds: placeholders.map(element => getElementId(element, 'placeholder')),
        virtualListElementIds: virtualLists.map(element => getElementId(element, 'virtual-list')),
        virtualItemElementIds: virtualItems
          .slice(0, 32)
          .map(element => getElementId(element, 'virtual-item')),
        virtualItemSummaries: summarizeVirtualItems(virtualItems),
        surfaceSignature,
        surfacePoints,
        messagesRect: rectSummary(messages?.getBoundingClientRect()),
        scrollerRect: rectSummary(scroller?.getBoundingClientRect()),
        overlayRect: rectSummary(overlays[0]?.getBoundingClientRect()),
        placeholderRect: rectSummary(placeholders[0]?.getBoundingClientRect()),
        messagesBackground: getBackground(messages),
        overlayBackground: getBackground(overlays[0]),
        containerBackground: getBackground(container),
        bodyBackground: getBackground(document.body),
        htmlBackground: getBackground(document.documentElement),
      };
    };

    const readVisualStateEvent = (reason: string): LongSessionVisualStateEvent => {
      const atMs = performance.now();
      const messages = document.querySelector<HTMLElement>('.modern-flowchat-container__messages');
      const container = document.querySelector<HTMLElement>('.modern-flowchat-container');
      const placeholders = Array.from(
        messages?.querySelectorAll<HTMLElement>('.history-session-placeholder') ?? []
      );
      const overlays = Array.from(
        messages?.querySelectorAll<HTMLElement>('.modern-flowchat-container__history-overlay') ?? []
      );
      const historyInitialPreviews = Array.from(
        messages?.querySelectorAll<HTMLElement>('.virtual-message-list__history-initial-preview') ?? []
      );
      const historyProjectionHandoffs = Array.from(
        messages?.querySelectorAll<HTMLElement>('.virtual-message-list__projection-handoff-overlay') ?? []
      );
      const virtualLists = Array.from(
        messages?.querySelectorAll<HTMLElement>('.virtual-message-list') ?? []
      );
      const root = document.querySelector<HTMLElement>(
        '.modern-flowchat-container__messages .virtual-message-list',
      );
      const scroller = root?.querySelector<HTMLElement>(
        '[data-virtuoso-scroller="true"], [data-virtuoso-scroller]',
      ) ?? null;
      const footer = scroller?.querySelector<HTMLElement>('.message-list-footer') ?? null;
      const footerRect = footer?.getBoundingClientRect() ?? null;
      const footerStyle = footer ? window.getComputedStyle(footer) : null;
      const virtualItems = Array.from(
        scroller?.querySelectorAll<HTMLElement>('.virtual-item-wrapper[data-turn-id]') ?? []
      );
      const completedToolTransitions = Array.from(
        scroller?.querySelectorAll<HTMLElement>(
          '.flowchat-flow-item--tool-transition.flowchat-flow-item--tool-completed',
        ) ?? []
      );
      const completedToolTransitionStyles = completedToolTransitions
        .map(tool => window.getComputedStyle(tool));
      const completedToolTransitionHeight = completedToolTransitions.reduce(
        (total, tool) => total + tool.getBoundingClientRect().height,
        0,
      );
      const completedToolTransitionSignature = completedToolTransitionStyles
        .slice(0, 16)
        .map(style => [
          Math.round(Number.parseFloat(style.maxHeight || '0') || 0),
          Math.round(Number.parseFloat(style.opacity || '0') * 100) / 100,
          style.animationName,
          style.animationPlayState,
        ].join(':'))
        .join(',');
      const inputOverlay = document.querySelector<HTMLElement>('.bitfun-chat-input-drop-zone');
      const scrollerRect = scroller?.getBoundingClientRect() ?? null;
      const inputOverlayRect = inputOverlay?.getBoundingClientRect() ?? null;
      const historyPlaceholderRect = placeholders[0]?.getBoundingClientRect() ?? null;
      const historyPlaceholderStyle = placeholders[0]
        ? window.getComputedStyle(placeholders[0])
        : null;
      const historyPlaceholderVisible = Boolean(
        placeholders[0] &&
        historyPlaceholderRect &&
        historyPlaceholderRect.width > 0 &&
        historyPlaceholderRect.height > 0 &&
        historyPlaceholderStyle?.visibility !== 'hidden' &&
        historyPlaceholderStyle?.display !== 'none' &&
        isElementEffectivelyVisible(placeholders[0] ?? null)
      );
      const historyInitialPreviewVisible = historyInitialPreviews.some(element => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.visibility !== 'hidden' &&
          style.display !== 'none' &&
          isElementEffectivelyVisible(element)
        );
      });
      const historyProjectionHandoffVisible = historyProjectionHandoffs.some(element => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.visibility !== 'hidden' &&
          style.display !== 'none' &&
          isElementEffectivelyVisible(element)
        );
      });
      const effectiveScrollerBottom = scrollerRect
        ? Math.min(scrollerRect.bottom, inputOverlayRect?.top ?? scrollerRect.bottom)
        : null;
      const historyPlaceholderCoversMessages = Boolean(
        historyPlaceholderVisible &&
        historyPlaceholderRect &&
        (
          !scrollerRect ||
          (
            historyPlaceholderRect.top <= scrollerRect.top + 4 &&
            historyPlaceholderRect.bottom >= (effectiveScrollerBottom ?? scrollerRect.bottom) - 4 &&
            historyPlaceholderRect.left <= scrollerRect.left + 4 &&
            historyPlaceholderRect.right >= scrollerRect.right - 4
          )
        )
      );
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
      const isVisibleWithinScroller = (rect: DOMRect | null, element?: HTMLElement | null): boolean => Boolean(
        scrollerRect &&
        rect &&
        (!element || isElementEffectivelyVisible(element)) &&
        rect.bottom > scrollerRect.top &&
        rect.top < (effectiveScrollerBottom ?? scrollerRect.bottom)
      );
      const latestVisible = isVisibleWithinScroller(latest?.getBoundingClientRect() ?? null, latest);
      const latestModelRoundVisibleSegments = latestModelRoundSegments
        .filter(element => isVisibleWithinScroller(element.getBoundingClientRect(), element));
      const latestModelRoundTextLength = latestModelRoundVisibleSegments
        .reduce((total, element) => total + (element.innerText?.length ?? 0), 0);
      const latestContentVisuallyVisible =
        (latestVisible || latestModelRoundVisibleSegments.length > 0) &&
        !historyPlaceholderCoversMessages;
      const scrollTop = scroller?.scrollTop ?? null;
      const scrollHeight = scroller?.scrollHeight ?? null;
      const clientHeight = scroller?.clientHeight ?? null;
      const footerHeight = footerRect?.height ?? null;
      const footerStyleHeight = footerStyle?.height ?? null;
      const footerStyleMinHeight = footerStyle?.minHeight ?? null;
      const inputOverlayHeight = inputOverlayRect?.height ?? null;
      const messagesBackground = getBackground(messages);
      const overlayBackground = getBackground(overlays[0]);
      const containerBackground = getBackground(container);
      const bodyBackground = getBackground(document.body);
      const htmlBackground = getBackground(document.documentElement);
      const loadingSurfaces = readLoadingSurfaces();
      const visibleLoadingSurfaceCount = loadingSurfaces.filter(surface => surface.visible).length;
      const surfacePoints = readSurfacePoints(messages ?? null, scroller, inputOverlay, targetTurnId);
      const surfaceSignature = surfaceSignatureFor(surfacePoints);
      const roundMetric = (value: number | null): string =>
        value === null ? 'null' : String(Math.round(value));
      const signature = [
        messages?.dataset.activeSessionId || '',
        messages?.dataset.historyState || '',
        messages?.dataset.contextRestoreState || '',
        messages?.dataset.isPartial || '',
        messages?.dataset.dialogTurnCount || '',
        messages?.dataset.virtualItemCount || '',
        messages?.dataset.showHistoryPlaceholder || '',
        messages?.dataset.showHistoryTransitionOverlay || '',
        messages?.dataset.showHistoryLoadingLayer || '',
        messages?.dataset.showHistoryOpenIntentOverlay || '',
        messages?.dataset.hasPendingHistoryCompletion || '',
        messages?.dataset.hasDeferredHistoryProjection || '',
        messages?.dataset.historyInitialContentReady || '',
        messages?.dataset.pendingHistoryOpenSessionId || '',
        placeholders.length,
        overlays.length,
        historyInitialPreviews.length,
        historyInitialPreviewVisible ? 'history-preview-visible' : 'history-preview-hidden',
        historyProjectionHandoffs.length,
        historyProjectionHandoffVisible ? 'history-projection-handoff-visible' : 'history-projection-handoff-hidden',
        visibleLoadingSurfaceCount,
        loadingSurfaces
          .filter(surface => surface.visible)
          .map(surface => `${surface.selector}:${surface.node}:${surface.textLength}`)
          .join(','),
        virtualLists.length,
        virtualItems.length,
        completedToolTransitions.length,
        roundMetric(completedToolTransitionHeight),
        completedToolTransitionSignature,
        latestContentVisuallyVisible ? 'latest-visible' : 'latest-hidden',
        latestModelRoundTextLength > 0 ? 'latest-text' : 'latest-no-text',
        globalWindow.__bitfunLongSessionOpenIntentSessionId ?? 'no-open-intent-session',
        roundMetric(scrollTop),
        roundMetric(scrollHeight),
        roundMetric(clientHeight),
        roundMetric(footerHeight),
        footerStyleHeight,
        footerStyleMinHeight,
        roundMetric(inputOverlayHeight),
        roundMetric(scrollerRect?.top ?? null),
        roundMetric(scrollerRect?.bottom ?? null),
        roundMetric(inputOverlayRect?.top ?? null),
        surfaceSignature,
        messagesBackground,
        overlayBackground,
        containerBackground,
        bodyBackground,
        htmlBackground,
      ].join('|');

      return {
        atMs,
        sinceClickMs: atMs - clickTime,
        sinceHistoryOpenIntentMs: typeof globalWindow.__bitfunLongSessionOpenIntentAt === 'number'
          ? atMs - globalWindow.__bitfunLongSessionOpenIntentAt
          : null,
        historyOpenIntentSessionId: globalWindow.__bitfunLongSessionOpenIntentSessionId ?? null,
        frame: visualFrame,
        reason,
        signature,
        activeSessionId: messages?.dataset.activeSessionId || null,
        historyState: messages?.dataset.historyState || null,
        contextRestoreState: messages?.dataset.contextRestoreState || null,
        isPartial: messages?.dataset.isPartial || null,
        dialogTurnCount: toNumberOrNull(messages?.dataset.dialogTurnCount),
        virtualItemCount: toNumberOrNull(messages?.dataset.virtualItemCount),
        showHistoryPlaceholder: messages?.dataset.showHistoryPlaceholder || null,
        showHistoryTransitionOverlay: messages?.dataset.showHistoryTransitionOverlay || null,
        showHistoryLoadingLayer: messages?.dataset.showHistoryLoadingLayer || null,
        showHistoryOpenIntentOverlay: messages?.dataset.showHistoryOpenIntentOverlay || null,
        hasPendingHistoryCompletion: messages?.dataset.hasPendingHistoryCompletion || null,
        hasDeferredHistoryProjection: messages?.dataset.hasDeferredHistoryProjection || null,
        latestTurnId: messages?.dataset.latestTurnId || null,
        historyInitialContentReady: messages?.dataset.historyInitialContentReady || null,
        pendingHistoryOpenSessionId: messages?.dataset.pendingHistoryOpenSessionId || null,
        placeholderCount: placeholders.length,
        overlayCount: overlays.length,
        historyInitialPreviewCount: historyInitialPreviews.length,
        historyInitialPreviewVisible,
        historyProjectionHandoffCount: historyProjectionHandoffs.length,
        historyProjectionHandoffVisible,
        virtualListCount: virtualLists.length,
        virtualItemDomCount: virtualItems.length,
        visibleLoadingSurfaceCount,
        virtualListElementIds: virtualLists.map(element => getElementId(element, 'virtual-list')),
        virtualItemElementIds: virtualItems
          .slice(0, 32)
          .map(element => getElementId(element, 'virtual-item')),
        virtualItemSummaries: summarizeVirtualItems(virtualItems),
        completedToolTransitionCount: completedToolTransitions.length,
        completedToolTransitionHeight,
        completedToolTransitionSignature,
        visibleTextLength: visibleTextLengthFor(virtualItems),
        loadingSurfaces,
        latestContentVisuallyVisible,
        latestModelRoundTextLength,
        surfaceSignature,
        surfacePoints,
        scrollerExists: Boolean(scroller),
        scrollTop,
        scrollHeight,
        clientHeight,
        footerHeight,
        footerStyleHeight,
        footerStyleMinHeight,
        inputOverlayHeight,
        messagesRect: rectSummary(messages?.getBoundingClientRect()),
        scrollerRect: rectSummary(scrollerRect),
        overlayRect: rectSummary(overlays[0]?.getBoundingClientRect()),
        placeholderRect: rectSummary(historyPlaceholderRect),
        inputOverlayRect: rectSummary(inputOverlayRect),
        messagesBackground,
        overlayBackground,
        containerBackground,
        bodyBackground,
        htmlBackground,
      };
    };

    const recordVisualStateEvent = (reason: string, force = false) => {
      const event = readVisualStateEvent(reason);
      if (!force && event.signature === previousVisualSignature) {
        return;
      }
      previousVisualSignature = event.signature;
      visualStateEvents.push(event);
      if (visualStateEvents.length > 1200) {
        visualStateEvents.shift();
      }
    };

    function recordMutationEvent(reason: string) {
      mutationEvents.push(readMutationEvent(reason));
      if (mutationEvents.length > 800) {
        mutationEvents.shift();
      }
      recordVisualStateEvent(reason);
    }

    recordMutationEvent('initial');
    const recordAnimationFrame = () => {
      visualFrame += 1;
      recordVisualStateEvent('raf');
      globalWindow.__bitfunLongSessionVisualFrameRequest =
        window.requestAnimationFrame(recordAnimationFrame);
    };
    globalWindow.__bitfunLongSessionVisualFrameRequest =
      window.requestAnimationFrame(recordAnimationFrame);
    globalWindow.__bitfunLongSessionVisualProbeTimers = [
      500,
      1_000,
      2_000,
      3_000,
      5_000,
      7_000,
    ].map(delayMs => window.setTimeout(
      () => recordVisualStateEvent(`probe-${delayMs}ms`, true),
      Math.max(0, clickTime + delayMs - performance.now()),
    ));
    const mutationObserver = new MutationObserver(records => {
      const relevant = records.some(record => {
        if (record.type === 'attributes') {
          const target = record.target as HTMLElement | null;
          return Boolean(
            target?.matches?.(OBSERVED_MUTATION_SELECTOR)
          );
        }
        const nodes = [
          ...Array.from(record.addedNodes),
          ...Array.from(record.removedNodes),
        ];
        return nodes.some(node => {
          if (!(node instanceof HTMLElement)) {
            return false;
          }
          return Boolean(
            node.matches?.(OBSERVED_MUTATION_SELECTOR) ||
            node.querySelector?.(OBSERVED_MUTATION_SELECTOR)
          );
        });
      });
      if (relevant) {
        recordMutationEvent('mutation');
      }
    });
    mutationObserver.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: [
        'class',
        'style',
        'data-active-session-id',
        'data-history-state',
        'data-context-restore-state',
        'data-is-partial',
        'data-dialog-turn-count',
        'data-virtual-item-count',
        'data-show-history-placeholder',
        'data-show-history-transition-overlay',
        'data-show-history-loading-layer',
        'data-show-history-open-intent-overlay',
        'data-has-pending-history-completion',
        'data-has-deferred-history-projection',
        'data-latest-turn-id',
        'data-history-initial-content-ready',
        'data-pending-history-open-session-id',
        'aria-busy',
        'role',
      ],
    });
    globalWindow.__bitfunLongSessionMutationObserver = mutationObserver;
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
    try {
      if (PerformanceObserver.supportedEntryTypes.includes('layout-shift')) {
        const observer = new PerformanceObserver(list => {
          for (const entry of list.getEntries()) {
            if (entry.startTime < clickTime) {
              continue;
            }
            const layoutEntry = entry as PerformanceEntry & {
              value?: number;
              hadRecentInput?: boolean;
              sources?: Array<{
                node?: Node;
                previousRect?: DOMRectReadOnly;
                currentRect?: DOMRectReadOnly;
              }>;
            };
            layoutShiftEvents.push({
              startMs: entry.startTime,
              sinceClickMs: entry.startTime - clickTime,
              value: layoutEntry.value ?? 0,
              hadRecentInput: layoutEntry.hadRecentInput === true,
              sources: (layoutEntry.sources ?? []).slice(0, 6).map(source => ({
                node: describeNode(source.node),
                previousRect: rectSummary(source.previousRect as DOMRect | undefined),
                currentRect: rectSummary(source.currentRect as DOMRect | undefined),
              })),
            });
            if (layoutShiftEvents.length > 160) {
              layoutShiftEvents.shift();
            }
          }
        });
        observer.observe({ type: 'layout-shift', buffered: true });
        globalWindow.__bitfunLongSessionLayoutShiftObserver = observer;
      }
    } catch {
      globalWindow.__bitfunLongSessionLayoutShiftObserver = undefined;
    }
    const readSample = (): LongSessionViewportTimelineSample => {
      const messages = document.querySelector<HTMLElement>('.modern-flowchat-container__messages');
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
        isElementEffectivelyVisible(historyPlaceholder)
      );
      const effectiveScrollerBottom = scrollerRect
        ? Math.min(scrollerRect.bottom, inputOverlayRect?.top ?? scrollerRect.bottom)
        : null;
      const historyPlaceholderCoversMessages = Boolean(
        historyPlaceholderVisible &&
        historyPlaceholderRect &&
        (
          !scrollerRect ||
          (
            historyPlaceholderRect.top <= scrollerRect.top + 4 &&
            historyPlaceholderRect.bottom >= (effectiveScrollerBottom ?? scrollerRect.bottom) - 4 &&
            historyPlaceholderRect.left <= scrollerRect.left + 4 &&
            historyPlaceholderRect.right >= scrollerRect.right - 4
          )
        )
      );
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
      const isVisibleWithinScroller = (rect: DOMRect | null, element?: HTMLElement | null): boolean => Boolean(
        scrollerRect &&
        rect &&
        (!element || isElementEffectivelyVisible(element)) &&
        rect.bottom > scrollerRect.top &&
        rect.top < (effectiveScrollerBottom ?? scrollerRect.bottom)
      );
      const latestModelRoundVisibleSegments = latestModelRoundSegments
        .filter(element => isVisibleWithinScroller(element.getBoundingClientRect(), element));
      const latestModelRoundVisible = latestModelRoundVisibleSegments.length > 0;
      const latestModelRoundTextLength = latestModelRoundVisibleSegments
        .reduce((total, element) => total + (element.innerText?.length ?? 0), 0);
      const latestVisible = isVisibleWithinScroller(latestRect, latest);
      const latestContentVisible = latestVisible || latestModelRoundVisible;
      const latestContentVisuallyVisible =
        latestContentVisible && !historyPlaceholderCoversMessages;
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
      const historyOpenIntentAtMs = globalWindow.__bitfunLongSessionOpenIntentAt ?? null;
      const historyOpenIntentSessionId = globalWindow.__bitfunLongSessionOpenIntentSessionId ?? null;

      return {
        atMs,
        sinceClickMs: atMs - clickTime,
        historyOpenIntentAtMs,
        historyOpenIntentSessionId,
        sinceHistoryOpenIntentMs: historyOpenIntentAtMs === null ? null : atMs - historyOpenIntentAtMs,
        activeSessionId: messages?.dataset.activeSessionId || null,
        pendingHistoryOpenSessionId: messages?.dataset.pendingHistoryOpenSessionId || null,
        hasRoot: Boolean(root),
        hasScroller: Boolean(scroller),
        latestRendered: Boolean(latest),
        latestModelRoundRendered: latestModelRoundSegments.length > 0,
        latestModelRoundVisible,
        latestModelRoundTextLength,
        latestContentVisible,
        historyPlaceholderVisible,
        historyPlaceholderCoversMessages,
        latestContentVisuallyVisible,
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
      if (samples.length > 1200) {
        samples.shift();
      }
    };

    globalWindow.__bitfunLongSessionViewportTimeline = samples;
    globalWindow.__bitfunLongSessionMainThreadTasks = mainThreadTasks;
    globalWindow.__bitfunLongSessionMutationEvents = mutationEvents;
    globalWindow.__bitfunLongSessionVisualStateEvents = visualStateEvents;
    globalWindow.__bitfunLongSessionLayoutShiftEvents = layoutShiftEvents;
    record();
    globalWindow.__bitfunLongSessionViewportTimelineTimer = window.setInterval(record, 50);
  }, expectedLatestTurnId, clickedAtMs, enableRenderProfile);
}

async function stopLongSessionViewportTimelineRecorder(): Promise<LongSessionViewportTimeline> {
  return browser.execute(() => {
    const globalWindow = window as typeof window & {
      __bitfunLongSessionViewportTimeline?: LongSessionViewportTimelineSample[];
      __bitfunLongSessionMainThreadTasks?: LongSessionMainThreadTask[];
      __bitfunLongSessionMutationEvents?: LongSessionDomMutationEvent[];
      __bitfunLongSessionVisualStateEvents?: LongSessionVisualStateEvent[];
      __bitfunLongSessionLayoutShiftEvents?: LongSessionLayoutShiftEvent[];
      __bitfunLongSessionViewportTimelineTimer?: number;
      __bitfunLongSessionVisualFrameRequest?: number;
      __bitfunLongSessionVisualProbeTimers?: number[];
      __bitfunLongSessionLongTaskObserver?: PerformanceObserver;
      __bitfunLongSessionLayoutShiftObserver?: PerformanceObserver;
      __bitfunLongSessionMutationObserver?: MutationObserver;
      __bitfunLongSessionOpenIntentAt?: number;
      __bitfunLongSessionOpenIntentHandler?: EventListener;
      __bitfunLongSessionUserInteractionHandler?: EventListener;
      __BITFUN_RENDER_PROFILE_ENABLED__?: boolean;
    };
    if (globalWindow.__bitfunLongSessionViewportTimelineTimer !== undefined) {
      window.clearInterval(globalWindow.__bitfunLongSessionViewportTimelineTimer);
      globalWindow.__bitfunLongSessionViewportTimelineTimer = undefined;
    }
    if (globalWindow.__bitfunLongSessionVisualFrameRequest !== undefined) {
      window.cancelAnimationFrame(globalWindow.__bitfunLongSessionVisualFrameRequest);
      globalWindow.__bitfunLongSessionVisualFrameRequest = undefined;
    }
    for (const timerId of globalWindow.__bitfunLongSessionVisualProbeTimers ?? []) {
      window.clearTimeout(timerId);
    }
    globalWindow.__bitfunLongSessionVisualProbeTimers = undefined;
    globalWindow.__bitfunLongSessionLongTaskObserver?.disconnect();
    globalWindow.__bitfunLongSessionLongTaskObserver = undefined;
    globalWindow.__bitfunLongSessionLayoutShiftObserver?.disconnect();
    globalWindow.__bitfunLongSessionLayoutShiftObserver = undefined;
    globalWindow.__bitfunLongSessionMutationObserver?.disconnect();
    globalWindow.__bitfunLongSessionMutationObserver = undefined;
    if (globalWindow.__bitfunLongSessionOpenIntentHandler) {
      window.removeEventListener(
        'flowchat:history-session-open-intent',
        globalWindow.__bitfunLongSessionOpenIntentHandler,
      );
      globalWindow.__bitfunLongSessionOpenIntentHandler = undefined;
    }
    if (globalWindow.__bitfunLongSessionUserInteractionHandler) {
      window.removeEventListener(
        'bitfun:e2e-long-session-user-interaction',
        globalWindow.__bitfunLongSessionUserInteractionHandler,
      );
      globalWindow.__bitfunLongSessionUserInteractionHandler = undefined;
    }
    globalWindow.__bitfunLongSessionOpenIntentAt = undefined;
    const samples = globalWindow.__bitfunLongSessionViewportTimeline ?? [];
    const mainThreadTasks = globalWindow.__bitfunLongSessionMainThreadTasks ?? [];
    const mutationEvents = globalWindow.__bitfunLongSessionMutationEvents ?? [];
    const visualStateEvents = globalWindow.__bitfunLongSessionVisualStateEvents ?? [];
    const layoutShiftEvents = globalWindow.__bitfunLongSessionLayoutShiftEvents ?? [];
    globalWindow.__bitfunLongSessionViewportTimeline = undefined;
    globalWindow.__bitfunLongSessionMainThreadTasks = undefined;
    globalWindow.__bitfunLongSessionMutationEvents = undefined;
    globalWindow.__bitfunLongSessionVisualStateEvents = undefined;
    globalWindow.__bitfunLongSessionLayoutShiftEvents = undefined;
    globalWindow.__BITFUN_RENDER_PROFILE_ENABLED__ = false;
    return { samples, mainThreadTasks, mutationEvents, visualStateEvents, layoutShiftEvents };
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
      return viewport.latestContentVisuallyVisible;
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

function requiresLatestModelRoundForFixture(fixtureScenario: string | null): boolean {
  return fixtureScenario !== 'user-only-latest';
}

function isLongSessionViewportUsable(
  viewport: LongSessionViewportState,
  options: LongSessionViewportUsabilityOptions = {},
): boolean {
  const requiresLatestModelRound = options.requireLatestModelRound !== false;
  const coverageRatio = viewport.coverageRatio ?? 0;
  const bottomBlankPx = viewport.bottomBlankPx ?? Number.POSITIVE_INFINITY;
  const largestBlankGapPx = viewport.largestBlankGapPx ?? Number.POSITIVE_INFINITY;
  return (
    viewport.latestContentVisible &&
    viewport.latestContentVisuallyVisible &&
    (
      !requiresLatestModelRound ||
      (
        viewport.latestModelRoundVisible &&
        viewport.latestModelRoundTextLength > 0
      )
    ) &&
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
    viewport.latestContentVisuallyVisible &&
    coverageRatio >= LONG_SESSION_VIEWPORT_MIN_COVERAGE_RATIO &&
    bottomBlankPx <= LONG_SESSION_LATEST_VISIBLE_MAX_BOTTOM_BLANK_PX &&
    largestBlankGapPx <= LONG_SESSION_LATEST_VISIBLE_MAX_BLANK_GAP_PX
  );
}

function isLongSessionLatestTailAnchored(viewport: LongSessionViewportState): boolean {
  if (
    !viewport.latestTurnId ||
    viewport.scrollTop === null ||
    viewport.scrollHeight === null ||
    viewport.clientHeight === null ||
    viewport.scrollerTop === null ||
    viewport.effectiveScrollerBottom === null
  ) {
    return false;
  }

  const distanceFromBottom = viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop;
  if (distanceFromBottom > LONG_SESSION_LATEST_TAIL_BOTTOM_TOLERANCE_PX) {
    return false;
  }

  const latestVisibleItems = viewport.visibleItemSummaries
    .filter(item => item.turnId === viewport.latestTurnId);
  const latestTailItem = latestVisibleItems[latestVisibleItems.length - 1];
  if (!latestTailItem) {
    return false;
  }

  const effectiveBottomInScroller = viewport.effectiveScrollerBottom - viewport.scrollerTop;
  return (
    latestTailItem.bottom <= effectiveBottomInScroller + LONG_SESSION_LATEST_TAIL_BOTTOM_TOLERANCE_PX &&
    latestTailItem.bottom >= effectiveBottomInScroller - LONG_SESSION_LATEST_TAIL_BOTTOM_TOLERANCE_PX
  );
}

function getLongSessionPhysicalDistanceFromBottom(viewport: LongSessionViewportState): number | null {
  if (
    viewport.scrollTop === null ||
    viewport.scrollHeight === null ||
    viewport.clientHeight === null
  ) {
    return null;
  }

  return Math.max(0, viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop);
}

function getLongSessionScrollTransitionDistanceFromBottom(
  transition: LongSessionVisualStateSummary['scrollTransitions'][number],
  side: 'from' | 'to',
): number | null {
  const scrollTop = side === 'from' ? transition.fromScrollTop : transition.toScrollTop;
  const scrollHeight = side === 'from' ? transition.fromScrollHeight : transition.toScrollHeight;
  const clientHeight = side === 'from' ? transition.fromClientHeight : transition.toClientHeight;
  if (scrollTop === null || scrollHeight === null || clientHeight === null) {
    return null;
  }
  return Math.max(0, scrollHeight - clientHeight - scrollTop);
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
  targetSessionId: string,
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
  const firstLatestContentVisuallyVisible = samples.find(sample =>
    sample.latestContentVisuallyVisible
  );
  const firstLatestTextVisible = samples.find(sample =>
    sample.latestContentVisuallyVisible &&
    sample.latestModelRoundVisible &&
    sample.latestModelRoundTextLength > 0
  );
  const samplesAfterOpenIntent = samples.filter(sample =>
    typeof sample.sinceHistoryOpenIntentMs === 'number' &&
    sample.sinceHistoryOpenIntentMs >= 0
  );
  const latestVisibleTextlessSamples = samples.filter(sample =>
    sample.latestVisible &&
      sample.latestModelRoundVisible &&
      sample.latestModelRoundTextLength === 0
  );
  const firstLatestVisibleTextless = latestVisibleTextlessSamples[0];
  const latestContentVisibleTextlessSamples = samples.filter(sample =>
    sample.latestContentVisuallyVisible &&
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
  const preLatestTextVisibleUncoveredAfterIntentSamples = samplesAfterOpenIntent.filter(sample =>
    (firstLatestTextVisible
      ? sample.sinceClickMs < firstLatestTextVisible.sinceClickMs
      : true) &&
    sample.activeSessionId === targetSessionId &&
    !sample.historyPlaceholderCoversMessages &&
    !sample.latestContentVisuallyVisible &&
    (
      sample.visibleItemCount > 0 ||
      sample.renderedItemCount > 0 ||
      sample.visibleTextLength > 0
    )
  );
  const postLatestTextVisibleBlankSamples = firstLatestTextVisible
    ? samples.filter(sample =>
      sample.sinceClickMs > firstLatestTextVisible.sinceClickMs &&
      sample.hasRoot &&
      sample.hasScroller &&
      sample.visibleItemCount === 0
    )
    : [];
  const postLatestTextVisibleCoveredSamples = firstLatestTextVisible
    ? samples.filter(sample =>
      sample.sinceClickMs > firstLatestTextVisible.sinceClickMs &&
      sample.historyPlaceholderCoversMessages
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
      !sample.latestContentVisuallyVisible
    )
    : [];

  return {
    firstScrollerAtMs: firstScroller?.sinceClickMs ?? null,
    firstScrollerBlankAtMs: firstScrollerBlank?.sinceClickMs ?? null,
    firstVisibleItemAtMs: firstVisibleItem?.sinceClickMs ?? null,
    firstHistoryPlaceholderAtMs: firstHistoryPlaceholder?.sinceClickMs ?? null,
    firstLatestVisibleAtMs: firstLatestVisible?.sinceClickMs ?? null,
    firstLatestContentVisibleAtMs: firstLatestContentVisible?.sinceClickMs ?? null,
    firstLatestContentVisuallyVisibleAtMs: firstLatestContentVisuallyVisible?.sinceClickMs ?? null,
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
    latestTextDelayAfterContentVisuallyVisibleMs: (
      firstLatestContentVisuallyVisible && firstLatestTextVisible
        ? firstLatestTextVisible.sinceClickMs - firstLatestContentVisuallyVisible.sinceClickMs
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
    preLatestTextVisibleUncoveredAfterIntentSampleCount: preLatestTextVisibleUncoveredAfterIntentSamples.length,
    firstPreLatestTextVisibleUncoveredAfterIntentAtMs:
      preLatestTextVisibleUncoveredAfterIntentSamples[0]?.sinceHistoryOpenIntentMs ??
      preLatestTextVisibleUncoveredAfterIntentSamples[0]?.sinceClickMs ??
      null,
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
    postLatestTextVisibleCoveredSampleCount: postLatestTextVisibleCoveredSamples.length,
    maxPostLatestTextVisibleBlankGapPx: postLatestTextVisibleBlankGaps.length > 0
      ? Math.max(...postLatestTextVisibleBlankGaps)
      : null,
    maxPostLatestTextVisibleBottomBlankPx: postLatestTextVisibleBottomBlanks.length > 0
      ? Math.max(...postLatestTextVisibleBottomBlanks)
      : null,
    postLatestTextVisibleLatestContentMissingSampleCount: postLatestTextVisibleLatestContentMissingSamples.length,
  };
}

function summarizeLongSessionVisualStateEvents(
  visualStateEvents: LongSessionVisualStateEvent[],
  mutationEvents: LongSessionDomMutationEvent[],
  layoutShiftEvents: LongSessionLayoutShiftEvent[],
  viewportTimelineSummary: LongSessionViewportTimelineSummary,
  sessionId: string,
): LongSessionVisualStateSummary {
  const firstLatestTextVisibleAtMs = viewportTimelineSummary.firstLatestTextVisibleAtMs;
  const firstVisibleItemAtMs = viewportTimelineSummary.firstVisibleItemAtMs;
  const isForcedVisualProbeEvent = (event: LongSessionVisualStateEvent): boolean =>
    event.reason.startsWith('probe-');
  const isUserInteractionVisualEvent = (event: LongSessionVisualStateEvent): boolean =>
    event.reason.startsWith('user-interaction-');
  const postLatestTextEvents = firstLatestTextVisibleAtMs === null
    ? []
    : visualStateEvents.filter(event => event.sinceClickMs > firstLatestTextVisibleAtMs);
  const postLatestTextChangeEvents = postLatestTextEvents.filter(event => !isForcedVisualProbeEvent(event));
  const firstUserInteractionEvent = visualStateEvents.find(event =>
    event.reason.startsWith('user-interaction-') &&
    !event.reason.includes('-probe-')
  ) ?? null;
  const firstUserInteractionAtMs = firstUserInteractionEvent?.sinceClickMs ?? null;
  const postUserInteractionEvents = firstUserInteractionAtMs === null
    ? []
    : visualStateEvents.filter(event =>
      event.sinceClickMs > firstUserInteractionAtMs &&
      !isForcedVisualProbeEvent(event) &&
      !isUserInteractionVisualEvent(event)
    );
  const postFirstVisibleItemEvents = firstVisibleItemAtMs === null
    ? []
    : visualStateEvents.filter(event => event.sinceClickMs > firstVisibleItemAtMs);
  const postFirstVisibleItemChangeEvents = postFirstVisibleItemEvents.filter(event => !isForcedVisualProbeEvent(event));
  const backgroundKey = (event: LongSessionVisualStateEvent): string => [
    event.htmlBackground,
    event.bodyBackground,
    event.containerBackground,
    event.messagesBackground,
    event.overlayBackground,
  ].join('|');
  const arrayKey = (values: string[]): string => values.join('|');
  const surfaceCategoryKey = (event: LongSessionVisualStateEvent): string =>
    event.surfacePoints
      .map(point => [
        point.label,
        point.category,
        point.itemType,
        point.latestTurnHit ? 'latest' : point.turnId,
        point.effectiveOpacity,
        Math.min(point.textLength, 9999),
        point.modelRoundGroupCount,
        point.modelRoundRenderedGroupCount,
        point.modelRoundVisibleGroupStart,
        point.modelRoundVisibleGroupEnd,
        point.modelRoundRenderAll,
        point.modelRoundHasDeferredEarlier,
        point.modelRoundHasDeferredLater,
        point.backgroundChain[0] ?? '',
      ].join(':'))
      .join('|');
  const isLoadingVisualEvent = (event: LongSessionVisualStateEvent): boolean =>
    event.showHistoryLoadingLayer === 'true' ||
    event.overlayCount > 0 ||
    event.placeholderCount > 0 ||
    event.visibleLoadingSurfaceCount > 0;
  const isLoadingSurfacePointEvent = (event: LongSessionVisualStateEvent): boolean =>
    event.surfacePoints.some(point =>
      point.category === 'history-overlay' ||
      point.category === 'loading-surface'
    );
  const isHistoryOpenIntentSurfacePoint = (category: string): boolean =>
    category === 'history-open-intent-shield' ||
    category === 'history-open-intent-transition-surface';

  const isBlankSurfacePointEvent = (event: LongSessionVisualStateEvent): boolean => {
    if (event.surfacePoints.length === 0) {
      return false;
    }

    const contentPointCount = event.surfacePoints.filter(point =>
      point.category.startsWith('virtual-item:') &&
      point.textLength > 0
    ).length;
    const blankPointCount = event.surfacePoints.filter(point =>
      point.category === 'list-blank' ||
      point.category === 'messages-blank' ||
      point.category === 'flowchat-shell' ||
      point.category === 'history-open-intent-shield' ||
      point.category === 'missing'
    ).length;
    return contentPointCount === 0 && blankPointCount >= Math.ceil(event.surfacePoints.length * 0.6);
  };
  const isOpenIntentBlankSurfaceEvent = (event: LongSessionVisualStateEvent): boolean =>
    event.sinceHistoryOpenIntentMs !== null &&
    event.historyOpenIntentSessionId === sessionId &&
    isBlankSurfacePointEvent(event) &&
    event.surfacePoints.some(point => point.category === 'history-open-intent-shield');
  const isTransparentSurfacePointEvent = (event: LongSessionVisualStateEvent): boolean =>
    event.surfacePoints.some(point =>
      point.category.startsWith('transparent-virtual-item:')
    );
  const isVisibleNonTargetContentEvent = (event: LongSessionVisualStateEvent): boolean =>
    event.historyOpenIntentSessionId === sessionId &&
    event.activeSessionId !== null &&
    event.activeSessionId !== sessionId &&
    event.pendingHistoryOpenSessionId !== sessionId &&
    (
      event.surfacePoints.some(point =>
        point.category.startsWith('virtual-item:') &&
        point.textLength > 0 &&
        point.effectiveOpacity > 0.01
      ) ||
      (
        !event.surfacePoints.some(point => isHistoryOpenIntentSurfacePoint(point.category)) &&
        event.virtualItemDomCount > 0 &&
        event.visibleTextLength > 0
      )
    );
  const openIntentBlankSurfaceEvents = visualStateEvents.filter(isOpenIntentBlankSurfaceEvent);
  const openIntentBlankSurfaceHolds = openIntentBlankSurfaceEvents
    .map(event => {
      const index = visualStateEvents.indexOf(event);
      const nextEvent = visualStateEvents
        .slice(index + 1)
        .find(candidate => candidate.sinceHistoryOpenIntentMs !== null);
      const untilHistoryOpenIntentMs = nextEvent?.sinceHistoryOpenIntentMs ?? event.sinceHistoryOpenIntentMs;
      const holdAfterGraceMs = Math.max(
        0,
        untilHistoryOpenIntentMs - Math.max(event.sinceHistoryOpenIntentMs ?? 0, 50),
      );
      if (holdAfterGraceMs <= 0) {
        return null;
      }

      return {
        event,
        untilHistoryOpenIntentMs,
        holdAfterGraceMs,
      };
    })
    .filter((entry): entry is {
      event: LongSessionVisualStateEvent;
      untilHistoryOpenIntentMs: number;
      holdAfterGraceMs: number;
    } => entry !== null);
  const postOpenIntentNonTargetContentEvents = visualStateEvents.filter(event =>
    event.sinceHistoryOpenIntentMs !== null &&
    event.sinceHistoryOpenIntentMs > 100 &&
    isVisibleNonTargetContentEvent(event)
  );
  const postOpenIntentNonTargetContentHolds = visualStateEvents
    .map((event, index) => {
      if (
        event.sinceHistoryOpenIntentMs === null ||
        event.sinceHistoryOpenIntentMs < 100 ||
        !isVisibleNonTargetContentEvent(event)
      ) {
        return null;
      }

      const nextEvent = visualStateEvents
        .slice(index + 1)
        .find(candidate => candidate.sinceHistoryOpenIntentMs !== null);
      const untilHistoryOpenIntentMs = nextEvent?.sinceHistoryOpenIntentMs ?? event.sinceHistoryOpenIntentMs;
      const holdAfterGraceMs = Math.max(
        0,
        untilHistoryOpenIntentMs - Math.max(event.sinceHistoryOpenIntentMs, 100),
      );
      if (holdAfterGraceMs <= 0) {
        return null;
      }

      return {
        event,
        untilHistoryOpenIntentMs,
        holdAfterGraceMs,
      };
    })
    .filter((entry): entry is {
      event: LongSessionVisualStateEvent;
      untilHistoryOpenIntentMs: number;
      holdAfterGraceMs: number;
    } => entry !== null);
  const isHistoryInitialPreviewVisibleAt = (sinceClickMs: number): boolean => {
    let nearest: LongSessionVisualStateEvent | null = null;
    for (const event of visualStateEvents) {
      if (event.sinceClickMs > sinceClickMs) {
        break;
      }
      nearest = event;
    }
    return nearest?.historyInitialPreviewVisible === true;
  };
  const isHistoryProjectionHandoffVisibleAt = (sinceClickMs: number): boolean => {
    let nearest: LongSessionVisualStateEvent | null = null;
    for (const event of visualStateEvents) {
      if (event.sinceClickMs > sinceClickMs) {
        break;
      }
      nearest = event;
    }
    return nearest?.historyProjectionHandoffVisible === true;
  };
  const loadingEvents = visualStateEvents.filter(isLoadingVisualEvent);

  let loadingLayerToggleCount = 0;
  let overlayCountToggleCount = 0;
  let placeholderCountToggleCount = 0;
  let backgroundChangeCount = 0;
  let scrollerScrollJumpCount = 0;
  let scrollerSizeChangeCount = 0;
  let postLatestTextVisibleBackgroundChangeCount = 0;
  let postLatestTextVisibleScrollJumpCount = 0;
  let postLatestTextVisibleVirtualItemElementChangeCount = 0;
  let postLatestTextVisibleSurfaceChangeCount = 0;
  let postUserInteractionScrollJumpCount = 0;
  let postUserInteractionScrollerCollapseCount = 0;
  let hasSeenTargetSession = false;
  let historyInitialPreviewActivationAfterActiveSessionCount = 0;
  const loadingTransitions: LongSessionVisualStateSummary['loadingTransitions'] = [];
  const backgroundTransitions: LongSessionVisualStateSummary['backgroundTransitions'] = [];
  const historyInitialPreviewTransitions: LongSessionVisualStateSummary['historyInitialPreviewTransitions'] = [];
  const scrollTransitions: LongSessionVisualStateSummary['scrollTransitions'] = [];
  const postUserInteractionScrollTransitions: LongSessionVisualStateSummary['postUserInteractionScrollTransitions'] = [];
  const surfaceTransitions: LongSessionVisualStateSummary['surfaceTransitions'] = [];

  for (let index = 1; index < visualStateEvents.length; index += 1) {
    const previous = visualStateEvents[index - 1];
    const current = visualStateEvents[index];
    if (previous.activeSessionId === sessionId) {
      hasSeenTargetSession = true;
    }
    if (
      previous.historyInitialPreviewVisible !== current.historyInitialPreviewVisible &&
      current.activeSessionId === sessionId
    ) {
      historyInitialPreviewTransitions.push({
        sinceClickMs: current.sinceClickMs,
        reason: current.reason,
        from: previous.historyInitialPreviewVisible,
        to: current.historyInitialPreviewVisible,
      });
      if (
        hasSeenTargetSession &&
        previous.historyInitialPreviewVisible === false &&
        current.historyInitialPreviewVisible === true
      ) {
        historyInitialPreviewActivationAfterActiveSessionCount += 1;
      }
    }
    if (current.activeSessionId === sessionId) {
      hasSeenTargetSession = true;
    }
    const historyLoadingLayerChanged =
      previous.showHistoryLoadingLayer !== null &&
      current.showHistoryLoadingLayer !== null &&
      previous.showHistoryLoadingLayer !== current.showHistoryLoadingLayer;
    const loadingChanged =
      historyLoadingLayerChanged ||
      previous.overlayCount !== current.overlayCount ||
      previous.placeholderCount !== current.placeholderCount;
    if (historyLoadingLayerChanged) {
      loadingLayerToggleCount += 1;
    }
    if (previous.overlayCount !== current.overlayCount) {
      overlayCountToggleCount += 1;
    }
    if (previous.placeholderCount !== current.placeholderCount) {
      placeholderCountToggleCount += 1;
    }
    if (loadingChanged) {
      loadingTransitions.push({
        sinceClickMs: current.sinceClickMs,
        reason: current.reason,
        showHistoryLoadingLayer: current.showHistoryLoadingLayer,
        overlayCount: current.overlayCount,
        placeholderCount: current.placeholderCount,
        visibleLoadingSurfaceCount: current.visibleLoadingSurfaceCount,
        loadingSurfaces: current.loadingSurfaces.filter(surface => surface.visible).slice(0, 8),
        virtualItemDomCount: current.virtualItemDomCount,
        latestContentVisuallyVisible: current.latestContentVisuallyVisible,
      });
    }

    const previousBackground = backgroundKey(previous);
    const currentBackground = backgroundKey(current);
    if (previousBackground !== currentBackground) {
      backgroundChangeCount += 1;
      if (
        firstLatestTextVisibleAtMs !== null &&
        current.sinceClickMs > firstLatestTextVisibleAtMs
      ) {
        postLatestTextVisibleBackgroundChangeCount += 1;
      }
      backgroundTransitions.push({
        sinceClickMs: current.sinceClickMs,
        reason: current.reason,
        from: previousBackground,
        to: currentBackground,
      });
    }

    const previousSurface = surfaceCategoryKey(previous);
    const currentSurface = surfaceCategoryKey(current);
    if (previousSurface !== currentSurface) {
      if (
        firstLatestTextVisibleAtMs !== null &&
        current.sinceClickMs > firstLatestTextVisibleAtMs &&
        !previous.historyInitialPreviewVisible &&
        !current.historyInitialPreviewVisible &&
        !previous.historyProjectionHandoffVisible &&
        !current.historyProjectionHandoffVisible &&
        !isUserInteractionVisualEvent(current)
      ) {
        postLatestTextVisibleSurfaceChangeCount += 1;
      }
      surfaceTransitions.push({
        sinceClickMs: current.sinceClickMs,
        reason: current.reason,
        from: previousSurface,
        to: currentSurface,
        surfacePoints: current.surfacePoints,
      });
    }

    const scrollTopDelta = Math.abs((current.scrollTop ?? 0) - (previous.scrollTop ?? 0));
    const scrollHeightDelta = Math.abs((current.scrollHeight ?? 0) - (previous.scrollHeight ?? 0));
    const clientHeightDelta = Math.abs((current.clientHeight ?? 0) - (previous.clientHeight ?? 0));
    const previousDistanceFromBottom =
      previous.scrollTop === null || previous.scrollHeight === null || previous.clientHeight === null
        ? null
        : previous.scrollHeight - previous.clientHeight - previous.scrollTop;
    const currentDistanceFromBottom =
      current.scrollTop === null || current.scrollHeight === null || current.clientHeight === null
        ? null
        : current.scrollHeight - current.clientHeight - current.scrollTop;
    const distanceFromBottomDelta =
      previousDistanceFromBottom === null || currentDistanceFromBottom === null
        ? scrollTopDelta
        : Math.abs(currentDistanceFromBottom - previousDistanceFromBottom);
    const hasScrollJump = scrollTopDelta > 32;
    const hasVisualScrollJump = hasScrollJump && distanceFromBottomDelta > 32;
    const hasSizeJump = scrollHeightDelta > 16 || clientHeightDelta > 16;
    const isPostUserInteractionNonUserEvent =
      firstUserInteractionAtMs !== null &&
      current.sinceClickMs > firstUserInteractionAtMs &&
      !isUserInteractionVisualEvent(current) &&
      !isForcedVisualProbeEvent(current);
    if (hasScrollJump) {
      scrollerScrollJumpCount += 1;
      if (
        firstLatestTextVisibleAtMs !== null &&
        current.sinceClickMs > firstLatestTextVisibleAtMs &&
        hasVisualScrollJump &&
        !previous.historyInitialPreviewVisible &&
        !current.historyInitialPreviewVisible &&
        !previous.historyProjectionHandoffVisible &&
        !current.historyProjectionHandoffVisible &&
        !isUserInteractionVisualEvent(current)
      ) {
        postLatestTextVisibleScrollJumpCount += 1;
      }
    }
    if (isPostUserInteractionNonUserEvent && hasVisualScrollJump) {
      postUserInteractionScrollJumpCount += 1;
    }
    if (
      isPostUserInteractionNonUserEvent &&
      previous.scrollHeight !== null &&
      current.scrollHeight !== null &&
      previous.scrollHeight - current.scrollHeight > Math.max(120, (previous.clientHeight ?? 0) * 0.5)
    ) {
      postUserInteractionScrollerCollapseCount += 1;
    }
    if (hasSizeJump) {
      scrollerSizeChangeCount += 1;
    }
    if (hasScrollJump || hasSizeJump) {
      scrollTransitions.push({
        sinceClickMs: current.sinceClickMs,
        reason: current.reason,
        fromScrollTop: previous.scrollTop,
        toScrollTop: current.scrollTop,
        fromScrollHeight: previous.scrollHeight,
        toScrollHeight: current.scrollHeight,
        fromClientHeight: previous.clientHeight,
        toClientHeight: current.clientHeight,
        fromFooterHeight: previous.footerHeight,
        toFooterHeight: current.footerHeight,
        fromFooterStyleHeight: previous.footerStyleHeight,
        toFooterStyleHeight: current.footerStyleHeight,
      });
      if (isPostUserInteractionNonUserEvent) {
        postUserInteractionScrollTransitions.push({
          sinceClickMs: current.sinceClickMs,
          reason: current.reason,
          fromScrollTop: previous.scrollTop,
          toScrollTop: current.scrollTop,
          fromScrollHeight: previous.scrollHeight,
          toScrollHeight: current.scrollHeight,
          fromClientHeight: previous.clientHeight,
          toClientHeight: current.clientHeight,
          fromFooterHeight: previous.footerHeight,
          toFooterHeight: current.footerHeight,
          fromFooterStyleHeight: previous.footerStyleHeight,
          toFooterStyleHeight: current.footerStyleHeight,
        });
      }
    }
  }

  let overlayElementIdChangeCount = 0;
  let placeholderElementIdChangeCount = 0;
  let virtualListElementIdChangeCount = 0;
  let virtualItemElementIdChangeCount = 0;
  const overlayElementTransitions: LongSessionVisualStateSummary['overlayElementTransitions'] = [];
  const virtualItemElementTransitions: LongSessionVisualStateSummary['virtualItemElementTransitions'] = [];
  for (let index = 1; index < mutationEvents.length; index += 1) {
    const previous = mutationEvents[index - 1];
    const current = mutationEvents[index];
    const previousOverlayKey = arrayKey(previous.overlayElementIds);
    const currentOverlayKey = arrayKey(current.overlayElementIds);
    if (previousOverlayKey !== currentOverlayKey) {
      overlayElementIdChangeCount += 1;
      overlayElementTransitions.push({
        sinceClickMs: current.sinceClickMs,
        reason: current.reason,
        from: previous.overlayElementIds,
        to: current.overlayElementIds,
      });
    }
    if (arrayKey(previous.placeholderElementIds) !== arrayKey(current.placeholderElementIds)) {
      placeholderElementIdChangeCount += 1;
    }
    if (arrayKey(previous.virtualListElementIds) !== arrayKey(current.virtualListElementIds)) {
      virtualListElementIdChangeCount += 1;
    }
    const previousVirtualItemKey = arrayKey(previous.virtualItemElementIds);
    const currentVirtualItemKey = arrayKey(current.virtualItemElementIds);
    if (previousVirtualItemKey !== currentVirtualItemKey) {
      virtualItemElementIdChangeCount += 1;
      if (
        firstLatestTextVisibleAtMs !== null &&
        current.sinceClickMs > firstLatestTextVisibleAtMs &&
        !previous.historyInitialPreviewVisible &&
        !current.historyInitialPreviewVisible &&
        !previous.historyProjectionHandoffVisible &&
        !current.historyProjectionHandoffVisible
      ) {
        postLatestTextVisibleVirtualItemElementChangeCount += 1;
      }
      virtualItemElementTransitions.push({
        sinceClickMs: current.sinceClickMs,
        reason: current.reason,
        from: previous.virtualItemElementIds,
        to: current.virtualItemElementIds,
      });
    }
  }
  const layoutShiftScore = layoutShiftEvents
    .reduce((total, event) => total + event.value, 0);
  const postLatestTextVisibleLayoutShiftEvents = firstLatestTextVisibleAtMs === null
    ? []
    : layoutShiftEvents.filter(event => (
      event.sinceClickMs > firstLatestTextVisibleAtMs &&
      !isHistoryInitialPreviewVisibleAt(event.sinceClickMs) &&
      !isHistoryProjectionHandoffVisibleAt(event.sinceClickMs)
    ));

  return {
    visualStateEventCount: visualStateEvents.length,
    mutationEventCount: mutationEvents.length,
    firstVisualStateAtMs: visualStateEvents[0]?.sinceClickMs ?? null,
    firstLoadingLayerAtMs: loadingEvents[0]?.sinceClickMs ?? null,
    lastLoadingLayerAtMs: loadingEvents[loadingEvents.length - 1]?.sinceClickMs ?? null,
    historyInitialPreviewVisibleAtEnd:
      visualStateEvents.findLast(event => event.activeSessionId === sessionId)?.historyInitialPreviewVisible === true,
    historyProjectionHandoffVisibleAtEnd:
      visualStateEvents.findLast(event => event.activeSessionId === sessionId)?.historyProjectionHandoffVisible === true,
    historyInitialPreviewActivationAfterActiveSessionCount,
    loadingLayerToggleCount,
    overlayCountToggleCount,
    placeholderCountToggleCount,
    overlayElementIdChangeCount,
    placeholderElementIdChangeCount,
    virtualListElementIdChangeCount,
    virtualItemElementIdChangeCount,
    backgroundChangeCount,
    scrollerScrollJumpCount,
    scrollerSizeChangeCount,
    layoutShiftCount: layoutShiftEvents.length,
    layoutShiftScore,
    postLatestTextVisibleVisualChangeCount: postLatestTextChangeEvents.length,
    postLatestTextVisibleLoadingEventCount: postLatestTextEvents.filter(isLoadingVisualEvent).length,
    postFirstVisibleItemVisualChangeCount: postFirstVisibleItemChangeEvents.length,
    postFirstVisibleItemLoadingEventCount: postFirstVisibleItemEvents.filter(isLoadingVisualEvent).length,
    postLatestTextVisibleBackgroundChangeCount,
    postLatestTextVisibleScrollJumpCount,
    postLatestTextVisibleVirtualItemElementChangeCount,
    postLatestTextVisibleSurfaceChangeCount,
    postLatestTextVisibleLoadingSurfacePointEventCount: postLatestTextEvents
      .filter(isLoadingSurfacePointEvent).length,
    postLatestTextVisibleBlankSurfacePointEventCount: postLatestTextEvents
      .filter(isBlankSurfacePointEvent).length,
    postLatestTextVisibleTransparentSurfacePointEventCount: postLatestTextEvents
      .filter(isTransparentSurfacePointEvent).length,
    postLatestTextVisibleLayoutShiftCount: postLatestTextVisibleLayoutShiftEvents.length,
    postLatestTextVisibleLayoutShiftScore: postLatestTextVisibleLayoutShiftEvents
      .reduce((total, event) => total + event.value, 0),
    firstUserInteractionAtMs,
    postUserInteractionScrollJumpCount,
    postUserInteractionScrollerCollapseCount,
    postUserInteractionBlankSurfacePointEventCount: postUserInteractionEvents
      .filter(isBlankSurfacePointEvent).length,
    openIntentBlankSurfacePointEventCount: openIntentBlankSurfaceEvents.length,
    openIntentBlankSurfaceHoldCount: openIntentBlankSurfaceHolds.length,
    maxOpenIntentBlankSurfaceHoldMs: openIntentBlankSurfaceHolds.length > 0
      ? Math.max(...openIntentBlankSurfaceHolds.map(entry => entry.holdAfterGraceMs))
      : null,
    postOpenIntentNonTargetContentEventCount: postOpenIntentNonTargetContentEvents.length,
    postOpenIntentNonTargetContentHoldCount: postOpenIntentNonTargetContentHolds.length,
    maxPostOpenIntentNonTargetContentHoldMs: postOpenIntentNonTargetContentHolds.length > 0
      ? Math.max(...postOpenIntentNonTargetContentHolds.map(entry => entry.holdAfterGraceMs))
      : null,
    lastPostOpenIntentNonTargetContentAtMs:
      postOpenIntentNonTargetContentEvents.at(-1)?.sinceHistoryOpenIntentMs ?? null,
    loadingTransitions: loadingTransitions.slice(0, 40),
    backgroundTransitions: backgroundTransitions.slice(0, 40),
    historyInitialPreviewTransitions: historyInitialPreviewTransitions.slice(0, 40),
    overlayElementTransitions: overlayElementTransitions.slice(0, 40),
    virtualItemElementTransitions: virtualItemElementTransitions.slice(0, 40),
    surfaceTransitions: surfaceTransitions.slice(0, 80),
    postOpenIntentNonTargetContentEvents: postOpenIntentNonTargetContentEvents
      .slice(0, 40)
      .map(event => ({
        sinceClickMs: event.sinceClickMs,
        historyOpenIntentSessionId: event.historyOpenIntentSessionId,
        sinceHistoryOpenIntentMs: event.sinceHistoryOpenIntentMs,
        reason: event.reason,
        activeSessionId: event.activeSessionId,
        pendingHistoryOpenSessionId: event.pendingHistoryOpenSessionId,
        virtualItemDomCount: event.virtualItemDomCount,
        visibleTextLength: event.visibleTextLength,
        surfacePoints: event.surfacePoints,
      })),
    postOpenIntentNonTargetContentHolds: postOpenIntentNonTargetContentHolds
      .slice(0, 40)
      .map(({ event, untilHistoryOpenIntentMs, holdAfterGraceMs }) => ({
        sinceClickMs: event.sinceClickMs,
        historyOpenIntentSessionId: event.historyOpenIntentSessionId,
        sinceHistoryOpenIntentMs: event.sinceHistoryOpenIntentMs,
        untilHistoryOpenIntentMs,
        holdAfterGraceMs,
        reason: event.reason,
        activeSessionId: event.activeSessionId,
        virtualItemDomCount: event.virtualItemDomCount,
        visibleTextLength: event.visibleTextLength,
      })),
    scrollTransitions: scrollTransitions.slice(0, 40),
    postUserInteractionScrollTransitions: postUserInteractionScrollTransitions.slice(0, 20),
    layoutShiftEvents: layoutShiftEvents.slice(0, 40),
  };
}

async function waitForLatestLongSessionViewportUsable(
  timeoutMs: number,
  expectedLatestTurnId?: string | null,
  options: LongSessionViewportUsabilityOptions = {},
): Promise<{
  usableAtMs: number;
  viewport: LongSessionViewportState;
}> {
  let viewport = await readLongSessionViewportState(expectedLatestTurnId);
  try {
    await browser.waitUntil(async () => {
      viewport = await readLongSessionViewportState(expectedLatestTurnId);
      return isLongSessionViewportUsable(viewport, options);
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
  postVisibleInteraction: LongSessionPostVisibleInteraction | null;
  postVisibleInteractionResult: LongSessionPostVisibleInteractionResult | null;
  postVisibleObserveMs: number;
  verboseTimelineReport: boolean;
  traceWaitErrors: string[];
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
  mutationEvents: LongSessionDomMutationEvent[];
  visualStateEvents: LongSessionVisualStateEvent[];
  layoutShiftEvents: LongSessionLayoutShiftEvent[];
  visualStateSummary: LongSessionVisualStateSummary;
  screenshotPath: string | null;
  events: StartupTraceSnapshot['phases']['events'];
  apiSegments: ReturnType<typeof summarizeApiCommandSegments>;
  api: StartupTraceSnapshot['api'];
  native: StartupTraceSnapshot['native'];
};

type LongSessionOpenMeasurementOptions = {
  requireFrameTrace?: boolean;
  expectNoHistoryLoadingAfterClick?: boolean;
  postVisibleInteraction?: LongSessionPostVisibleInteraction;
};

type LongSessionSendAfterOpenMeasurement = {
  appMode: string;
  sessionId: string;
  fixtureScenario: string | null;
  previousLatestTurnId: string;
  message: string;
  clickedAtMs: number;
  postSendObserveMs: number;
  beforeState: LongSessionActiveMessageState;
  afterSendState: LongSessionActiveMessageState;
  finalState: LongSessionActiveMessageState;
  observedLatestTurnId: string;
  viewport: LongSessionViewportState;
  viewportTimelineSummary: LongSessionViewportTimelineSummary;
  visualStateSummary: LongSessionVisualStateSummary;
  visualStateEvents: LongSessionVisualStateEvent[];
  mutationEvents: LongSessionDomMutationEvent[];
  layoutShiftEvents: LongSessionLayoutShiftEvent[];
  screenshotPath: string | null;
  events: StartupTraceSnapshot['phases']['events'];
  apiSegments: ReturnType<typeof summarizeApiCommandSegments>;
  api: StartupTraceSnapshot['api'];
  native: StartupTraceSnapshot['native'];
};

type LongSessionActiveMessageState = {
  activeSessionId: string | null;
  latestTurnId: string | null;
  dialogTurnCount: number;
  virtualItemCount: number;
  visibleTextLength: number;
  showHistoryLoadingLayer: string | null;
  showHistoryOpenIntentOverlay: string | null;
  showHistoryTransitionOverlay: string | null;
};

type RapidLongSessionSwitchMeasurement = {
  appMode: string;
  requestedSessionIds: string[];
  sessionIds: string[];
  activeSessionIdAtStart: string | null;
  targetSessionId: string;
  expectedLatestTurnId: string;
  clickedAtMs: number;
  clickPlan: Array<{
    sessionId: string;
    clickedAtMs: number;
    findDurationMs: number;
    clickDurationMs: number;
    pauseDurationMs?: number;
  }>;
  activeSessionIdAtEnd: string | null;
  targetLatestVisibleAtMs: number;
  targetLatestUsableAtMs: number;
  clickToTargetLatestVisibleMs: number;
  clickToTargetLatestUsableMs: number;
  rapidSwitchBreakdown: {
    firstClickToTargetClickMs: number;
    target: {
      clickedAtMs: number;
      clickSinceFirstClickMs: number;
      findDurationMs: number;
      clickDurationMs: number;
      pauseBeforeTargetMs: number;
      clickActionCompletedAtMs: number;
      timelineClickToLatestContentVisibleMs: number | null;
      timelineClickToLatestTextVisibleMs: number | null;
      timelineClickActionCompletedToLatestTextVisibleMs: number | null;
      clickActionCompletedToLatestVisibleMs: number;
      clickActionCompletedToLatestUsableMs: number;
      clickToLatestVisibleMs: number;
      clickToLatestUsableMs: number;
      latestVisibleToUsableMs: number;
    };
    sessions: Array<{
      sessionId: string;
      clickIndex: number;
      sinceFirstClickMs: number;
      eventCount: number;
      sessionOpen: ReturnType<typeof summarizeSessionOpen>;
    }>;
  };
  postVisibleObserveMs: number;
  viewport: LongSessionViewportState;
  viewportTimelineSummary: LongSessionViewportTimelineSummary;
  visualStateSummary: LongSessionVisualStateSummary;
  visualStateEvents: LongSessionVisualStateEvent[];
  mutationEvents: LongSessionDomMutationEvent[];
  layoutShiftEvents: LongSessionLayoutShiftEvent[];
  events: StartupTraceSnapshot['phases']['events'];
  apiSegments: ReturnType<typeof summarizeApiCommandSegments>;
  api: StartupTraceSnapshot['api'];
  native: StartupTraceSnapshot['native'];
};

function readPostVisibleInteractionOption(
  options: LongSessionOpenMeasurementOptions,
): LongSessionPostVisibleInteraction | null {
  const envValue = process.env.BITFUN_E2E_PERF_POST_VISIBLE_INTERACTION;
  if (
    envValue === 'first-scroll' ||
    envValue === 'scroll-down' ||
    envValue === 'resize-window' ||
    envValue === 'resize-window-width'
  ) {
    return envValue;
  }
  return options.postVisibleInteraction ?? null;
}

function isLongSessionResizeInteraction(
  interaction: LongSessionPostVisibleInteraction | null,
): boolean {
  return interaction === 'resize-window' || interaction === 'resize-window-width';
}

function getWebDriverSessionId(): string {
  const sessionId = (browser as unknown as { sessionId?: string }).sessionId;
  if (!sessionId) {
    throw new Error('WebDriver session id is not available for window resize measurement');
  }
  return sessionId;
}

function webDriverEndpoint(pathname: string): string {
  const port = Number(process.env.BITFUN_E2E_WEBDRIVER_PORT || 4445);
  return `http://127.0.0.1:${port}${pathname}`;
}

async function readWebDriverWindowRect(): Promise<LongSessionWindowRect> {
  const sessionId = getWebDriverSessionId();
  const response = await fetch(webDriverEndpoint(`/session/${sessionId}/window/rect`));
  if (!response.ok) {
    throw new Error(`Failed to read WebDriver window rect: ${response.status} ${await response.text()}`);
  }
  const payload = await response.json() as { value?: Partial<LongSessionWindowRect> };
  const rect = payload.value;
  if (
    !rect ||
    typeof rect.width !== 'number' ||
    typeof rect.height !== 'number'
  ) {
    throw new Error(`WebDriver window rect response is missing width/height: ${JSON.stringify(payload)}`);
  }
  return {
    x: typeof rect.x === 'number' ? rect.x : undefined,
    y: typeof rect.y === 'number' ? rect.y : undefined,
    width: rect.width,
    height: rect.height,
  };
}

async function setWebDriverWindowRect(rect: Partial<LongSessionWindowRect>): Promise<LongSessionWindowRect> {
  const sessionId = getWebDriverSessionId();
  const response = await fetch(webDriverEndpoint(`/session/${sessionId}/window/rect`), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(rect),
  });
  if (!response.ok) {
    throw new Error(`Failed to set WebDriver window rect: ${response.status} ${await response.text()}`);
  }
  return readWebDriverWindowRect();
}

async function waitForBrowserAnimationFrames(frameCount = 2): Promise<void> {
  await browser.executeAsync((frames, done) => {
    let remaining = frames;
    const waitFrame = () => {
      remaining -= 1;
      if (remaining <= 0) {
        done();
        return;
      }
      requestAnimationFrame(waitFrame);
    };
    requestAnimationFrame(waitFrame);
  }, frameCount);
}

async function readLongSessionScrollerMetrics(): Promise<{
  scrollTop: number;
  maxScrollTop: number;
  clientHeight: number;
}> {
  return browser.execute(() => {
    const scroller = document.querySelector<HTMLElement>(
      '.modern-flowchat-container__messages [data-virtuoso-scroller="true"], ' +
      '.modern-flowchat-container__messages [data-virtuoso-scroller]',
    );
    if (!scroller) {
      throw new Error('Could not find long-session scroller for post-visible interaction');
    }
    return {
      scrollTop: scroller.scrollTop,
      maxScrollTop: Math.max(0, scroller.scrollHeight - scroller.clientHeight),
      clientHeight: scroller.clientHeight,
    };
  });
}

async function recordLongSessionPostVisibleInteraction(
  detail: LongSessionPostVisibleInteractionResult,
): Promise<void> {
  await browser.execute((eventDetail) => {
    window.dispatchEvent(new CustomEvent('bitfun:e2e-long-session-user-interaction', {
      detail: eventDetail,
    }));
  }, detail);
}

async function performLongSessionPostVisibleInteraction(
  interaction: LongSessionPostVisibleInteraction,
): Promise<LongSessionPostVisibleInteractionResult> {
  if (isLongSessionResizeInteraction(interaction)) {
    const beforeWindowRect = await readWebDriverWindowRect();
    const beforeMetrics = await readLongSessionScrollerMetrics();
    const nextWidth = Math.max(
      960,
      Math.round(beforeWindowRect.width - Math.min(420, Math.max(220, beforeWindowRect.width * 0.22))),
    );
    const nextHeight = interaction === 'resize-window-width'
      ? beforeWindowRect.height
      : Math.max(
        540,
        Math.round(beforeWindowRect.height - Math.min(260, Math.max(140, beforeWindowRect.height * 0.22))),
      );
    await recordLongSessionPostVisibleInteraction({
      type: interaction,
      beforeScrollTop: beforeMetrics.scrollTop,
      afterScrollTop: beforeMetrics.scrollTop,
      maxScrollTop: beforeMetrics.maxScrollTop,
      deltaY: 0,
      beforeClientHeight: beforeMetrics.clientHeight,
      beforeWindowRect,
    });
    const afterWindowRect = await setWebDriverWindowRect({
      x: beforeWindowRect.x,
      y: beforeWindowRect.y,
      width: interaction === 'resize-window-width' ? nextWidth : beforeWindowRect.width,
      height: nextHeight,
    });
    await waitForBrowserAnimationFrames(3);
    const afterMetrics = await readLongSessionScrollerMetrics();
    const result: LongSessionPostVisibleInteractionResult = {
      type: interaction,
      beforeScrollTop: beforeMetrics.scrollTop,
      afterScrollTop: afterMetrics.scrollTop,
      maxScrollTop: afterMetrics.maxScrollTop,
      deltaY: 0,
      beforeClientHeight: beforeMetrics.clientHeight,
      afterClientHeight: afterMetrics.clientHeight,
      beforeWindowRect,
      afterWindowRect,
    };
    return result;
  }

  return browser.execute((interactionType) => {
    const scroller = document.querySelector<HTMLElement>(
      '.modern-flowchat-container__messages [data-virtuoso-scroller="true"], ' +
      '.modern-flowchat-container__messages [data-virtuoso-scroller]',
    );
    if (!scroller) {
      throw new Error('Could not find long-session scroller for first-scroll interaction');
    }

    const beforeScrollTop = scroller.scrollTop;
    const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    const direction = interactionType === 'first-scroll' ? -1 : 1;
    const deltaY = direction * Math.min(520, Math.max(160, scroller.clientHeight * 0.45));
    scroller.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      deltaMode: WheelEvent.DOM_DELTA_PIXEL,
      deltaY,
    }));
    scroller.scrollTop = Math.max(0, Math.min(maxScrollTop, beforeScrollTop + deltaY));
    scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    window.dispatchEvent(new CustomEvent('bitfun:e2e-long-session-user-interaction', {
      detail: {
        type: interactionType,
        beforeScrollTop,
        afterScrollTop: scroller.scrollTop,
        maxScrollTop,
        deltaY,
      },
    }));
    return {
      type: interactionType,
      beforeScrollTop,
      afterScrollTop: scroller.scrollTop,
      maxScrollTop,
      deltaY,
    };
  }, interaction);
}

async function restoreLongSessionPostVisibleInteraction(
  result: LongSessionPostVisibleInteractionResult | null,
): Promise<void> {
  if (!result?.beforeWindowRect) {
    return;
  }
  await setWebDriverWindowRect(result.beforeWindowRect);
  await waitForBrowserAnimationFrames(2);
}

function readRapidSwitchSessionIds(): string[] {
  const raw = process.env.BITFUN_E2E_PERF_RAPID_SWITCH_SESSION_IDS;
  const sessionIds = (raw ?? 'perf-rapid-a-000,perf-rapid-b-000,perf-rapid-c-000')
    .split(',')
    .map(value => value.trim())
    .filter(Boolean);
  return Array.from(new Set(sessionIds));
}

async function readActiveSessionNavId(): Promise<string | null> {
  return browser.execute(() => {
    const active = document.querySelector<HTMLElement>(
      '[data-testid="session-nav-item"].is-active, ' +
      '[data-testid="nav-session-item"].is-active, ' +
      '.bitfun-nav-panel__inline-item.is-active[data-session-id]',
    );
    return active?.getAttribute('data-session-id') ?? null;
  });
}

async function ensureRapidSwitchTargetStartsInactive(
  targetSessionId: string,
  requestedSessionIds: string[],
): Promise<string | null> {
  const activeSessionId = await readActiveSessionNavId();
  if (activeSessionId !== targetSessionId) {
    return activeSessionId;
  }

  const setupSessionId = requestedSessionIds.find(sessionId => sessionId !== targetSessionId);
  if (!setupSessionId) {
    return activeSessionId;
  }

  const setupItem = await findSessionItem(setupSessionId);
  if (!setupItem) {
    return activeSessionId;
  }

  const beforeSnapshot = await readStartupTraceSnapshot();
  const frameCountBefore = countPhase(
    beforeSnapshot,
    'historical_session_after_state_commit_frame',
  );
  const clickedAtMs = await readPerformanceNow();
  await setupItem.click();
  const afterFrameSnapshot = await waitForOptionalTracePhaseForSessionSince(
    'historical_session_after_state_commit_frame',
    setupSessionId,
    clickedAtMs,
    10000,
  );
  if (countPhase(afterFrameSnapshot, 'historical_session_after_state_commit_frame') <= frameCountBefore) {
    await waitForOptionalPhaseCount(
      'historical_session_after_state_commit_frame',
      frameCountBefore + 1,
      1000,
    );
  }
  await browser.pause(50);
  return readActiveSessionNavId();
}

async function collectRapidLongSessionSwitchMeasurement(
  requestedSessionIds: string[],
  sessionIds: string[],
  activeSessionIdAtStart: string | null,
  expectedLatestTurnId: string,
): Promise<RapidLongSessionSwitchMeasurement | null> {
  if (sessionIds.length < 3) {
    throw new Error('Rapid switch measurement requires at least 3 session ids');
  }

  for (const sessionId of sessionIds) {
    const item = await findSessionItem(sessionId);
    if (!item) {
      return null;
    }
  }

  const targetSessionId = sessionIds[sessionIds.length - 1];
  const clickedAtMs = await readPerformanceNow();
  const clickPlan: RapidLongSessionSwitchMeasurement['clickPlan'] = [];
  await startLongSessionViewportTimelineRecorder(
    expectedLatestTurnId,
    clickedAtMs,
    process.env.BITFUN_E2E_RENDER_PROFILE === '1',
  );

  for (let index = 0; index < sessionIds.length; index += 1) {
    const sessionId = sessionIds[index];
    const findStartedAtRunnerMs = nodePerformance.now();
    const item = await findSessionItem(sessionId);
    const findDurationMs = nodePerformance.now() - findStartedAtRunnerMs;
    if (!item) {
      throw new Error(`Rapid switch session disappeared before click: ${sessionId}`);
    }
    const itemClickedAtMs = await readPerformanceNow();
    const clickPlanEntry: RapidLongSessionSwitchMeasurement['clickPlan'][number] = {
      sessionId,
      clickedAtMs: itemClickedAtMs,
      findDurationMs,
      clickDurationMs: 0,
    };
    clickPlan.push({
      ...clickPlanEntry,
    });
    const clickStartedAtRunnerMs = nodePerformance.now();
    await item.click();
    clickPlanEntry.clickDurationMs = nodePerformance.now() - clickStartedAtRunnerMs;
    clickPlan[index] = clickPlanEntry;
    if (index < sessionIds.length - 1) {
      const delayMs = numericEnv('BITFUN_E2E_PERF_RAPID_SWITCH_DELAY_MS') ?? 75;
      const pauseStartedAtRunnerMs = nodePerformance.now();
      await browser.pause(Math.max(0, delayMs));
      clickPlanEntry.pauseDurationMs = nodePerformance.now() - pauseStartedAtRunnerMs;
      clickPlan[index] = clickPlanEntry;
    }
  }

  const latestVisible = await waitForLatestLongSessionTurnVisible(5000, expectedLatestTurnId);
  const latestUsable = await waitForLatestLongSessionViewportUsable(5000, expectedLatestTurnId);
  const postVisibleObserveMs =
    numericEnv('BITFUN_E2E_PERF_RAPID_SWITCH_OBSERVE_MS') ??
    numericEnv('BITFUN_E2E_PERF_POST_VISIBLE_OBSERVE_MS') ??
    DEFAULT_POST_VISIBLE_OBSERVE_MS;
  const observeRemainingMs = latestUsable.usableAtMs + postVisibleObserveMs - await readPerformanceNow();
  if (observeRemainingMs > 0) {
    await browser.pause(Math.ceil(observeRemainingMs));
  }

  const activeSessionIdAtEnd = await readActiveSessionNavId();
  const viewport = await readLongSessionViewportState(expectedLatestTurnId);
  const viewportTimeline = await stopLongSessionViewportTimelineRecorder();
  const viewportTimelineSummary = summarizeLongSessionViewportTimeline(
    viewportTimeline.samples,
    targetSessionId,
  );
  const visualStateSummary = summarizeLongSessionVisualStateEvents(
    viewportTimeline.visualStateEvents,
    viewportTimeline.mutationEvents,
    viewportTimeline.layoutShiftEvents,
    viewportTimelineSummary,
    targetSessionId,
  );
  const targetClickedAtMs =
    clickPlan.find(entry => entry.sessionId === targetSessionId)?.clickedAtMs ?? clickedAtMs;
  const targetClickPlanIndex = clickPlan.findIndex(entry => entry.sessionId === targetSessionId);
  const targetClickPlanEntry = targetClickPlanIndex >= 0
    ? clickPlan[targetClickPlanIndex]
    : undefined;
  const targetClickDurationMs = targetClickPlanEntry?.clickDurationMs ?? 0;
  const targetClickActionCompletedAtMs = targetClickedAtMs + targetClickDurationMs;
  const pauseBeforeTargetMs = clickPlan
    .slice(0, Math.max(0, targetClickPlanIndex))
    .reduce((total, entry) => total + (entry.pauseDurationMs ?? 0), 0);
  const targetClickSinceFirstClickMs = targetClickedAtMs - clickedAtMs;
  const targetClickActionCompletedSinceFirstClickMs =
    targetClickActionCompletedAtMs - clickedAtMs;
  const relativeToTargetClick = (timelineMs: number | null): number | null =>
    timelineMs === null
      ? null
      : timelineMs - targetClickSinceFirstClickMs;
  const relativeToTargetClickActionCompleted = (timelineMs: number | null): number | null =>
    timelineMs === null
      ? null
      : timelineMs - targetClickActionCompletedSinceFirstClickMs;
  const timelineClickToLatestContentVisibleMs =
    relativeToTargetClick(viewportTimelineSummary.firstLatestContentVisuallyVisibleAtMs);
  const timelineClickToLatestTextVisibleMs =
    relativeToTargetClick(viewportTimelineSummary.firstLatestTextVisibleAtMs);
  const timelineClickActionCompletedToLatestTextVisibleMs =
    relativeToTargetClickActionCompleted(viewportTimelineSummary.firstLatestTextVisibleAtMs);
  const finalSnapshot = await readStartupTraceSnapshot();
  const sessionIdSet = new Set(sessionIds);
  const events = finalSnapshot.phases.events.filter(event =>
    event.atMs >= clickedAtMs &&
    (
      (
        event.phase.startsWith('historical_session') &&
        typeof event.sessionId === 'string' &&
        sessionIdSet.has(event.sessionId)
      ) ||
      event.phase.startsWith('flowchat_latest_end_anchor') ||
      event.phase.startsWith('flowchat_initial_history') ||
      event.phase === 'react_render_profile' ||
      event.phase === 'git_status_request' ||
      event.phase === 'git_state_refresh'
    )
  );
  const sessionBreakdowns = clickPlan.map((entry, index) => {
    const sessionEvents = events.filter(event =>
      typeof event.sessionId === 'string' &&
      event.sessionId === entry.sessionId
    );

    return {
      sessionId: entry.sessionId,
      clickIndex: index,
      sinceFirstClickMs: entry.clickedAtMs - clickedAtMs,
      eventCount: sessionEvents.length,
      sessionOpen: summarizeSessionOpen(sessionEvents, entry.clickedAtMs),
    };
  });
  const verboseTimelineReport = process.env.BITFUN_E2E_PERF_VERBOSE_REPORT === '1';

  return {
    appMode: process.env.BITFUN_E2E_APP_MODE ?? 'auto',
    requestedSessionIds,
    sessionIds,
    activeSessionIdAtStart,
    targetSessionId,
    expectedLatestTurnId,
    clickedAtMs,
    clickPlan,
    activeSessionIdAtEnd,
    targetLatestVisibleAtMs: latestVisible.visibleAtMs,
    targetLatestUsableAtMs: latestUsable.usableAtMs,
    clickToTargetLatestVisibleMs: latestVisible.visibleAtMs - clickedAtMs,
    clickToTargetLatestUsableMs: latestUsable.usableAtMs - clickedAtMs,
    rapidSwitchBreakdown: {
      firstClickToTargetClickMs: targetClickedAtMs - clickedAtMs,
      target: {
        clickedAtMs: targetClickedAtMs,
        clickSinceFirstClickMs: targetClickSinceFirstClickMs,
        findDurationMs: targetClickPlanEntry?.findDurationMs ?? 0,
        clickDurationMs: targetClickDurationMs,
        pauseBeforeTargetMs,
        clickActionCompletedAtMs: targetClickActionCompletedAtMs,
        timelineClickToLatestContentVisibleMs,
        timelineClickToLatestTextVisibleMs,
        timelineClickActionCompletedToLatestTextVisibleMs,
        clickActionCompletedToLatestVisibleMs: latestVisible.visibleAtMs - targetClickActionCompletedAtMs,
        clickActionCompletedToLatestUsableMs: latestUsable.usableAtMs - targetClickActionCompletedAtMs,
        clickToLatestVisibleMs: latestVisible.visibleAtMs - targetClickedAtMs,
        clickToLatestUsableMs: latestUsable.usableAtMs - targetClickedAtMs,
        latestVisibleToUsableMs: latestUsable.usableAtMs - latestVisible.visibleAtMs,
      },
      sessions: sessionBreakdowns,
    },
    postVisibleObserveMs,
    viewport,
    viewportTimelineSummary,
    visualStateSummary,
    visualStateEvents: verboseTimelineReport ? viewportTimeline.visualStateEvents : [],
    mutationEvents: verboseTimelineReport ? viewportTimeline.mutationEvents : [],
    layoutShiftEvents: verboseTimelineReport ? viewportTimeline.layoutShiftEvents : [],
    events,
    apiSegments: summarizeApiCommandSegments(finalSnapshot),
    api: finalSnapshot.api,
    native: finalSnapshot.native,
  };
}

function expectRapidSwitchMeasurementUsesFreshTargetRestore(
  measurement: RapidLongSessionSwitchMeasurement,
): void {
  const targetSession = measurement.rapidSwitchBreakdown.sessions.find(session =>
    session.sessionId === measurement.targetSessionId
  );
  if (!targetSession) {
    throw new Error(`Rapid switch target ${measurement.targetSessionId} is missing from session breakdown`);
  }
  if (measurement.activeSessionIdAtStart === measurement.targetSessionId) {
    throw new Error(
      `Rapid switch target ${measurement.targetSessionId} was already active at test start; ` +
      'this would measure a startup-preloaded session instead of a first rapid switch.',
    );
  }

  const open = targetSession.sessionOpen;
  const missingSegments = [
    ['clickToHydrateStartMs', open.clickToHydrateStartMs],
    ['clickToHydrateEndMs', open.clickToHydrateEndMs],
    ['restoreDurationMs', open.restoreDurationMs],
    ['hydrateDurationMs', open.hydrateDurationMs],
    ['latestFrameSinceHydrateMs', open.latestFrameSinceHydrateMs],
  ]
    .filter(([, value]) => typeof value !== 'number' || value < 0)
    .map(([name]) => name);

  if (missingSegments.length > 0) {
    throw new Error(
      `Rapid switch target ${measurement.targetSessionId} did not produce fresh restore timings ` +
      `(${missingSegments.join(', ')} missing/invalid). ` +
      `activeAtStart=${measurement.activeSessionIdAtStart ?? 'none'}, ` +
      `requested=${measurement.requestedSessionIds.join(',')}, ` +
      `effective=${measurement.sessionIds.join(',')}`,
    );
  }

  const target = measurement.rapidSwitchBreakdown.target;
  if (
    target.timelineClickToLatestTextVisibleMs !== null &&
    target.timelineClickToLatestTextVisibleMs < 0
  ) {
    throw new Error(
      `Rapid switch latest text became visible before target click ` +
      `(${target.timelineClickToLatestTextVisibleMs.toFixed(1)}ms). ` +
      `activeAtStart=${measurement.activeSessionIdAtStart ?? 'none'}, ` +
      `requested=${measurement.requestedSessionIds.join(',')}, ` +
      `effective=${measurement.sessionIds.join(',')}`,
    );
  }
}

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
  const requireLatestModelRound = requiresLatestModelRoundForFixture(fixtureScenario);

  const requireFrameTrace = options.requireFrameTrace !== false;
  const afterFrameTimeoutMs = requireFrameTrace ? 20000 : 1000;
  const fullHydrateTimeoutMs = requireFrameTrace ? 10000 : 1000;
  const latestAnchorTimeoutMs = requireFrameTrace ? 5000 : 1000;
  const clickedAtMs = await readPerformanceNow();
  const traceWaitErrors: string[] = [];
  const waitForRequiredTracePhase = async (
    phase: string,
    timeoutMs: number,
  ): Promise<StartupTraceSnapshot> => {
    try {
      return await waitForTracePhaseForSessionSince(
        phase,
        sessionId,
        clickedAtMs,
        timeoutMs,
      );
    } catch (error) {
      traceWaitErrors.push(error instanceof Error ? error.message : String(error));
      return readStartupTraceSnapshot();
    }
  };
  await startLongSessionViewportTimelineRecorder(
    expectedLatestTurnId,
    clickedAtMs,
    process.env.BITFUN_E2E_RENDER_PROFILE === '1',
  );

  await item.click();
  const latestVisiblePromise = waitForLatestLongSessionTurnVisible(5000, expectedLatestTurnId);
  const latestUsablePromise = waitForLatestLongSessionViewportUsable(
    5000,
    expectedLatestTurnId,
    { requireLatestModelRound },
  );
  const postVisibleInteraction = readPostVisibleInteractionOption(options);
  let postVisibleInteractionResult: LongSessionPostVisibleInteractionResult | null = null;
  let latestVisible: Awaited<typeof latestVisiblePromise> | null = null;
  let latestUsable: Awaited<typeof latestUsablePromise> | null = null;

  if (postVisibleInteraction) {
    latestVisible = await latestVisiblePromise;
    latestUsable = await latestUsablePromise;
    postVisibleInteractionResult = await performLongSessionPostVisibleInteraction(postVisibleInteraction);
  }

  const afterFrameSnapshot = requireFrameTrace
    ? await waitForRequiredTracePhase(
      'historical_session_after_state_commit_frame',
      afterFrameTimeoutMs,
    )
    : await waitForOptionalTracePhaseForSessionSince(
      'historical_session_after_state_commit_frame',
      sessionId,
      clickedAtMs,
      afterFrameTimeoutMs,
    );
  const afterFullSnapshot = await waitForOptionalTracePhaseForSessionSince(
    'historical_session_full_hydrate_end',
    sessionId,
    clickedAtMs,
    fullHydrateTimeoutMs,
  );
  const afterFullFrameSnapshot = await waitForOptionalTracePhaseForSessionSince(
    'historical_session_full_hydrate_after_state_commit_frame',
    sessionId,
    clickedAtMs,
    Math.min(fullHydrateTimeoutMs, 1000),
  );
  const afterAnchorSnapshot = await waitForOptionalTracePhaseForSessionSince(
    'historical_session_latest_anchor_attempt',
    sessionId,
    clickedAtMs,
    latestAnchorTimeoutMs,
  );
  latestVisible ??= await latestVisiblePromise;
  latestUsable ??= await latestUsablePromise;
  const postVisibleObserveMs =
    numericEnv('BITFUN_E2E_PERF_POST_VISIBLE_OBSERVE_MS') ?? DEFAULT_POST_VISIBLE_OBSERVE_MS;
  const observeRemainingMs = latestUsable.usableAtMs + postVisibleObserveMs - await readPerformanceNow();
  if (observeRemainingMs > 0) {
    await browser.pause(Math.ceil(observeRemainingMs));
  }
  let finalViewport = await readLongSessionViewportState(expectedLatestTurnId);
  let finalViewportCheckedAtMs = await readPerformanceNow();
  if (!isLongSessionViewportUsable(finalViewport, { requireLatestModelRound })) {
    const finalUsable = await waitForLatestLongSessionViewportUsable(
      3000,
      expectedLatestTurnId,
      { requireLatestModelRound },
    );
    finalViewport = finalUsable.viewport;
    finalViewportCheckedAtMs = finalUsable.usableAtMs;
  }
  const viewportTimeline = await stopLongSessionViewportTimelineRecorder();
  const viewportTimelineSummary = summarizeLongSessionViewportTimeline(
    viewportTimeline.samples,
    sessionId,
  );
  const visualStateSummary = summarizeLongSessionVisualStateEvents(
    viewportTimeline.visualStateEvents,
    viewportTimeline.mutationEvents,
    viewportTimeline.layoutShiftEvents,
    viewportTimelineSummary,
    sessionId,
  );
  const verboseTimelineReport = process.env.BITFUN_E2E_PERF_VERBOSE_REPORT === '1';
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
      (
        event.phase.startsWith('historical_session') &&
        traceEventSessionId(event) === sessionId
      ) ||
      event.phase.startsWith('flowchat_latest_end_anchor') ||
      event.phase.startsWith('flowchat_initial_history') ||
      event.phase === 'react_render_profile' ||
      event.phase === 'git_status_request' ||
      event.phase === 'git_state_refresh'
    )
  );
  const screenshotPath = await maybeSavePerfScreenshot(`long-session-${sessionId}`);

  const measurement: LongSessionOpenMeasurement = {
    appMode: process.env.BITFUN_E2E_APP_MODE ?? 'auto',
    sessionId,
    fixtureScenario,
    expectedLatestTurnId,
    postVisibleInteraction,
    postVisibleInteractionResult,
    postVisibleObserveMs,
    verboseTimelineReport,
    traceWaitErrors,
    clickedAtMs,
    sessionOpen: summarizeSessionOpen(sessionEvents, clickedAtMs),
    latestVisibleAtMs: latestVisible.visibleAtMs,
    clickToLatestVisibleMs: latestVisible.visibleAtMs - clickedAtMs,
    latestUsableAtMs: latestUsable.usableAtMs,
    clickToLatestUsableMs: latestUsable.usableAtMs - clickedAtMs,
    latestAnswerTextVisibleAtMs: latestUsable.usableAtMs,
    clickToLatestAnswerTextVisibleMs: latestUsable.usableAtMs - clickedAtMs,
    finalViewportCheckedAtMs,
    ...(requireFrameTrace && traceWaitErrors.length === 0
      ? {
        postHydrateUsableAtMs: finalViewportCheckedAtMs,
        clickToPostHydrateUsableMs: finalViewportCheckedAtMs - clickedAtMs,
      }
      : {}),
    latestVisibleViewport: latestVisible.viewport,
    latestUsableViewport: latestUsable.viewport,
    latestAnswerTextVisibleViewport: latestUsable.viewport,
    viewport: finalViewport,
    viewportTimeline: verboseTimelineReport ? viewportTimeline.samples : [],
    viewportTimelineSummary,
    mainThreadTasks: verboseTimelineReport ? viewportTimeline.mainThreadTasks : [],
    mutationEvents: verboseTimelineReport ? viewportTimeline.mutationEvents : [],
    visualStateEvents: verboseTimelineReport ? viewportTimeline.visualStateEvents : [],
    layoutShiftEvents: verboseTimelineReport ? viewportTimeline.layoutShiftEvents : [],
    visualStateSummary,
    screenshotPath,
    events: sessionEvents,
    apiSegments: summarizeApiCommandSegments(finalSnapshot),
    api: finalSnapshot.api,
    native: finalSnapshot.native,
  };
  await restoreLongSessionPostVisibleInteraction(postVisibleInteractionResult);
  return measurement;
}

async function readLongSessionActiveMessageState(): Promise<LongSessionActiveMessageState> {
  return browser.execute(() => {
    const messages = document.querySelector<HTMLElement>('.modern-flowchat-container__messages');
    const virtualItems = Array.from(
      document.querySelectorAll<HTMLElement>('.modern-flowchat-container__messages .virtual-item-wrapper'),
    );
    const visibleTextLength = virtualItems.reduce((total, item) => {
      const rect = item.getBoundingClientRect();
      const style = window.getComputedStyle(item);
      if (
        rect.width <= 0 ||
        rect.height <= 0 ||
        style.display === 'none' ||
        style.visibility === 'hidden' ||
        Number(style.opacity || '1') <= 0.01
      ) {
        return total;
      }
      return total + (item.innerText?.length ?? 0);
    }, 0);
    const numeric = (value: string | undefined): number => {
      const parsed = Number(value ?? 0);
      return Number.isFinite(parsed) ? parsed : 0;
    };

    return {
      activeSessionId: messages?.dataset.activeSessionId || null,
      latestTurnId: messages?.dataset.latestTurnId || null,
      dialogTurnCount: numeric(messages?.dataset.dialogTurnCount),
      virtualItemCount: numeric(messages?.dataset.virtualItemCount),
      visibleTextLength,
      showHistoryLoadingLayer: messages?.dataset.showHistoryLoadingLayer || null,
      showHistoryOpenIntentOverlay: messages?.dataset.showHistoryOpenIntentOverlay || null,
      showHistoryTransitionOverlay: messages?.dataset.showHistoryTransitionOverlay || null,
    };
  });
}

async function ensureLongSessionOpenForSend(
  sessionId: string,
  expectedLatestTurnId: string,
): Promise<boolean> {
  await switchAwayFromSession(sessionId);

  const activeSessionId = await readActiveSessionNavId();
  if (activeSessionId !== sessionId) {
    const item = await findSessionItem(sessionId);
    if (!item) {
      return false;
    }
    await item.click();
  }

  const fixtureScenario = await readLongSessionFixtureScenario(sessionId);
  await waitForLatestLongSessionTurnVisible(5000, expectedLatestTurnId);
  await waitForLatestLongSessionViewportUsable(
    5000,
    expectedLatestTurnId,
    { requireLatestModelRound: requiresLatestModelRoundForFixture(fixtureScenario) },
  );
  await waitForBrowserAnimationFrames(2);
  return true;
}

async function clearLongSessionPendingQueue(sessionId: string): Promise<void> {
  await browser.execute(async (targetSessionId) => {
    try {
      const { pendingQueueManager } = await import(
        '/src/flow_chat/services/flow-chat-manager/PendingQueueModule.ts'
      );
      pendingQueueManager.clear(targetSessionId);
    } catch {
      // Keep the localStorage fallback below for bundled/runtime import differences.
    }
    try {
      window.localStorage.removeItem(`flowChat.pendingQueue.${targetSessionId}`);
    } catch {
      // localStorage may be unavailable in unusual WebView states.
    }
  }, sessionId);

  await browser.waitUntil(async () => {
    const pendingQueueCount = await browser.execute(() =>
      document.querySelectorAll('[data-testid="pending-queue-panel"] .bitfun-pending-queue-panel__item').length
    );
    return pendingQueueCount === 0;
  }, {
    timeout: 1000,
    interval: 50,
    timeoutMsg: 'Pending queue did not clear before send-after-open measurement',
  }).catch(() => undefined);
}

async function setLongSessionChatInputValue(message: string): Promise<void> {
  await browser.execute((value) => {
    const input = document.querySelector<HTMLElement>(
      '.rich-text-input[contenteditable="true"], ' +
      '[data-testid="chat-input-textarea"], ' +
      '.bitfun-chat-input [contenteditable="true"]',
    );
    if (!input) {
      throw new Error('Chat input was not found');
    }

    input.focus();
    if (input.isContentEditable) {
      input.textContent = value;
    } else if ('value' in input) {
      (input as HTMLInputElement | HTMLTextAreaElement).value = value;
    } else {
      input.textContent = value;
    }

    const inputEvent = typeof InputEvent !== 'undefined'
      ? new InputEvent('input', { bubbles: true, inputType: 'insertText', data: value })
      : new Event('input', { bubbles: true });
    input.dispatchEvent(inputEvent);
    input.dispatchEvent(new Event('change', { bubbles: true }));
  }, message);

  await browser.waitUntil(async () => {
    const button = await $('[data-testid="chat-input-send-btn"], button.bitfun-chat-input__send-button');
    if (!await button.isExisting()) {
      return false;
    }
    const disabled = await button.getAttribute('disabled');
    const ariaDisabled = await button.getAttribute('aria-disabled');
    return await button.isEnabled() && disabled === null && ariaDisabled !== 'true';
  }, {
    timeout: 3000,
    interval: 50,
    timeoutMsg: 'Chat send button did not become enabled',
  });
}

async function clickLongSessionSendButton(): Promise<void> {
  const button = await $('[data-testid="chat-input-send-btn"], button.bitfun-chat-input__send-button');
  await button.waitForExist({ timeout: 3000 });
  await button.click();
  await browser.execute(() => {
    window.dispatchEvent(new CustomEvent('bitfun:e2e-long-session-user-interaction', {
      detail: { type: 'send-message' },
    }));
  });
}

async function collectLongSessionSendAfterOpenMeasurement(
  sessionId: string,
  previousLatestTurnId: string,
): Promise<LongSessionSendAfterOpenMeasurement> {
  const fixtureScenario = await readLongSessionFixtureScenario(sessionId);
  const message = `E2E history send blank probe ${Date.now()}`;

  await clearLongSessionPendingQueue(sessionId);
  await setLongSessionChatInputValue(message);
  const beforeState = await readLongSessionActiveMessageState();
  if (beforeState.activeSessionId !== sessionId) {
    throw new Error(
      `Expected active session ${sessionId} before send, got ${beforeState.activeSessionId ?? 'none'}`,
    );
  }

  const clickedAtMs = await readPerformanceNow();
  await startLongSessionViewportTimelineRecorder(
    previousLatestTurnId,
    clickedAtMs,
    process.env.BITFUN_E2E_RENDER_PROFILE === '1',
  );

  await clickLongSessionSendButton();

  let afterSendState = await readLongSessionActiveMessageState();
  await browser.waitUntil(async () => {
    const state = await readLongSessionActiveMessageState();
    const changed =
      state.dialogTurnCount !== beforeState.dialogTurnCount ||
      state.latestTurnId !== beforeState.latestTurnId ||
      state.virtualItemCount !== beforeState.virtualItemCount;
    if (changed) {
      afterSendState = state;
    }
    return changed;
  }, {
    timeout: 1500,
    interval: 25,
    timeoutMsg: 'Send did not change the active session message state',
  }).catch(() => undefined);

  const postSendObserveMs = numericEnv('BITFUN_E2E_PERF_SEND_AFTER_OPEN_OBSERVE_MS') ?? 2000;
  if (postSendObserveMs > 0) {
    await browser.pause(postSendObserveMs);
  }

  const finalState = await readLongSessionActiveMessageState();
  const observedLatestTurnId =
    finalState.latestTurnId ||
    afterSendState.latestTurnId ||
    beforeState.latestTurnId ||
    previousLatestTurnId;
  const viewport = await readLongSessionViewportState(observedLatestTurnId);
  const viewportTimeline = await stopLongSessionViewportTimelineRecorder();
  const viewportTimelineSummary = summarizeLongSessionViewportTimeline(
    viewportTimeline.samples,
    sessionId,
  );
  const visualStateSummary = summarizeLongSessionVisualStateEvents(
    viewportTimeline.visualStateEvents,
    viewportTimeline.mutationEvents,
    viewportTimeline.layoutShiftEvents,
    viewportTimelineSummary,
    sessionId,
  );
  const verboseTimelineReport = process.env.BITFUN_E2E_PERF_VERBOSE_REPORT === '1';
  const finalSnapshot = await readStartupTraceSnapshot();
  const events = finalSnapshot.phases.events.filter(event =>
    event.atMs >= clickedAtMs &&
    (
      (
        event.phase.startsWith('historical_session') &&
        traceEventSessionId(event) === sessionId
      ) ||
      event.phase.startsWith('flowchat') ||
      event.phase === 'react_render_profile' ||
      event.phase === 'git_status_request' ||
      event.phase === 'git_state_refresh'
    )
  );
  const screenshotPath = await maybeSavePerfScreenshot(`long-session-send-${sessionId}`);

  return {
    appMode: process.env.BITFUN_E2E_APP_MODE ?? 'auto',
    sessionId,
    fixtureScenario,
    previousLatestTurnId,
    message,
    clickedAtMs,
    postSendObserveMs,
    beforeState,
    afterSendState,
    finalState,
    observedLatestTurnId,
    viewport,
    viewportTimelineSummary,
    visualStateSummary,
    visualStateEvents: verboseTimelineReport ? viewportTimeline.visualStateEvents : [],
    mutationEvents: verboseTimelineReport ? viewportTimeline.mutationEvents : [],
    layoutShiftEvents: verboseTimelineReport ? viewportTimeline.layoutShiftEvents : [],
    screenshotPath,
    events,
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
  const requireLatestModelRound = requiresLatestModelRoundForFixture(measurement.fixtureScenario);
  const requireFrameTrace = options.requireFrameTrace !== false;
  expect(measurement.clickToLatestVisibleMs).toBeGreaterThan(0);
  expect(measurement.clickToLatestUsableMs).toBeGreaterThan(0);
  if (requireFrameTrace && measurement.traceWaitErrors.length > 0) {
    throw new Error(
      `Long session measurement missing required trace phases: ${measurement.traceWaitErrors.join('; ')}`,
    );
  }
  if (requireFrameTrace) {
    expect(measurement.clickToPostHydrateUsableMs).toBeGreaterThan(0);
    expect(measurement.sessionOpen.hydrateDurationMs).toBeGreaterThan(0);
    expect(measurement.sessionOpen.latestFrameSinceHydrateMs).toBeGreaterThan(0);
    expect(measurement.sessionOpen.clickToLatestFrameMs).toBeGreaterThan(0);
  }
  expect(measurement.viewport.hasScroller).toBe(true);
  expect(measurement.viewport.latestContentVisible).toBe(true);
  expect(measurement.viewport.latestContentVisuallyVisible).toBe(true);
  expect(measurement.viewport.historyPlaceholderCoversMessages).toBe(false);
  if (requireLatestModelRound) {
    expect(measurement.viewport.latestModelRoundVisible).toBe(true);
    expect(measurement.viewport.latestModelRoundTextLength).toBeGreaterThan(0);
  } else {
    expect(measurement.viewport.latestVisible).toBe(true);
  }
  expect(measurement.viewport.latestTurnId).toBe(measurement.expectedLatestTurnId);
  expect(measurement.latestVisibleViewport.hasScroller).toBe(true);
  expect(measurement.latestVisibleViewport.latestContentVisible).toBe(true);
  expect(measurement.latestVisibleViewport.latestContentVisuallyVisible).toBe(true);
  expect(measurement.latestVisibleViewport.historyPlaceholderCoversMessages).toBe(false);
  if (requireLatestModelRound) {
    expect(measurement.latestVisibleViewport.latestModelRoundVisible).toBe(true);
  } else {
    expect(measurement.latestVisibleViewport.latestVisible).toBe(true);
  }
  expect(measurement.latestVisibleViewport.latestTurnId).toBe(measurement.expectedLatestTurnId);
  expect(isLongSessionLatestVisibleViewportPositioned(measurement.latestVisibleViewport)).toBe(true);
  expect(isLongSessionLatestTailAnchored(measurement.latestVisibleViewport)).toBe(true);
  if (requireLatestModelRound) {
    expect(measurement.latestAnswerTextVisibleViewport.latestModelRoundVisible).toBe(true);
    expect(measurement.latestAnswerTextVisibleViewport.latestModelRoundTextLength).toBeGreaterThan(0);
    expect(isLongSessionViewportUsable(measurement.latestAnswerTextVisibleViewport)).toBe(true);
    expect(isLongSessionLatestTailAnchored(measurement.latestAnswerTextVisibleViewport)).toBe(true);
    if (measurement.viewportTimelineSummary.latestTextDelayAfterContentVisuallyVisibleMs !== null) {
      expect(measurement.viewportTimelineSummary.latestTextDelayAfterContentVisuallyVisibleMs)
        .toBeLessThanOrEqual(LONG_SESSION_MAX_LATEST_TEXT_DELAY_AFTER_VISIBLE_MS);
    }
    expect(measurement.viewportTimelineSummary.preLatestTextVisibleBlankWithoutPlaceholderSampleCount).toBe(0);
    expect(measurement.viewportTimelineSummary.preLatestTextVisibleUncoveredAfterIntentSampleCount).toBe(0);
    expect(measurement.viewportTimelineSummary.postLatestTextVisibleBlankSampleCount).toBe(0);
    expect(measurement.viewportTimelineSummary.postLatestTextVisibleCoveredSampleCount).toBe(0);
    expect(measurement.viewportTimelineSummary.postLatestTextVisibleLatestContentMissingSampleCount).toBe(0);
  } else {
    expect(isLongSessionViewportUsable(
      measurement.latestAnswerTextVisibleViewport,
      { requireLatestModelRound: false },
    )).toBe(true);
    expect(isLongSessionLatestTailAnchored(measurement.latestAnswerTextVisibleViewport)).toBe(true);
  }
  expect(getLongSessionPhysicalDistanceFromBottom(measurement.latestUsableViewport))
    .toBeLessThanOrEqual(LONG_SESSION_PHYSICAL_BOTTOM_TOLERANCE_PX);
  const latestAnchorFailures = measurement.events.filter(event =>
    event.phase === 'historical_session_latest_anchor_failed' &&
    traceEventSessionId(event) === measurement.sessionId
  );
  if (latestAnchorFailures.length > 0) {
    throw new Error(
      `Unexpected latest anchor failures: ${JSON.stringify(latestAnchorFailures.slice(-5))}`,
    );
  }
  if (options.expectNoHistoryLoadingAfterClick === true) {
    expect(measurement.visualStateSummary.firstLoadingLayerAtMs).toBeNull();
    expect(measurement.visualStateSummary.lastLoadingLayerAtMs).toBeNull();
    expect(measurement.visualStateSummary.loadingLayerToggleCount).toBe(0);
    expect(measurement.visualStateSummary.overlayCountToggleCount).toBe(0);
    expect(measurement.visualStateSummary.placeholderCountToggleCount).toBe(0);
    expect(measurement.visualStateSummary.postFirstVisibleItemLoadingEventCount).toBe(0);
    if (requireLatestModelRound) {
      expect(measurement.visualStateSummary.postLatestTextVisibleLoadingEventCount).toBe(0);
      expect(measurement.visualStateSummary.postLatestTextVisibleLoadingSurfacePointEventCount).toBe(0);
      expect(measurement.visualStateSummary.postLatestTextVisibleBlankSurfacePointEventCount).toBe(0);
      expect(measurement.visualStateSummary.postLatestTextVisibleTransparentSurfacePointEventCount).toBe(0);
      if (!isLongSessionResizeInteraction(measurement.postVisibleInteraction)) {
        expect(measurement.visualStateSummary.postLatestTextVisibleScrollJumpCount).toBe(0);
        expect(measurement.visualStateSummary.postLatestTextVisibleVirtualItemElementChangeCount).toBe(0);
        expect(measurement.visualStateSummary.postLatestTextVisibleLayoutShiftScore).toBeLessThanOrEqual(0.005);
      }
    }
    expect(measurement.visualStateSummary.openIntentBlankSurfacePointEventCount).toBe(0);
    expect(measurement.visualStateSummary.openIntentBlankSurfaceHoldCount).toBe(0);
    expect(measurement.visualStateSummary.postOpenIntentNonTargetContentEventCount).toBe(0);
    expect(measurement.visualStateSummary.postOpenIntentNonTargetContentHoldCount).toBe(0);
    expect(measurement.visualStateSummary.historyInitialPreviewActivationAfterActiveSessionCount).toBe(0);
    expect(measurement.visualStateSummary.historyInitialPreviewVisibleAtEnd).toBe(false);
    expect(measurement.visualStateSummary.loadingTransitions).toHaveLength(0);
    if (measurement.postVisibleInteraction === 'first-scroll') {
      expect(measurement.visualStateSummary.firstUserInteractionAtMs).not.toBeNull();
      expect(measurement.visualStateSummary.postUserInteractionScrollJumpCount).toBe(0);
      expect(measurement.visualStateSummary.postUserInteractionScrollerCollapseCount).toBe(0);
      expect(measurement.visualStateSummary.postUserInteractionBlankSurfacePointEventCount).toBe(0);
      expect(measurement.visualStateSummary.postUserInteractionScrollTransitions).toHaveLength(0);
    }
    if (measurement.postVisibleInteraction === 'scroll-down') {
      expect(measurement.postVisibleInteractionResult).not.toBeNull();
      expect(
        (measurement.postVisibleInteractionResult?.afterScrollTop ?? 0) -
        (measurement.postVisibleInteractionResult?.beforeScrollTop ?? 0),
      ).toBeLessThanOrEqual(LONG_SESSION_PHYSICAL_BOTTOM_TOLERANCE_PX);
      expect(measurement.visualStateSummary.postUserInteractionScrollJumpCount).toBe(0);
      expect(measurement.visualStateSummary.postUserInteractionScrollerCollapseCount).toBe(0);
      expect(measurement.visualStateSummary.postUserInteractionBlankSurfacePointEventCount).toBe(0);
      expect(measurement.visualStateSummary.postUserInteractionScrollTransitions).toHaveLength(0);
    }
    if (isLongSessionResizeInteraction(measurement.postVisibleInteraction)) {
      expect(measurement.postVisibleInteractionResult).not.toBeNull();
      if (measurement.postVisibleInteraction === 'resize-window-width') {
        expect(measurement.postVisibleInteractionResult?.beforeWindowRect?.width).toBeGreaterThan(
          measurement.postVisibleInteractionResult?.afterWindowRect?.width ?? 0,
        );
        expect(Math.abs(
          (measurement.postVisibleInteractionResult?.beforeWindowRect?.height ?? 0) -
          (measurement.postVisibleInteractionResult?.afterWindowRect?.height ?? 0),
        )).toBeLessThanOrEqual(2);
      } else {
        expect(measurement.postVisibleInteractionResult?.beforeWindowRect?.height).toBeGreaterThan(
          measurement.postVisibleInteractionResult?.afterWindowRect?.height ?? 0,
        );
        expect(measurement.postVisibleInteractionResult?.beforeClientHeight).toBeGreaterThan(
          measurement.postVisibleInteractionResult?.afterClientHeight ?? 0,
        );
      }
      expect(
        Math.max(
          0,
          (measurement.postVisibleInteractionResult?.maxScrollTop ?? 0) -
            (measurement.postVisibleInteractionResult?.afterScrollTop ?? 0),
        ),
      ).toBeLessThanOrEqual(LONG_SESSION_PHYSICAL_BOTTOM_TOLERANCE_PX);
      expect(getLongSessionPhysicalDistanceFromBottom(measurement.viewport))
        .toBeLessThanOrEqual(LONG_SESSION_PHYSICAL_BOTTOM_TOLERANCE_PX);
      const resizeTransitions = measurement.visualStateSummary.scrollTransitions.filter(transition =>
        measurement.visualStateSummary.firstUserInteractionAtMs !== null &&
        transition.sinceClickMs > measurement.visualStateSummary.firstUserInteractionAtMs
      );
      for (const transition of resizeTransitions) {
        const toDistanceFromBottom = getLongSessionScrollTransitionDistanceFromBottom(transition, 'to');
        if (toDistanceFromBottom === null || toDistanceFromBottom <= LONG_SESSION_PHYSICAL_BOTTOM_TOLERANCE_PX) {
          continue;
        }
        const settledTransition = resizeTransitions.find(candidate =>
          candidate.sinceClickMs > transition.sinceClickMs &&
          candidate.sinceClickMs - transition.sinceClickMs <= LONG_SESSION_RESIZE_BOTTOM_SETTLE_MAX_MS &&
          (getLongSessionScrollTransitionDistanceFromBottom(candidate, 'to') ?? Number.POSITIVE_INFINITY) <=
            LONG_SESSION_PHYSICAL_BOTTOM_TOLERANCE_PX
        );
        expect(settledTransition).toBeDefined();
      }
      expect(measurement.visualStateSummary.postUserInteractionBlankSurfacePointEventCount).toBe(0);
    }
  }
  if (measurement.fixtureScenario === 'mixed-visible') {
    expect(measurement.latestVisibleViewport.visibleModelRoundCount).toBeGreaterThan(0);
  }
  expect(isLongSessionInputAnchoredNearBottom(measurement.latestVisibleViewport)).toBe(true);
  expect(isLongSessionViewportUsable(measurement.viewport, { requireLatestModelRound })).toBe(true);
  expect(isLongSessionInputAnchoredNearBottom(measurement.viewport)).toBe(true);
  if (measurement.postVisibleInteraction !== 'first-scroll') {
    expect(isLongSessionLatestTailAnchored(measurement.viewport)).toBe(true);
    expect(getLongSessionPhysicalDistanceFromBottom(measurement.viewport))
      .toBeLessThanOrEqual(LONG_SESSION_PHYSICAL_BOTTOM_TOLERANCE_PX);
  }
  if (
    maxLatestFrameMs !== undefined &&
    measurement.sessionOpen.latestFrameSinceHydrateMs !== undefined
  ) {
    expect(measurement.sessionOpen.latestFrameSinceHydrateMs).toBeLessThanOrEqual(maxLatestFrameMs);
  }
}

function expectLongSessionSendAfterOpenStable(
  measurement: LongSessionSendAfterOpenMeasurement,
): void {
  expect(measurement.beforeState.activeSessionId).toBe(measurement.sessionId);
  expect(measurement.afterSendState.activeSessionId).toBe(measurement.sessionId);
  expect(measurement.finalState.activeSessionId).toBe(measurement.sessionId);
  expect(measurement.beforeState.dialogTurnCount).toBeGreaterThan(0);
  expect(measurement.afterSendState.dialogTurnCount).toBeGreaterThan(measurement.beforeState.dialogTurnCount);
  expect(measurement.afterSendState.latestTurnId).not.toBe(measurement.beforeState.latestTurnId);
  expect(measurement.afterSendState.virtualItemCount).toBeGreaterThan(measurement.beforeState.virtualItemCount);
  expect(measurement.viewport.hasScroller).toBe(true);
  expect(measurement.viewport.visibleItemCount).toBeGreaterThan(0);
  expect(measurement.viewport.visibleTextLength).toBeGreaterThan(0);
  expect(measurement.visualStateSummary.firstLoadingLayerAtMs).toBeNull();
  expect(measurement.visualStateSummary.loadingLayerToggleCount).toBe(0);
  expect(measurement.visualStateSummary.overlayCountToggleCount).toBe(0);
  expect(measurement.visualStateSummary.placeholderCountToggleCount).toBe(0);
  expect(measurement.visualStateSummary.postUserInteractionBlankSurfacePointEventCount).toBe(0);
  expect(measurement.visualStateSummary.postLatestTextVisibleBlankSurfacePointEventCount).toBe(0);
  expect(measurement.visualStateSummary.postLatestTextVisibleLoadingSurfacePointEventCount).toBe(0);
  expect(measurement.visualStateSummary.postLatestTextVisibleTransparentSurfacePointEventCount).toBe(0);
  expect(measurement.visualStateSummary.loadingTransitions).toHaveLength(0);
}

describe('Performance telemetry', () => {
  const startupPage = new StartupPage();

  before(async () => {
    await waitForTracePhaseCount('interactive_shell_ready', 1, 30000);
    await assertReleaseFastPerfRuntime();
  });

  it('collects startup timing from the current build', async () => {
    const runtime = await assertReleaseFastPerfRuntime();
    const snapshot = await readStartupTraceSnapshot();
    const startup = summarizeStartup(snapshot);
    const breakdown = summarizeStartupBreakdown(snapshot);
    const apiSegments = summarizeApiCommandSegments(snapshot);
    const resourceTiming = await readStartupResourceTimingSummary(startup.interactiveShellReadyMs);
    const maxInteractiveMs = numericEnv('BITFUN_E2E_PERF_MAX_INTERACTIVE_MS');

    console.log('[Perf] startup', JSON.stringify({
      appMode: process.env.BITFUN_E2E_APP_MODE ?? 'auto',
      ...runtime,
      traceId: snapshot.traceId,
      startup,
      breakdown,
      api: snapshot.api,
      native: snapshot.native,
    }));
    await writeReport('startup', {
      appMode: process.env.BITFUN_E2E_APP_MODE ?? 'auto',
      ...runtime,
      traceId: snapshot.traceId,
      startup,
      breakdown,
      resourceTiming,
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
    await ensurePerformanceWorkspace(startupPage);

    const sessionId = process.env.BITFUN_E2E_PERF_SESSION_ID || DEFAULT_PERF_SESSION_ID;
    const expectedLatestTurnId = await readExpectedLatestTurnId(sessionId);
    const postVisibleInteraction = readPostVisibleInteractionOption({});
    const requireFrameTrace = postVisibleInteraction === null;
    const measurement = await collectLongSessionOpenMeasurement(
      sessionId,
      expectedLatestTurnId,
      { requireFrameTrace },
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
    expectLongSessionMeasurementUsable(measurement, maxLatestFrameMs, {
      requireFrameTrace,
      expectNoHistoryLoadingAfterClick: true,
    });
  });

  it('collects warm-reopen timing for a generated long session', async function () {
    await ensurePerformanceWorkspace(startupPage);

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
    expectLongSessionMeasurementUsable(measurement, maxLatestFrameMs, {
      requireFrameTrace: false,
      expectNoHistoryLoadingAfterClick: true,
    });
  });

  it('keeps a generated long session visible after sending a new message', async function () {
    await ensurePerformanceWorkspace(startupPage);

    const sessionId = process.env.BITFUN_E2E_PERF_SESSION_ID || DEFAULT_PERF_SESSION_ID;
    const expectedLatestTurnId = await readExpectedLatestTurnId(sessionId);
    if (!expectedLatestTurnId) {
      console.log(`[Perf] Session ${sessionId} not found; generate it before running this spec.`);
      this.skip();
      return;
    }

    const opened = await ensureLongSessionOpenForSend(sessionId, expectedLatestTurnId);
    if (!opened) {
      console.log(`[Perf] Session ${sessionId} not reachable from the session navigation.`);
      this.skip();
      return;
    }

    const measurement = await collectLongSessionSendAfterOpenMeasurement(
      sessionId,
      expectedLatestTurnId,
    );

    console.log('[Perf] long-session-send-after-open', JSON.stringify({
      appMode: measurement.appMode,
      sessionId,
      fixtureScenario: measurement.fixtureScenario,
      beforeState: measurement.beforeState,
      afterSendState: measurement.afterSendState,
      finalState: measurement.finalState,
      visualStateSummary: {
        firstLoadingLayerAtMs: measurement.visualStateSummary.firstLoadingLayerAtMs,
        loadingLayerToggleCount: measurement.visualStateSummary.loadingLayerToggleCount,
        postUserInteractionBlankSurfacePointEventCount:
          measurement.visualStateSummary.postUserInteractionBlankSurfacePointEventCount,
        postLatestTextVisibleBlankSurfacePointEventCount:
          measurement.visualStateSummary.postLatestTextVisibleBlankSurfacePointEventCount,
        virtualItemElementChangeCount:
          measurement.visualStateSummary.postLatestTextVisibleVirtualItemElementChangeCount,
      },
    }));

    await writeReport('long-session-send-after-open', measurement);
    expectLongSessionSendAfterOpenStable(measurement);
  });

  it('collects rapid-switch timing across generated long sessions', async function () {
    await ensurePerformanceWorkspace(startupPage);

    const requestedSessionIds = readRapidSwitchSessionIds();
    const sessionIds = requestedSessionIds;
    if (sessionIds.length < 3) {
      this.skip();
      return;
    }

    const targetSessionId = sessionIds[sessionIds.length - 1];
    const activeSessionIdAtStart = await ensureRapidSwitchTargetStartsInactive(
      targetSessionId,
      requestedSessionIds,
    );
    const expectedLatestTurnId = await readExpectedLatestTurnId(targetSessionId);
    if (!expectedLatestTurnId) {
      console.log(
        `[Perf] Rapid switch target session ${targetSessionId} not found; generate rapid fixtures before running this check.`,
      );
      this.skip();
      return;
    }

    const measurement = await collectRapidLongSessionSwitchMeasurement(
      requestedSessionIds,
      sessionIds,
      activeSessionIdAtStart,
      expectedLatestTurnId,
    );
    if (!measurement) {
      console.log(`[Perf] Rapid switch sessions not found; requested=${sessionIds.join(',')}`);
      this.skip();
      return;
    }

    console.log('[Perf] long-session-rapid-switch', JSON.stringify({
      appMode: measurement.appMode,
      requestedSessionIds,
      sessionIds,
      activeSessionIdAtStart,
      targetSessionId,
      clickToTargetLatestVisibleMs: measurement.clickToTargetLatestVisibleMs,
      clickToTargetLatestUsableMs: measurement.clickToTargetLatestUsableMs,
      rapidSwitchBreakdown: {
        firstClickToTargetClickMs: measurement.rapidSwitchBreakdown.firstClickToTargetClickMs,
        target: {
          clickSinceFirstClickMs: measurement.rapidSwitchBreakdown.target.clickSinceFirstClickMs,
          findDurationMs: measurement.rapidSwitchBreakdown.target.findDurationMs,
          clickDurationMs: measurement.rapidSwitchBreakdown.target.clickDurationMs,
          pauseBeforeTargetMs: measurement.rapidSwitchBreakdown.target.pauseBeforeTargetMs,
          clickActionCompletedToLatestVisibleMs:
            measurement.rapidSwitchBreakdown.target.clickActionCompletedToLatestVisibleMs,
          clickActionCompletedToLatestUsableMs:
            measurement.rapidSwitchBreakdown.target.clickActionCompletedToLatestUsableMs,
          timelineClickToLatestContentVisibleMs:
            measurement.rapidSwitchBreakdown.target.timelineClickToLatestContentVisibleMs,
          timelineClickToLatestTextVisibleMs:
            measurement.rapidSwitchBreakdown.target.timelineClickToLatestTextVisibleMs,
          timelineClickActionCompletedToLatestTextVisibleMs:
            measurement.rapidSwitchBreakdown.target.timelineClickActionCompletedToLatestTextVisibleMs,
          clickToLatestVisibleMs: measurement.rapidSwitchBreakdown.target.clickToLatestVisibleMs,
          latestVisibleToUsableMs: measurement.rapidSwitchBreakdown.target.latestVisibleToUsableMs,
          clickToLatestUsableMs: measurement.rapidSwitchBreakdown.target.clickToLatestUsableMs,
        },
        sessions: measurement.rapidSwitchBreakdown.sessions.map(session => ({
          sessionId: session.sessionId,
          sinceFirstClickMs: session.sinceFirstClickMs,
          eventCount: session.eventCount,
          clickToHydrateStartMs: session.sessionOpen.clickToHydrateStartMs,
          clickToLatestFrameMs: session.sessionOpen.clickToLatestFrameMs,
          clickToHydrateEndMs: session.sessionOpen.clickToHydrateEndMs,
          restoreDurationMs: session.sessionOpen.restoreDurationMs,
          stateCommitDurationMs: session.sessionOpen.stateCommitDurationMs,
          latestFrameSinceHydrateMs: session.sessionOpen.latestFrameSinceHydrateMs,
        })),
      },
      visualStateSummary: {
        postLatestTextVisibleLoadingEventCount:
          measurement.visualStateSummary.postLatestTextVisibleLoadingEventCount,
        postLatestTextVisibleBlankSurfacePointEventCount:
          measurement.visualStateSummary.postLatestTextVisibleBlankSurfacePointEventCount,
        postLatestTextVisibleScrollJumpCount:
          measurement.visualStateSummary.postLatestTextVisibleScrollJumpCount,
        postOpenIntentNonTargetContentHoldCount:
          measurement.visualStateSummary.postOpenIntentNonTargetContentHoldCount,
      },
    }));

    await writeReport('long-session-rapid-switch', measurement);

    expectRapidSwitchMeasurementUsesFreshTargetRestore(measurement);
    expect(measurement.activeSessionIdAtEnd).toBe(targetSessionId);
    expect(isLongSessionViewportUsable(measurement.viewport)).toBe(true);
    expect(measurement.visualStateSummary.postLatestTextVisibleLoadingEventCount).toBe(0);
    expect(measurement.visualStateSummary.postLatestTextVisibleBlankSurfacePointEventCount).toBe(0);
    expect(measurement.visualStateSummary.postLatestTextVisibleScrollJumpCount).toBe(0);
    expect(measurement.visualStateSummary.postOpenIntentNonTargetContentHoldCount).toBe(0);
  });
});
