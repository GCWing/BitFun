/**
 * Release-compatible regression coverage for header turn navigation.
 *
 * This spec intentionally avoids Vite-only /src imports so it can run against
 * the bundled release-fast desktop app with persisted long-session fixtures.
 */
import { $, browser, expect } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';
import { saveFailureScreenshot } from '../helpers/screenshot-utils';

const SESSION_NAV_ITEM_SELECTOR =
  '[data-testid="session-nav-item"], [data-testid="nav-session-item"]';
const SESSION_NAV_TOGGLE_SELECTOR =
  '[data-testid="session-nav-show-more"], [data-testid="nav-session-list-toggle"]';
const DEFAULT_SESSION_ID = 'release-turn-nav-000';
const DEFAULT_TARGET_TURN_INDEX = 2;

type TurnViewportMetrics = {
  rootExists: boolean;
  scrollerExists: boolean;
  targetExists: boolean;
  targetVisible: boolean;
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  maxScrollTop: number;
  targetTop: number | null;
  targetBottom: number | null;
  scrollerTop: number | null;
  scrollerBottom: number | null;
  deltaToPinnedTop: number | null;
  distanceFromBottom: number | null;
  visibleTurnIds: string[];
  activeTurnListText: string | null;
};

type SessionShellState = {
  historyState: string | null;
  contextRestoreState: string | null;
  isPartial: boolean;
  dialogTurnCount: number;
  virtualItemCount: number;
  hasPendingHistoryCompletion: boolean;
  hasDeferredHistoryProjection: boolean;
  latestTurnId: string | null;
  turnListOpen: boolean;
  turnListTexts: string[];
};

function escapeCssAttributeValue(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
}

