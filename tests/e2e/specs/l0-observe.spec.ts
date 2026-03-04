/**
 * L0 observe spec: launch app and keep window open for manual inspection.
 */

import { browser, expect, $ } from '@wdio/globals';

describe('L0 Observe - Keep window open', () => {
  it('launches app and keeps window open for 60 seconds', async () => {
    console.log('='.repeat(50));
    console.log('[Observe] App is starting...');
    console.log('[Observe] Debug build may be slow; please wait...');
    console.log('[Observe] Check the app window on screen');
    console.log('='.repeat(50));

    console.log('[Observe] Waiting 15s for app to fully start...');
    await browser.pause(15000);

    const title = await browser.getTitle();
    console.log('[Observe] Window title:', title || '(none)');

    try {
      const pageSource = await browser.getPageSource();
      console.log('[Observe] Page source length:', pageSource.length, 'chars');
    } catch (e) {
      console.log('[Observe] Could not get page source');
    }

    console.log('');
    console.log('='.repeat(50));
    console.log('[Observe] Window will stay open for 60 seconds');
    console.log('[Observe] You can interact with the app');
    console.log('='.repeat(50));

    for (let i = 6; i > 0; i--) {
      console.log(`[Observe] ${i * 10}s remaining...`);
      await browser.pause(10000);
    }

    console.log('[Observe] Done');
    expect(title).toBeDefined();
  });
});
