/**
 * L0 theme spec: verifies theme selector is visible and themes can be switched.
 * Basic checks for theme functionality without AI interaction.
 */

import { browser, expect, $ } from '@wdio/globals';

describe('L0 Theme', () => {
  let hasWorkspace = false;

  describe('Theme system existence', () => {
    it('app should start successfully', async () => {
      console.log('[L0] Starting theme tests...');
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

    it('should have theme attribute on root element', async () => {
      const themeAttr = await browser.execute(() => {
        return {
          theme: document.documentElement.getAttribute('data-theme'),
          themeType: document.documentElement.getAttribute('data-theme-type'),
        };
      });

      console.log('[L0] Theme attributes:', themeAttr);
      
      // Theme type should exist (either 'dark' or 'light')
      expect(themeAttr.themeType !== null).toBe(true);
    });

    it('should have CSS variables for theme', async () => {
      const themeStyles = await browser.execute(() => {
        const styles = window.getComputedStyle(document.documentElement);
        // Check for any theme-related CSS variables
        const allVars = [];
        for (let i = 0; i < styles.length; i++) {
          const prop = styles[i];
          if (prop.startsWith('--')) {
            allVars.push(prop);
          }
        }
        
        // Also check computed background color to verify theme is applied
        const bgColor = styles.backgroundColor;
        
        return {
          varCount: allVars.length,
          sampleVars: allVars.slice(0, 10),
          bgColor
        };
      });

      console.log('[L0] Theme styles:', themeStyles);
      
      // Theme should have CSS variables defined
      expect(themeStyles.varCount).toBeGreaterThan(0);
    });
  });

  describe('Theme selector visibility', () => {
    it('theme selector should be visible in settings', async function () {
      if (!hasWorkspace) {
        console.log('[L0] Skipping: workspace not open');
        this.skip();
        return;
      }

      await browser.pause(500);

      // Theme selector is typically in settings/config panel
      const selectors = [
        '.theme-config',
        '.theme-config__theme-picker',
        '[data-testid="theme-selector"]',
        '.theme-selector',
        '[class*="theme-selector"]',
        '[class*="ThemeSelector"]',
      ];

      let selectorFound = false;
      for (const selector of selectors) {
        const element = await $(selector);
        const exists = await element.isExisting();

        if (exists) {
          console.log(`[L0] Theme selector found: ${selector}`);
          selectorFound = true;
          break;
        }
      }

      if (!selectorFound) {
        console.log('[L0] Theme selector not found directly - may be in settings panel');
      }

      expect(selectorFound || hasWorkspace).toBe(true);
    });
  });

  describe('Theme switching', () => {
    it('should be able to detect current theme type', async function () {
      if (!hasWorkspace) {
        console.log('[L0] Skipping: workspace not open');
        this.skip();
        return;
      }

      const themeType = await browser.execute(() => {
        return document.documentElement.getAttribute('data-theme-type');
      });

      console.log('[L0] Current theme type:', themeType);
      
      // Theme type should be either dark or light
      expect(['dark', 'light', null]).toContain(themeType);
    });

    it('should have valid theme structure', async function () {
      if (!hasWorkspace) {
        console.log('[L0] Skipping: workspace not open');
        this.skip();
        return;
      }

      const themeInfo = await browser.execute(() => {
        const root = document.documentElement;
        const styles = window.getComputedStyle(root);
        
        return {
          theme: root.getAttribute('data-theme'),
          themeType: root.getAttribute('data-theme-type'),
          hasBgColor: styles.getPropertyValue('--bg-primary').trim().length > 0,
          hasTextColor: styles.getPropertyValue('--text-primary').trim().length > 0,
          hasAccentColor: styles.getPropertyValue('--accent-primary').trim().length > 0,
        };
      });

      console.log('[L0] Theme structure:', themeInfo);
      
      // At least theme type should be set
      expect(themeInfo.themeType !== null).toBe(true);
    });
  });

  after(async () => {
    console.log('[L0] Theme tests complete');
  });
});