async function findSessionItem(sessionId: string): Promise<WebdriverIO.Element | null> {
  const targetSelector =
    `[data-testid="session-nav-item"][data-session-id="${escapeCssAttributeValue(sessionId)}"], ` +
    `[data-testid="nav-session-item"][data-session-id="${escapeCssAttributeValue(sessionId)}"]`;

  const readVisibleSessionIds = async (): Promise<string[]> =>
    browser.execute((selector) =>
      Array.from(document.querySelectorAll(selector))
        .map(element => element.getAttribute('data-session-id') || '')
        .filter(Boolean),
    SESSION_NAV_ITEM_SELECTOR);

  const findTarget = async (): Promise<WebdriverIO.Element | null> => {
    const item = await $(targetSelector);
    return await item.isExisting() ? item : null;
  };

  const findExpandableToggles = async (): Promise<WebdriverIO.Element[]> => {
    const toggles = await browser.$$(SESSION_NAV_TOGGLE_SELECTOR);
    const expandable: WebdriverIO.Element[] = [];
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

  console.log('[ReleaseTurnNav] visible sessions while locating target', JSON.stringify({
    target: sessionId,
    visibleSessionIds: lastVisibleSessionIds.slice(0, 40),
    visibleSessionCount: lastVisibleSessionIds.length,
  }));
  return null;
}

async function isSessionItemActive(item: WebdriverIO.Element): Promise<boolean> {
  const className = await item.getAttribute('class') ?? '';
  return className.split(/\s+/).includes('is-active');
}

async function readTurnViewportMetrics(targetTurnId: string): Promise<TurnViewportMetrics> {
  return browser.execute((turnId) => {
    const root = document.querySelector<HTMLElement>(
      '.modern-flowchat-container__messages .virtual-message-list',
    );
    const scroller = root?.querySelector<HTMLElement>(
      '[data-virtuoso-scroller="true"], [data-virtuoso-scroller], .virtual-message-list__static-scroller',
    ) ?? null;
    const wrappers = Array.from(root?.querySelectorAll<HTMLElement>(
      '.virtual-item-wrapper[data-turn-id]',
    ) ?? []);
    const target = wrappers.find(element =>
      element.dataset.turnId === turnId && element.dataset.itemType === 'user-message'
    ) ?? null;
    const scrollerRect = scroller?.getBoundingClientRect() ?? null;
    const targetRect = target?.getBoundingClientRect() ?? null;
    const visibleTurnIds = scrollerRect
      ? wrappers
        .filter(element => {
          const rect = element.getBoundingClientRect();
          return rect.bottom > scrollerRect.top && rect.top < scrollerRect.bottom;
        })
        .map(element => element.dataset.turnId || '')
        .filter(Boolean)
      : [];
    const targetVisible = Boolean(
      scrollerRect &&
      targetRect &&
      targetRect.bottom > scrollerRect.top &&
      targetRect.top < scrollerRect.bottom
    );
    const activeTurnListItem = document.querySelector<HTMLElement>(
      '.flowchat-header__turn-list-item--active',
    );

    return {
      rootExists: Boolean(root),
      scrollerExists: Boolean(scroller),
      targetExists: Boolean(target),
      targetVisible,
      scrollTop: scroller?.scrollTop ?? 0,
      scrollHeight: scroller?.scrollHeight ?? 0,
      clientHeight: scroller?.clientHeight ?? 0,
      maxScrollTop: scroller ? Math.max(0, scroller.scrollHeight - scroller.clientHeight) : 0,
      targetTop: targetRect?.top ?? null,
      targetBottom: targetRect?.bottom ?? null,
      scrollerTop: scrollerRect?.top ?? null,
      scrollerBottom: scrollerRect?.bottom ?? null,
      deltaToPinnedTop: scrollerRect && targetRect ? targetRect.top - scrollerRect.top : null,
      distanceFromBottom: scrollerRect && targetRect ? scrollerRect.bottom - targetRect.bottom : null,
      visibleTurnIds: Array.from(new Set(visibleTurnIds)),
      activeTurnListText: activeTurnListItem?.textContent?.trim() ?? null,
    };
  }, targetTurnId);
}

async function readSessionShellState(): Promise<SessionShellState> {
  return browser.execute(() => {
    const messages = document.querySelector<HTMLElement>(
      '.modern-flowchat-container__messages',
    );
    const turnListItems = Array.from(document.querySelectorAll<HTMLElement>(
      '.flowchat-header__turn-list-item',
    ));
    return {
      historyState: messages?.dataset.historyState ?? null,
      contextRestoreState: messages?.dataset.contextRestoreState ?? null,
      isPartial: messages?.dataset.isPartial === 'true',
      dialogTurnCount: Number(messages?.dataset.dialogTurnCount ?? 0),
      virtualItemCount: Number(messages?.dataset.virtualItemCount ?? 0),
      hasPendingHistoryCompletion: messages?.dataset.hasPendingHistoryCompletion === 'true',
      hasDeferredHistoryProjection: messages?.dataset.hasDeferredHistoryProjection === 'true',
      latestTurnId: messages?.dataset.latestTurnId || null,
      turnListOpen: turnListItems.length > 0,
      turnListTexts: turnListItems.map(item => item.textContent?.trim() ?? ''),
    };
  });
}

async function waitForSessionHydrated(): Promise<void> {
  await browser.waitUntil(async () => {
    const turnListButton = await $('[data-testid="flowchat-header-turn-list"]');
    if (!(await turnListButton.isExisting()) || !(await turnListButton.isEnabled())) {
      return false;
    }
    const state = await browser.execute(() => {
      const root = document.querySelector<HTMLElement>(
        '.modern-flowchat-container__messages .virtual-message-list',
      );
      const scroller = root?.querySelector<HTMLElement>(
        '[data-virtuoso-scroller="true"], [data-virtuoso-scroller], .virtual-message-list__static-scroller',
      ) ?? null;
      const items = root?.querySelectorAll('.virtual-item-wrapper[data-turn-id]') ?? [];
      return {
        hasRoot: Boolean(root),
        hasScroller: Boolean(scroller),
        itemCount: items.length,
      };
    });
    return state.hasRoot && state.hasScroller && state.itemCount > 0;
  }, {
    timeout: 30000,
    interval: 200,
    timeoutMsg: '[ReleaseTurnNav] generated session did not hydrate message list',
  });
}

async function openHeaderTurnList(): Promise<void> {
  const existingItems = await $$('.flowchat-header__turn-list-item');
  if (existingItems.length > 0) {
    return;
  }

  const turnListButton = await $('[data-testid="flowchat-header-turn-list"]');
  await turnListButton.waitForClickable({ timeout: 10000 });
  await turnListButton.click();
  await browser.waitUntil(async () => {
    const items = await $$('.flowchat-header__turn-list-item');
    return items.length > 0;
  }, {
    timeout: 3000,
    interval: 100,
    timeoutMsg: '[ReleaseTurnNav] turn list did not open',
  });
}

async function closeHeaderTurnList(): Promise<void> {
  const items = await $$('.flowchat-header__turn-list-item');
  if (items.length === 0) {
    return;
  }

  const turnListButton = await $('[data-testid="flowchat-header-turn-list"]');
  await turnListButton.click();
  await browser.waitUntil(async () => {
    const currentItems = await $$('.flowchat-header__turn-list-item');
    return currentItems.length === 0;
  }, { timeout: 3000, interval: 100 }).catch(() => undefined);
}

async function readHeaderTurnListTexts(): Promise<string[]> {
  await openHeaderTurnList();
  return browser.execute(() =>
    Array.from(document.querySelectorAll<HTMLElement>('.flowchat-header__turn-list-item'))
      .map(item => item.textContent?.trim() ?? ''),
  );
}

async function scrollToPreviousHistoryBoundary(): Promise<void> {
  await browser.execute(() => {
    const scroller = document.querySelector<HTMLElement>(
      '.modern-flowchat-container__messages .virtual-message-list [data-virtuoso-scroller="true"], ' +
      '.modern-flowchat-container__messages .virtual-message-list [data-virtuoso-scroller], ' +
      '.modern-flowchat-container__messages .virtual-message-list .virtual-message-list__static-scroller',
    );
    if (!scroller) {
      return;
    }
    scroller.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      deltaY: -1200,
      deltaMode: 0,
    }));
    scroller.scrollTop = 0;
    scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
  });
}

