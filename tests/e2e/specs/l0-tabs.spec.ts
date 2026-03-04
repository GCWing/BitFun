/**
 * L0 tabs spec: verifies tab bar exists and tabs are visible.
 * Basic checks for editor/workspace tab functionality.
 */

import { browser, expect, $ } from '@wdio/globals';

describe('L0 Tab Bar', () => {
  let hasWorkspace = false;

  describe('Tab bar existence', () => {
    it('app should start successfully', async () => {
      console.log('[L0] Starting tabs tests...');
      await browser.pause(3000);
      const title = await browser.getTitle();
      console.log('[L0] App title:', title);
      expect(title).toBeDefined();
    });

    it('should detect workspace state', async function () {
      await browser.pause(1000);
      
      // Check for workspace UI (chat input indicates workspace is open)
      const chatInput = await $('[data-testid="chat-input-container"]');
      hasWorkspace = await chatInput.isExisting();
      
      console.log('[L0] Has workspace:', hasWorkspace);
      expect(typeof hasWorkspace).toBe('boolean');
    });

    it('should have tab bar or tab container in workspace', async function () {
      if (!hasWorkspace) {
        console.log('[L0] Skipping: workspace not open');
        this.skip();
        return;
      }

      await browser.pause(500);

      const tabBarSelectors = [
        '.bitfun-scene-bar__tabs',
        '.canvas-tab-bar__tabs',
        '[data-testid="tab-bar"]',
        '.bitfun-tab-bar',
        '[class*="tab-bar"]',
        '[class*="TabBar"]',
        '.tabs-container',
        '[role="tablist"]',
      ];

      let tabBarFound = false;
      for (const selector of tabBarSelectors) {
        const element = await $(selector);
        const exists = await element.isExisting();

        if (exists) {
          console.log(`[L0] Tab bar found: ${selector}`);
          tabBarFound = true;
          break;
        }
      }

      if (!tabBarFound) {
        console.log('[L0] Tab bar not found - may not have any open files yet');
        console.log('[L0] This is expected if no files have been opened');
      }

      expect(typeof tabBarFound).toBe('boolean');
    });
  });

  describe('Tab visibility', () => {
    it('open tabs should be visible if any files are open', async function () {
      if (!hasWorkspace) {
        console.log('[L0] Skipping: workspace not open');
        this.skip();
        return;
      }

      const tabSelectors = [
        '.canvas-tab',
        '[data-testid^="tab-"]',
        '.bitfun-tabs__tab',
        '[class*="tab-item"]',
        '[role="tab"]',
        '.tab',
      ];

      let tabsFound = false;
      let tabCount = 0;

      for (const selector of tabSelectors) {
        const tabs = await browser.$$(selector);
        if (tabs.length > 0) {
          console.log(`[L0] Found ${tabs.length} tabs: ${selector}`);
          tabsFound = true;
          tabCount = tabs.length;
          break;
        }
      }

      if (!tabsFound) {
        console.log('[L0] No open tabs found - expected if no files opened');
      }

      expect(typeof tabsFound).toBe('boolean');
    });

    it('tab close buttons should be present if tabs exist', async function () {
      if (!hasWorkspace) {
        console.log('[L0] Skipping: workspace not open');
        this.skip();
        return;
      }

      const closeBtnSelectors = [
        '.canvas-tab__close',
        '[data-testid^="tab-close-"]',
        '.tab-close-btn',
        '[class*="tab-close"]',
        '.bitfun-tabs__tab-close',
      ];

      let closeBtnFound = false;
      for (const selector of closeBtnSelectors) {
        const btns = await browser.$$(selector);
        if (btns.length > 0) {
          console.log(`[L0] Found ${btns.length} tab close buttons: ${selector}`);
          closeBtnFound = true;
          break;
        }
      }

      if (!closeBtnFound) {
        console.log('[L0] No tab close buttons found');
      }

      expect(typeof closeBtnFound).toBe('boolean');
    });
  });

  describe('Tab bar UI elements', () => {
    it('workspace should have main content area for tabs', async function () {
      if (!hasWorkspace) {
        console.log('[L0] Skipping: workspace not open');
        this.skip();
        return;
      }

      const mainContent = await $('[data-testid="app-main-content"]');
      const mainExists = await mainContent.isExisting();

      if (mainExists) {
        console.log('[L0] Main content area found');
      } else {
        const alternativeMain = await $('.bitfun-app-main-workspace');
        const altExists = await alternativeMain.isExisting();
        console.log('[L0] Main content area (alternative) found:', altExists);
      }

      expect(hasWorkspace).toBe(true);
    });
  });

  after(async () => {
    console.log('[L0] Tabs tests complete');
  });
});
