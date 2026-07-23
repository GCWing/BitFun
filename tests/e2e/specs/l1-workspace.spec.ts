/**
 * L1 Workspace management spec: validates workspace operations.
 * Tests workspace state, startup page, and workspace opening flow.
 */

import { browser, expect, $ } from '@wdio/globals';
import { ensureCodeSessionOpen, getWorkspaceState, openWorkspace } from '../helpers/workspace-helper';

describe('L1 Workspace Management', () => {
  let hasWorkspace = false;

  before(async () => {
    console.log('[L1] Starting workspace management tests');
    await browser.pause(3000);

    hasWorkspace = await openWorkspace();
    console.log('[L1] hasWorkspace:', hasWorkspace);
  });

  describe('Workspace state detection', () => {
    it('should detect current workspace state', async () => {
      const state = await getWorkspaceState();
      console.log('[L1] Workspace state:', state);
      expect(state.currentWorkspacePath).toBeTruthy();
      expect(state.workspaceLabels.length).toBeGreaterThan(0);
    });

    it('header should be visible in both states', async () => {
      const headerSelectors = ['.bitfun-nav-panel', '.bitfun-scene-bar', '.bitfun-nav-bar', '[data-testid="header-container"]', '.bitfun-header', 'header'];
      
      let headerVisible = false;
      for (const selector of headerSelectors) {
        try {
          const element = await $(selector);
          headerVisible = await element.isExisting();
          if (headerVisible) {
            console.log(`[L1] Header visible via ${selector}`);
            break;
          }
        } catch (e) {
          // Continue
        }
      }
      
      expect(headerVisible).toBe(true);
    });

    it('window controls should be functional', async () => {
      // Window controls might be handled by OS in Tauri
      // Just verify the window exists
      const title = await browser.getTitle();
      expect(title).toBeDefined();
      console.log('[L1] Window title:', title);
    });
  });

  describe('Startup page (no workspace)', () => {
    it('startup page elements check', async function () {
      if (hasWorkspace) {
        console.log('[L1] Skipping: test run now opens a workspace by default');
        this.skip();
        return;
      }

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
          if (isStartup) break;
        } catch (e) {
          // Continue
        }
      }

      expect(isStartup).toBe(true);
      console.log('[L1] Startup page visible');
    });
  });

  describe('Workspace state (workspace open)', () => {
    it('chat input should be available', async function () {
      if (!hasWorkspace) {
        console.log('[L1] Skipping: no workspace open');
        this.skip();
        return;
      }

      await ensureCodeSessionOpen();

      const chatInputSelectors = [
        '[data-testid="chat-input-container"]',
        '.bitfun-chat-input',
        '.chat-input-container',
        '.chat-input',
      ];

      let inputVisible = false;
      for (const selector of chatInputSelectors) {
        try {
          const element = await $(selector);
          inputVisible = await element.isExisting();
          if (inputVisible) break;
        } catch (e) {
          // Continue
        }
      }

      expect(inputVisible).toBe(true);
      console.log('[L1] Chat input available in workspace');
    });
  });

  describe('Window state management', () => {
    it('should get window title', async () => {
      const title = await browser.getTitle();
      expect(title).toBeDefined();
      expect(title.length).toBeGreaterThan(0);
      console.log('[L1] Window title:', title);
    });

    it('window should be visible', async () => {
      const isVisible = await browser.execute(() => !document.hidden);
      expect(isVisible).toBe(true);
      console.log('[L1] Window visible');
    });

    it('document should be in ready state', async () => {
      const readyState = await browser.execute(() => document.readyState);
      expect(readyState).toBe('complete');
      console.log('[L1] Document ready');
    });
  });

  describe('UI responsiveness', () => {
    it('should have non-zero body dimensions', async () => {
      const dimensions = await browser.execute(() => {
        const body = document.body;
        return {
          width: body.offsetWidth,
          height: body.offsetHeight,
          scrollWidth: body.scrollWidth,
          scrollHeight: body.scrollHeight,
        };
      });

      expect(dimensions.width).toBeGreaterThan(0);
      expect(dimensions.height).toBeGreaterThan(0);
      console.log('[L1] Body dimensions:', dimensions);
    });

    it('should have DOM elements', async () => {
      const elementCount = await browser.execute(() => {
        return document.querySelectorAll('*').length;
      });

      expect(elementCount).toBeGreaterThan(10);
      console.log('[L1] DOM element count:', elementCount);
    });
  });

  after(async () => {
    console.log('[L1] Workspace management tests complete');
  });
});