async function revealHistoryUntilTurnListContains(targetTitle: string): Promise<{
  attempts: number;
  finalState: SessionShellState;
}> {
  let state = await readSessionShellState();
  for (let attempt = 0; attempt < 8; attempt += 1) {
    const texts = await readHeaderTurnListTexts();
    state = {
      ...await readSessionShellState(),
      turnListOpen: true,
      turnListTexts: texts,
    };
    if (texts.some(text => text.includes(targetTitle))) {
      return { attempts: attempt, finalState: state };
    }

    await closeHeaderTurnList();
    if (!state.isPartial && !state.hasPendingHistoryCompletion && !state.hasDeferredHistoryProjection) {
      break;
    }

    const beforeCount = state.dialogTurnCount;
    await scrollToPreviousHistoryBoundary();
    await browser.waitUntil(async () => {
      const next = await readSessionShellState();
      return (
        next.dialogTurnCount > beforeCount ||
        next.isPartial === false ||
        next.hasDeferredHistoryProjection !== state.hasDeferredHistoryProjection ||
        next.hasPendingHistoryCompletion !== state.hasPendingHistoryCompletion
      );
    }, { timeout: 6000, interval: 150 }).catch(() => undefined);
    await browser.pause(500);
  }

  await openHeaderTurnList();
  state = await readSessionShellState();
  throw new Error(`[ReleaseTurnNav] target turn title not available in header list: ${JSON.stringify({
    targetTitle,
    state,
  })}`);
}

async function scrollToLatest(targetTurnId: string): Promise<TurnViewportMetrics> {
  await browser.execute(() => {
    const scroller = document.querySelector<HTMLElement>(
      '.modern-flowchat-container__messages .virtual-message-list [data-virtuoso-scroller="true"], ' +
      '.modern-flowchat-container__messages .virtual-message-list [data-virtuoso-scroller], ' +
      '.modern-flowchat-container__messages .virtual-message-list .virtual-message-list__static-scroller',
    );
    if (!scroller) {
      return;
    }
    scroller.scrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
  });
  await browser.pause(800);
  return readTurnViewportMetrics(targetTurnId);
}

