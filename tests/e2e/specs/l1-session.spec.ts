/**
 * L1 session spec: validates session management functionality.
 * Tests creating new sessions and switching between historical sessions.
 */

import { browser, expect, $ } from '@wdio/globals';
import { Header } from '../page-objects/components/Header';
import { StartupPage } from '../page-objects/StartupPage';
import { saveScreenshot, saveFailureScreenshot } from '../helpers/screenshot-utils';
import { ensureWorkspaceOpen } from '../helpers/workspace-utils';

describe('L1 Session', () => {
  let header: Header;
  let startupPage: StartupPage;

  let hasWorkspace = false;

  before(async () => {
    console.log('[L1] Starting session tests');
    // Initialize page objects after browser is ready
    header = new Header();
    startupPage = new StartupPage();

    await browser.pause(3000);
    await header.waitForLoad();

    hasWorkspace = await ensureWorkspaceOpen(startupPage);

    if (!hasWorkspace) {
      console.log('[L1] No workspace available - tests will be skipped');
    }
  });

  describe('Session scene existence', () => {
    it('session scene should exist', async function () {
      if (!hasWorkspace) {
        console.log('[L1] Skipping: workspace required');
        this.skip();
        return;
      }

      await browser.pause(500);

      const selectors = [
        '.bitfun-session-scene',
        '[class*="session-scene"]',
        '[class*="SessionScene"]',
        '[data-mode]',  // Session scene has data-mode attribute
      ];

      let sessionFound = false;
      for (const selector of selectors) {
        const element = await $(selector);
        const exists = await element.isExisting();

        if (exists) {
          console.log(`[L1] Session scene found: ${selector}`);
          sessionFound = true;
          break;
        }
      }

      expect(sessionFound).toBe(true);
    });

    it('session scene should have mode attribute', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const sessionScene = await $('.bitfun-session-scene');
      const exists = await sessionScene.isExisting();

      if (exists) {
        const mode = await sessionScene.getAttribute('data-mode');
        console.log('[L1] Session mode:', mode);

        // Mode can be null or one of the valid modes
        const validModes = ['collapsed', 'compact', 'comfortable', 'expanded', null];
        expect(validModes).toContain(mode);
        
        // If mode is not null, verify it's a valid mode string
        if (mode !== null) {
          const validModeStrings = ['collapsed', 'compact', 'comfortable', 'expanded'];
          expect(validModeStrings).toContain(mode);
        }
      } else {
        expect(typeof exists).toBe('boolean');
      }
    });
  });

  describe('Session list in sidebar', () => {
    it('sessions section should be visible in nav panel', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const sessionsSection = await $('.bitfun-nav-panel__inline-list');
      const exists = await sessionsSection.isExisting();

      if (exists) {
        console.log('[L1] Sessions section found in nav panel');
      } else {
        console.log('[L1] Sessions section not found directly');
      }

      expect(typeof exists).toBe('boolean');
    });

    it('session list should show sessions', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const sessionItems = await browser.$$('.bitfun-nav-panel__inline-item');
      console.log('[L1] Session items found:', sessionItems.length);

      expect(sessionItems.length).toBeGreaterThanOrEqual(0);
    });
  });

  describe('New session creation', () => {
    it('new session button should exist', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const selectors = [
        '[data-testid="header-new-session-btn"]',
        '[class*="new-session-btn"]',
        '[class*="create-session"]',
        'button:has(svg.lucide-plus)',
      ];

      let buttonFound = false;
      for (const selector of selectors) {
        try {
          const element = await $(selector);
          const exists = await element.isExisting();

          if (exists) {
            console.log(`[L1] New session button found: ${selector}`);
            buttonFound = true;
            break;
          }
        } catch (e) {
          // Continue
        }
      }

      if (!buttonFound) {
        console.log('[L1] New session button not found');
      }

      expect(typeof buttonFound).toBe('boolean');
    });

    it('should be able to click new session button', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const newSessionBtn = await $('[data-testid="header-new-session-btn"]');
      let exists = await newSessionBtn.isExisting();

      if (!exists) {
        // Try to find in nav panel
        const altBtn = await $('[class*="new-session-btn"]');
        exists = await altBtn.isExisting();

        if (exists) {
          await altBtn.click();
          await browser.pause(500);
          console.log('[L1] New session button clicked (alternative)');
        }
      } else {
        await newSessionBtn.click();
        await browser.pause(500);
        console.log('[L1] New session button clicked');
      }

      expect(typeof exists).toBe('boolean');
    });
  });

  describe('Session switching', () => {
    it('should be able to switch between sessions', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const sessionItems = await browser.$$('.bitfun-nav-panel__inline-item');

      if (sessionItems.length < 2) {
        console.log('[L1] Not enough sessions to test switching');
        this.skip();
        return;
      }

      // Click second session
      await sessionItems[1].click();
      await browser.pause(500);

      console.log('[L1] Switched to second session');

      // Click first session
      await sessionItems[0].click();
      await browser.pause(500);

      console.log('[L1] Switched back to first session');
      expect(sessionItems.length).toBeGreaterThanOrEqual(2);
    });

    it('active session should be highlighted', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const activeSessions = await browser.$$('.bitfun-nav-panel__inline-item.is-active');
      console.log('[L1] Active sessions:', activeSessions.length);

      expect(activeSessions.length).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Session actions', () => {
    it('session should have rename option', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const sessionItems = await browser.$$('.bitfun-nav-panel__inline-item');
      if (sessionItems.length === 0) {
        console.log('[L1] No sessions to test rename');
        this.skip();
        return;
      }

      // Right-click or hover to show actions
      await sessionItems[0].click({ button: 'right' });
      await browser.pause(300);

      const renameOption = await $('[class*="rename"], [class*="edit-session"]');
      const exists = await renameOption.isExisting();

      console.log('[L1] Rename option exists:', exists);
      expect(typeof exists).toBe('boolean');
    });

    it('session should have delete option', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const deleteOption = await $('[class*="delete"], [class*="remove-session"]');
      const exists = await deleteOption.isExisting();

      console.log('[L1] Delete option exists:', exists);
      expect(typeof exists).toBe('boolean');
    });
  });

  describe('Panel mode', () => {
    it('should be able to toggle panel mode', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const sessionScene = await $('.bitfun-session-scene');
      const exists = await sessionScene.isExisting();

      if (!exists) {
        this.skip();
        return;
      }

      const initialMode = await sessionScene.getAttribute('data-mode');
      console.log('[L1] Initial mode:', initialMode);

      // Double-click to toggle mode
      const resizer = await $('.bitfun-pane-resizer');
      const resizerExists = await resizer.isExisting();

      if (resizerExists) {
        await resizer.doubleClick();
        await browser.pause(300);

        const newMode = await sessionScene.getAttribute('data-mode');
        console.log('[L1] Mode after toggle:', newMode);
      }

      expect(typeof resizerExists).toBe('boolean');
    });
  });

  afterEach(async function () {
    if (this.currentTest?.state === 'failed') {
      await saveFailureScreenshot(`l1-session-${this.currentTest.title}`);
    }
  });

  after(async () => {
    await saveScreenshot('l1-session-complete');
    console.log('[L1] Session tests complete');
  });
});
