/**
 * L0 smoke spec: minimal critical checks that the app starts.
 * These tests must pass before any release - they verify basic app functionality.
 */

import { browser, expect, $ } from '@wdio/globals';

describe('L0 Smoke Tests', () => {
  describe('Application launch', () => {
    it('app window should open with title', async () => {
      await browser.pause(5000);
      const title = await browser.getTitle();
      console.log('[L0] App title:', title);
      expect(title).toBeDefined();
      expect(title.length).toBeGreaterThan(0);
    });

    it('document should be in ready state', async () => {
      const readyState = await browser.execute(() => document.readyState);
      expect(readyState).toBe('complete');
      console.log('[L0] Document ready state: complete');
    });
  });

  describe('DOM structure', () => {
    it('page should have body element', async () => {
      await browser.pause(1000);
      const body = await $('body');
      const exists = await body.isExisting();
      expect(exists).toBe(true);
      console.log('[L0] Body element exists');
    });

    it('should have root React element', async () => {
      const root = await $('#root');
      const exists = await root.isExisting();

      if (exists) {
        console.log('[L0] Found #root element');
        expect(exists).toBe(true);
      } else {
        const appLayout = await $('[data-testid="app-layout"]');
        const appExists = await appLayout.isExisting();
        console.log('[L0] app-layout exists:', appExists);
        expect(appExists).toBe(true);
      }
    });

    it('should have non-trivial DOM tree', async () => {
      const elementCount = await browser.execute(() => {
        return document.querySelectorAll('*').length;
      });
      
      expect(elementCount).toBeGreaterThan(10);
      console.log('[L0] DOM element count:', elementCount);
    });
  });

  describe('Core UI components', () => {
    it('Header should be visible', async () => {
      await browser.pause(2000);
      const header = await $('.bitfun-nav-panel, .bitfun-scene-bar, .bitfun-nav-bar, [data-testid="header-container"]');
      const exists = await header.isExisting();

      if (exists) {
        console.log('[L0] Header found via data-testid');
        expect(exists).toBe(true);
      } else {
        console.log('[L0] Checking fallback selectors...');
        const selectors = [
          '.bitfun-nav-panel',
          '.bitfun-scene-bar',
          '.bitfun-nav-bar',
          'header',
          '.header',
          '[class*="header"]',
          '[class*="Header"]'
        ];

        let found = false;
        for (const selector of selectors) {
          const element = await $(selector);
          const fallbackExists = await element.isExisting();
          if (fallbackExists) {
            console.log(`[L0] Header found: ${selector}`);
            found = true;
            break;
          }
        }

        if (!found) {
          const html = await $('body').getHTML();
          console.log('[L0] Body HTML snippet:', html.substring(0, 500));
          console.error('[L0] CRITICAL: Header not found - frontend may not be loaded');
        }
        
        expect(found).toBe(true);
      }
    });

    it('should have either startup page or workspace UI', async () => {
      // Check for workspace UI (chat input indicates workspace is open)
      const chatInput = await $('[data-testid="chat-input-container"]');
      const chatExists = await chatInput.isExisting();

      if (chatExists) {
        console.log('[L0] Workspace UI visible');
        expect(chatExists).toBe(true);
        return;
      }

      // Check for welcome/startup scene with multiple selectors
      const welcomeSelectors = [
        '.welcome-scene--first-time',
        '.welcome-scene',
        '.bitfun-scene-viewport--welcome',
      ];

      let welcomeExists = false;
      for (const selector of welcomeSelectors) {
        try {
          const element = await $(selector);
          welcomeExists = await element.isExisting();
          if (welcomeExists) {
            console.log(`[L0] Welcome/startup page visible via ${selector}`);
            break;
          }
        } catch (e) {
          // Try next selector
        }
      }

      if (!welcomeExists) {
        // Fallback: check for scene viewport
        const sceneViewport = await $('.bitfun-scene-viewport');
        welcomeExists = await sceneViewport.isExisting();
        console.log('[L0] Fallback check - scene viewport exists:', welcomeExists);
      }

      if (!welcomeExists && !chatExists) {
        console.error('[L0] CRITICAL: Neither welcome nor workspace UI found');
      }

      expect(welcomeExists || chatExists).toBe(true);
    });
  });

  describe('No critical errors', () => {
    it('should not have JavaScript errors', async () => {
      const logs = await browser.getLogs('browser');
      const errors = logs.filter(log => log.level === 'SEVERE');
      
      if (errors.length > 0) {
        console.error('[L0] Console errors detected:', errors.length);
        errors.slice(0, 3).forEach(err => {
          console.error('[L0] Error:', err.message);
        });
      } else {
        console.log('[L0] No JavaScript errors');
      }
      
      expect(errors.length).toBe(0);
    });

    it('viewport should have valid dimensions', async () => {
      const dimensions = await browser.execute(() => {
        return {
          width: window.innerWidth,
          height: window.innerHeight,
        };
      });
      
      expect(dimensions.width).toBeGreaterThan(0);
      expect(dimensions.height).toBeGreaterThan(0);
      console.log('[L0] Viewport dimensions:', dimensions);
    });
  });
});