async function clickHeaderTurnListItemByTitle(targetTitle: string): Promise<{
  itemCount: number;
  itemTexts: string[];
}> {
  await openHeaderTurnList();
  const items = await $$('.flowchat-header__turn-list-item');
  const itemTexts: string[] = [];
  let targetIndex = -1;
  for (let index = 0; index < items.length; index += 1) {
    const text = await items[index].getText();
    itemTexts.push(text);
    if (targetIndex < 0 && text.includes(targetTitle)) {
      targetIndex = index;
    }
  }
  if (targetIndex < 0) {
    throw new Error(`[ReleaseTurnNav] turn list item not found: ${JSON.stringify({
      targetTitle,
      itemTexts,
    })}`);
  }
  await items[targetIndex].click();
  return { itemCount: items.length, itemTexts };
}

describe('Release long-session turn navigation', () => {
  let hasWorkspace = false;

  before(async function () {
    const workspacePath = process.env.E2E_TEST_WORKSPACE;
    if (!workspacePath) {
      console.log('[ReleaseTurnNav] E2E_TEST_WORKSPACE is missing; skipping release fixture spec.');
      return;
    }
    hasWorkspace = await openWorkspace(workspacePath, { requireWorkspaceLabel: true });
  });

  it('moves the real message viewport when selecting an older turn from the header list (#1281)', async function () {
    if (!hasWorkspace) {
      this.skip();
      return;
    }

    const sessionId = process.env.BITFUN_E2E_TURN_NAV_SESSION_ID || DEFAULT_SESSION_ID;
    const targetTurnIndex = Number(
      process.env.BITFUN_E2E_TURN_NAV_TARGET_INDEX || DEFAULT_TARGET_TURN_INDEX,
    );
    const targetTurnId = `${sessionId}-turn-${String(targetTurnIndex).padStart(4, '0')}`;
    const targetTitle = `Synthetic user turn ${targetTurnIndex}`;

    const item = await findSessionItem(sessionId);
    if (!item) {
      throw new Error(`[ReleaseTurnNav] generated session not found: ${sessionId}`);
    }
    if (!(await isSessionItemActive(item))) {
      await item.click();
    }
    await waitForSessionHydrated();

    const before = await scrollToLatest(targetTurnId);
    expect(before.scrollerExists).toBe(true);
    expect(before.maxScrollTop).toBeGreaterThan(300);

    const revealState = await revealHistoryUntilTurnListContains(targetTitle);
    const clickResult = await clickHeaderTurnListItemByTitle(targetTitle);
    expect(clickResult.itemCount).toBeGreaterThan(0);
    await browser.waitUntil(async () => {
      return browser.execute(() =>
        document.querySelectorAll('.flowchat-header__turn-list-item').length === 0,
      );
    }, {
      timeout: 500,
      interval: 50,
      timeoutMsg: '[ReleaseTurnNav] turn list did not close promptly after accepted selection',
    });

    let lastMetrics = await readTurnViewportMetrics(targetTurnId);
    await browser.waitUntil(async () => {
      const metrics = await readTurnViewportMetrics(targetTurnId);
      lastMetrics = metrics;
      return (
        metrics.targetVisible &&
        metrics.deltaToPinnedTop !== null &&
        Math.abs(metrics.deltaToPinnedTop) <= 100
      );
    }, {
      timeout: 8000,
      interval: 150,
      timeoutMsg: `[ReleaseTurnNav] selected turn did not move near the top of the message viewport: ${JSON.stringify({
        targetTurnId,
        targetTitle,
        revealState,
        clickResult,
        lastMetrics,
        shellState: await readSessionShellState(),
      })}`,
    });

    await browser.pause(600);
    const after = await readTurnViewportMetrics(targetTurnId);
    const diagnostics = { sessionId, targetTurnId, targetTitle, revealState, clickResult, before, after };
    console.log('[ReleaseTurnNav] diagnostics:', JSON.stringify(diagnostics));

    expect(after.targetExists).toBe(true);
    expect(after.targetVisible).toBe(true);
    expect(after.deltaToPinnedTop).not.toBeNull();
    expect(Math.abs(after.deltaToPinnedTop!)).toBeLessThanOrEqual(100);
    expect(after.distanceFromBottom).not.toBeNull();
    expect(after.distanceFromBottom!).toBeGreaterThan(200);
  });

  afterEach(async function () {
    if (this.currentTest?.state === 'failed') {
      await saveFailureScreenshot(`l1-chat-turn-navigation-release-${this.currentTest.title}`);
    }
  });
});
