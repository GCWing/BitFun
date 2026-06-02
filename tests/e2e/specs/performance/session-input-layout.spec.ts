import { $, browser, expect } from '@wdio/globals';
import { saveStepScreenshot } from '../../helpers/screenshot-utils';
import { readStartupTraceSnapshot, waitForTracePhaseCount } from '../../helpers/performance-trace';

const DEFAULT_PERF_SESSION_ID = 'perf-long-session-000';

function countPhase(snapshot: Awaited<ReturnType<typeof readStartupTraceSnapshot>>, phase: string): number {
  return snapshot.phases.events.filter(event => event.phase === phase).length;
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

async function readInputLayout() {
  return browser.execute(() => {
    const pane = document.querySelector('.bitfun-chat-pane__content');
    const input = document.querySelector('[data-testid="chat-input-container"]');
    const dropZone = document.querySelector('.bitfun-chat-input-drop-zone');
    const welcomePanel = document.querySelector('.welcome-panel');
    const historyPlaceholder = document.querySelector('.history-session-placeholder');

    const rectOf = (element: Element | null) => {
      if (!element) return null;
      const rect = element.getBoundingClientRect();
      return {
        top: rect.top,
        bottom: rect.bottom,
        height: rect.height,
        centerY: rect.top + rect.height / 2,
      };
    };

    const paneRect = rectOf(pane);
    const inputRect = rectOf(input);
    const dropZoneRect = rectOf(dropZone);
    return {
      pane: paneRect,
      input: inputRect,
      dropZone: dropZoneRect,
      hasWelcomePanel: Boolean(welcomePanel),
      hasHistoryPlaceholder: Boolean(historyPlaceholder),
      relativeInputCenterY:
        paneRect && inputRect ? (inputRect.centerY - paneRect.top) / paneRect.height : null,
      relativeDropZoneCenterY:
        paneRect && dropZoneRect ? (dropZoneRect.centerY - paneRect.top) / paneRect.height : null,
    };
  });
}

describe('Session input layout', () => {
  it('keeps the chat input anchored near the bottom while opening a saved session', async function () {
    const sessionId = process.env.BITFUN_E2E_PERF_SESSION_ID || DEFAULT_PERF_SESSION_ID;
    const item = await findSessionItem(sessionId);
    if (!item) {
      console.log(`[Layout] Session ${sessionId} not found; generate it before running this spec.`);
      this.skip();
      return;
    }

    const beforeClickSnapshot = await readStartupTraceSnapshot();
    const frameCountBefore = countPhase(beforeClickSnapshot, 'historical_session_after_state_commit_frame');
    await item.click();
    await browser.pause(50);

    const immediateLayout = await readInputLayout();
    console.log('[Layout] immediate', JSON.stringify(immediateLayout));
    await saveStepScreenshot('session-input-layout-immediate');

    await waitForTracePhaseCount('historical_session_after_state_commit_frame', frameCountBefore + 1, 20000);
    await browser.pause(50);

    const hydratedLayout = await readInputLayout();
    console.log('[Layout] hydrated', JSON.stringify(hydratedLayout));
    await saveStepScreenshot('session-input-layout-hydrated');

    expect(immediateLayout.input).toBeTruthy();
    expect(immediateLayout.relativeInputCenterY).toBeGreaterThan(0.65);
    expect(hydratedLayout.input).toBeTruthy();
    expect(hydratedLayout.relativeInputCenterY).toBeGreaterThan(0.65);
  });
});
