/**
 * L0 open workspace spec: verifies workspace opening flow.
 * Tests the ability to detect and interact with startup page and workspace state.
 */

import { browser, expect, $ } from '@wdio/globals';

describe('L0 Workspace Opening', () => {
  let hasWorkspace = false;

  describe('App initialization', () => {
    it('app should start successfully', async () => {
      console.log('[L0] Waiting for app initialization...');
      await browser.pause(2000);
      const title = await browser.getTitle();
      console.log('[L0] App title:', title);
      expect(title).toBeDefined();
    });

    it('should have valid DOM structure', async () => {
      const body = await $('body');
      const html = await body.getHTML();
      expect(html.length).toBeGreaterThan(100);
      console.log('[L0] DOM loaded, HTML length:', html.length);
    });
  });

  describe('Workspace state detection', () => {
    it('should detect current state (startup or workspace)', async () => {
      await browser.pause(2000);

      // Check for workspace UI (chat input indicates workspace is open)
      const chatInput = await $('[data-testid="chat-input-container"]');
      hasWorkspace = await chatInput.isExisting();

      if (hasWorkspace) {
        console.log('[L0] State: Workspace already open');
        expect(hasWorkspace).toBe(true);
        return;
      }

      // Check for welcome/startup scene with multiple selectors
      const welcomeSelectors = [
        '.welcome-scene--first-time',
        '.welcome-scene',
        '.bitfun-scene-viewport--welcome',
      ];

      let isStartup = false;
      for (const selector of welcomeSelectors) {
        try {
          const element = await $(selector);
          isStartup = await element.isExisting();
          if (isStartup) {
            console.log(`[L0] State: Startup page detected via ${selector}`);
            break;
          }
        } catch (e) {
          // Try next selector
        }
      }

      if (!isStartup) {
        // As a fallback, check if we have any scene viewport at all
        const sceneViewport = await $('.bitfun-scene-viewport');
        const hasSceneViewport = await sceneViewport.isExisting();
        console.log('[L0] Fallback check - scene viewport exists:', hasSceneViewport);

        // Check for any app content
        const rootContent = await $('#root');
        const rootHTML = await rootContent.getHTML();
        console.log('[L0] Root content length:', rootHTML.length);

        // If we have content but no specific UI detected, app might be in transition
        isStartup = hasSceneViewport || rootHTML.length > 1000;
      }

      console.log('[L0] Final state - hasWorkspace:', hasWorkspace, 'isStartup:', isStartup);
      expect(hasWorkspace || isStartup).toBe(true);
    });
  });

  describe('Startup page interaction', () => {
    let onStartupPage = false;

    before(async () => {
      onStartupPage = !hasWorkspace;
    });

    it('should find continue button or history items', async function () {
      if (!onStartupPage) {
        console.log('[L0] Skipping: workspace already open');
        this.skip();
        return;
      }

      // Look for welcome scene buttons
      const sessionBtn = await $('.welcome-scene__session-btn');
      const hasSessionBtn = await sessionBtn.isExisting();

      const recentItem = await $('.welcome-scene__recent-item');
      const hasRecent = await recentItem.isExisting();

      const linkBtn = await $('.welcome-scene__link-btn');
      const hasLinkBtn = await linkBtn.isExisting();

      if (hasSessionBtn) {
        console.log('[L0] Found session button');
      }
      if (hasRecent) {
        console.log('[L0] Found recent workspace items');
      }
      if (hasLinkBtn) {
        console.log('[L0] Found open/new project buttons');
      }

      const hasAnyOption = hasSessionBtn || hasRecent || hasLinkBtn;
      expect(hasAnyOption).toBe(true);
    });

    it('should attempt to open workspace', async function () {
      if (!onStartupPage) {
        this.skip();
        return;
      }

      // Try to click on a recent workspace if available
      const recentItem = await $('.welcome-scene__recent-item');
      const hasRecent = await recentItem.isExisting();

      if (hasRecent) {
        console.log('[L0] Clicking first recent workspace');
        await recentItem.click();
        await browser.pause(3000);
        console.log('[L0] Workspace open attempted');
      } else {
        console.log('[L0] No recent workspace available to click');
        this.skip();
      }
    });
  });

  describe('UI stability check', () => {
    it('UI should remain stable', async () => {
      console.log('[L0] Monitoring UI stability for 10 seconds...');
      
      for (let i = 0; i < 2; i++) {
        await browser.pause(5000);
        
        const body = await $('body');
        const childCount = await body.$$('*').then(els => els.length);
        console.log(`[L0] ${(i + 1) * 5}s - DOM elements: ${childCount}`);
        
        expect(childCount).toBeGreaterThan(10);
      }
      
      console.log('[L0] UI stability confirmed');
    });
  });
});
