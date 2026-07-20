/**
 * Diagnostic-only spec for comparing the real local stream inside BitFun's
 * embedded browser. Run with BITFUN_BROWSER_STREAM_URL=http://127.0.0.1:41953/
 */

import { browser, $ } from '@wdio/globals';

const streamUrl = process.env.BITFUN_BROWSER_STREAM_URL || 'http://127.0.0.1:41953/';
const describeRealStream = process.env.BITFUN_BROWSER_STREAM_URL ? describe : describe.skip;

async function fetchClientStatus() {
  const response = await fetch(`${streamUrl.replace(/\/$/, '')}/api/client-status`, {
    cache: 'no-store',
  });
  if (!response.ok) {
    throw new Error(`client-status failed: ${response.status} ${await response.text()}`);
  }
  return response.json() as Promise<Record<string, any>>;
}

async function runLongDrag() {
  const baseUrl = streamUrl.replace(/\/$/, '');
  const send = async (payload: Record<string, unknown>) => {
    const response = await fetch(`${baseUrl}/api/input`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(payload),
    });
    if (!response.ok) {
      throw new Error(`input failed: ${response.status} ${await response.text()}`);
    }
  };

  const x = 1200;
  const startY = 1750;
  const endY = 450;
  await send({ kind: 'touchDown', x, y: startY });
  let previousY = startY;
  for (let index = 1; index <= 180; index += 1) {
    const phase = (index % 90) / 90;
    const nextY = Math.round(index <= 90
      ? startY + (endY - startY) * phase
      : endY + (startY - endY) * phase);
    await send({
      kind: 'touchMove',
      x1: x,
      y1: previousY,
      x2: x,
      y2: nextY,
      duration: 16,
    });
    previousY = nextY;
    await new Promise((resolve) => setTimeout(resolve, 16));
  }
  await send({ kind: 'touchUp', x, y: previousY });
}

function summarizeStatus(status: Record<string, any>) {
  const stream = status.stream || {};
  const browserTelemetry = status.browser || {};
  const selfTest = status.browserSelfTest || {};
  return {
    streamRecentFps: stream.recentFps,
    streamLastFrameAgoMs: stream.lastFrameAgoMs,
    avccClients: stream.avccClients,
    browserDecodedFps: browserTelemetry.decodedFps,
    frameIntervalAvgMs: browserTelemetry.frameIntervalAvgMs,
    frameIntervalP95Ms: browserTelemetry.frameIntervalP95Ms,
    frameIntervalMaxMs: browserTelemetry.frameIntervalMaxMs,
    jankFrames: browserTelemetry.jankFrames,
    jankRatio: browserTelemetry.jankRatio,
    canvasWidth: browserTelemetry.canvasWidth,
    canvasHeight: browserTelemetry.canvasHeight,
    visibility: browserTelemetry.visibility,
    pageUrl: browserTelemetry.pageUrl,
    inputAckAvgMs: browserTelemetry.inputAckAvgMs,
    inputAckMaxMs: browserTelemetry.inputAckMaxMs,
    selfTestLastResult: selfTest.lastResult,
  };
}

describeRealStream('L1 Built-in browser real stream diagnostic', () => {
  it('opens the real local stream and prints browser telemetry', async () => {
    await browser.execute(async () => {
      const invoke = window.__TAURI__?.core?.invoke;
      if (typeof invoke === 'function') {
        await invoke('plugin:window|maximize', { label: 'main' });
      }
    });
    await browser.pause(500);

    const entry = await $('[data-testid="browser-panel-entry"]');
    await entry.waitForClickable({ timeout: 15000 });
    await entry.click();

    const input = await $('[data-testid="browser-url-input"]');
    await input.waitForDisplayed({ timeout: 15000 });
    await browser.execute((url: string) => {
      const inputElement = document.querySelector<HTMLInputElement>('[data-testid="browser-url-input"]');
      if (!inputElement) throw new Error('Browser URL input not found');

      const valueSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
      valueSetter?.call(inputElement, url);
      inputElement.dispatchEvent(new Event('input', { bubbles: true }));
      inputElement.form?.requestSubmit();
    }, streamUrl);

    await browser.waitUntil(async () => {
      const current = await $('[data-testid="browser-current-url"]');
      if (!(await current.isExisting())) return false;
      return (await current.getText()).includes('127.0.0.1:41953');
    }, {
      timeout: 15000,
      interval: 250,
      timeoutMsg: 'Browser UI did not reflect the real stream URL',
    });

    const webviewLabel = await browser.waitUntil(async () => {
      const label = await browser.execute(() => {
        const host = document.querySelector('[data-webview-label]');
        return host?.getAttribute('data-webview-label') || '';
      });
      return label || false;
    }, {
      timeout: 15000,
      interval: 250,
      timeoutMsg: 'Browser WebView label was not exposed',
    }) as string;

    await browser.waitUntil(async () => {
      return browser.execute(async (label: string, expectedUrl: string) => {
        const invoke = window.__TAURI__?.core?.invoke;
        if (typeof invoke !== 'function') return false;
        const url = await invoke('browser_get_url', { request: { label } });
        return url === expectedUrl;
      }, webviewLabel, streamUrl);
    }, {
      timeout: 15000,
      interval: 250,
      timeoutMsg: 'Native browser WebView did not navigate to the real stream URL',
    });

    await browser.pause(5000);

    for (let index = 0; index < 8; index += 1) {
      const status = await fetchClientStatus();
      console.log(`[real-stream-before-${index}] ${JSON.stringify(summarizeStatus(status))}`);
      await browser.pause(1000);
    }

    if (process.env.BITFUN_BROWSER_LONG_DRAG === '1') {
      await runLongDrag();
    } else {
      await fetch(`${streamUrl.replace(/\/$/, '')}/api/browser/self-test`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ kind: 'drag' }),
      });
    }

    await browser.pause(7000);

    for (let index = 0; index < 8; index += 1) {
      const status = await fetchClientStatus();
      console.log(`[real-stream-after-${index}] ${JSON.stringify(summarizeStatus(status))}`);
      await browser.pause(1000);
    }
  });
});
