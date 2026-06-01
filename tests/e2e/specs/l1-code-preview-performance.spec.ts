import { browser, expect } from '@wdio/globals';

interface CodePreviewStreamingProbeResult {
  renderedLineCount: number;
  startingLineNumber: number;
  containsLine090: boolean;
  containsFinalLine: boolean;
  scrollWrites: number;
  durationMs: number;
  maxFrameMs: number;
  longFrameCount: number;
}

describe('L1 Code preview streaming performance', () => {
  it('bounds streamed preview work and avoids nested auto-scroll writes', async () => {
    await browser.waitUntil(
      async () => browser.execute(() => document.readyState === 'complete'),
      {
        timeout: 30000,
        timeoutMsg: 'Webview document did not finish loading',
      },
    );

    const result = await browser.execute(async () => {
      const module = await import('/src/flow_chat/components/codePreviewPerfHarness.tsx');
      return module.runCodePreviewStreamingPerfProbe({
        totalLines: 160,
        updateCount: 30,
        maxHeight: 88,
        autoScrollToBottom: false,
        charsPerLine: 96,
      });
    }) as CodePreviewStreamingProbeResult;

    console.log('[CodePreviewPerf] Probe result:', JSON.stringify(result));

    expect(result.containsFinalLine).toBe(true);
    expect(result.containsLine090).toBe(false);
    expect(result.startingLineNumber).toBeGreaterThan(140);
    expect(result.renderedLineCount).toBeLessThanOrEqual(12);
    expect(result.scrollWrites).toBe(0);
    expect(result.maxFrameMs).toBeLessThan(500);
  });
});
