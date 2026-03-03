/**
 * L0 navigation spec: verifies sidebar navigation panel exists and items are visible.
 * Basic checks that navigation structure is present - no AI interaction needed.
 */

import { browser, expect, $ } from '@wdio/globals';

describe('L0 Navigation Panel', () => {
  let hasWorkspace = false;

  describe('Navigation panel existence', () => {
    it('app should start successfully', async () => {
      console.log('[L0] Starting navigation tests...');
      await browser.pause(3000);
      const title = await browser.getTitle();
      console.log('[L0] App title:', title);
      expect(title).toBeDefined();
    });

    it('should detect workspace or startup state', async () => {
      await browser.pause(1000);
      
      // Check for workspace UI (chat input indicates workspace is open)
      const chatInput = await $('[data-testid="chat-input-container"]');
      hasWorkspace = await chatInput.isExisting();
      
      if (hasWorkspace) {
        console.log('[L0] Workspace is open');
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
            console.log(`[L0] On startup page via ${selector}`);
            break;
          }
        } catch (e) {
          // Try next selector
        }
      }

      if (!isStartup) {
        // Fallback: check for scene viewport
        const sceneViewport = await $('.bitfun-scene-viewport');
        isStartup = await sceneViewport.isExisting();
        console.log('[L0] Fallback check - scene viewport exists:', isStartup);
      }

      if (!isStartup && !hasWorkspace) {
        console.error('[L0] CRITICAL: Neither welcome nor workspace UI found');
      }

      // 验证应用处于有效状态：要么是启动页，要么是工作区
      expect(isStartup || hasWorkspace).toBe(true);
    });

    it('should have navigation panel or sidebar when workspace is open', async function () {
      if (!hasWorkspace) {
        console.log('[L0] Skipping: no workspace open');
        this.skip();
        return;
      }

      await browser.pause(500);
      
      const selectors = [
        '[data-testid="nav-panel"]',
        '.bitfun-nav-panel',
        '[class*="nav-panel"]',
        '[class*="NavPanel"]',
        'nav',
        '.sidebar',
      ];

      let navFound = false;
      for (const selector of selectors) {
        const element = await $(selector);
        const exists = await element.isExisting();
        
        if (exists) {
          console.log(`[L0] Navigation panel found: ${selector}`);
          navFound = true;
          break;
        }
      }

      expect(navFound).toBe(true);
    });
  });

  describe('Navigation items visibility', () => {
    it('navigation items should be present if workspace is open', async function () {
      if (!hasWorkspace) {
        console.log('[L0] Skipping: workspace not open');
        this.skip();
        return;
      }

      await browser.pause(500);
      
      const navItemSelectors = [
        '.bitfun-nav-panel__item',
        '[data-testid^="nav-item-"]',
        '[class*="nav-item"]',
        '.nav-item',
        '.bitfun-nav-panel__inline-item',
      ];

      let itemsFound = false;
      let itemCount = 0;

      for (const selector of navItemSelectors) {
        try {
          const items = await browser.$$(selector);
          if (items.length > 0) {
            console.log(`[L0] Found ${items.length} navigation items: ${selector}`);
            itemsFound = true;
            itemCount = items.length;
            break;
          }
        } catch (e) {
          // Continue to next selector
        }
      }

      expect(itemsFound).toBe(true);
      expect(itemCount).toBeGreaterThan(0);
    });

    it('navigation sections should be present', async function () {
      if (!hasWorkspace) {
        console.log('[L0] Skipping: workspace not open');
        this.skip();
        return;
      }

      const sectionSelectors = [
        '.bitfun-nav-panel__sections',
        '.bitfun-nav-panel__section-label',
        '[class*="nav-section"]',
        '.nav-section',
      ];

      let sectionsFound = false;
      for (const selector of sectionSelectors) {
        const sections = await browser.$$(selector);
        if (sections.length > 0) {
          console.log(`[L0] Found ${sections.length} navigation sections: ${selector}`);
          sectionsFound = true;
          break;
        }
      }

      if (!sectionsFound) {
        console.log('[L0] Navigation sections not found (may use different structure)');
      }

      // 导航区域应该存在
      expect(sectionsFound).toBe(true);
    });
  });

  describe('Navigation interactivity', () => {
    it('navigation items should be clickable', async function () {
      if (!hasWorkspace) {
        console.log('[L0] Skipping: workspace not open');
        this.skip();
        return;
      }

      const navItems = await browser.$$('.bitfun-nav-panel__inline-item');
      
      if (navItems.length === 0) {
        const altItems = await browser.$$('.bitfun-nav-panel__item');
        if (altItems.length === 0) {
          console.log('[L0] No nav items found to test clickability');
          this.skip();
          return;
        }
      }

      const firstItem = navItems.length > 0 ? navItems[0] : (await browser.$$('.bitfun-nav-panel__item'))[0];
      const isClickable = await firstItem.isClickable();
      console.log('[L0] First nav item clickable:', isClickable);

      // 导航项应该是可点击的
      expect(isClickable).toBe(true);
    });
  });

  after(async () => {
    console.log('[L0] Navigation tests complete');
  });
});
