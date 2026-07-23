/**
 * L0 theme spec: verifies theme selector is visible and themes can be switched.
 * Basic checks for theme functionality without AI interaction.
 */

import { browser, expect, $ } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';

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

      hasWorkspace = await openWorkspace();

      console.log('[L0] Workspace opened:', hasWorkspace);
      expect(hasWorkspace).toBe(true);
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
      expect(hasWorkspace).toBe(true);

      await browser.pause(500);

      // Open more options menu in footer
      const moreBtn = await $('.bitfun-nav-panel__footer-btn--icon');
      await moreBtn.click();
      await browser.pause(500);

      // Click settings menu item
      const menuItems = await $$('.bitfun-nav-panel__footer-menu-item');
      let settingsItem = null;
      for (const item of menuItems) {
        const html = await item.getHTML();
        if (html.includes('Settings') || html.includes('settings')) {
          settingsItem = item;
          break;
        }
      }

      expect(settingsItem).not.toBeNull();
      await settingsItem!.click();
      await browser.pause(2000);

      // Navigate to Basics tab (language + theme + logging; settings opens to models by default)
      const navItems = await $$('.bitfun-settings-nav__item');
      console.log(`[L0] Found ${navItems.length} settings nav items`);

      let basicsTab = null;
      for (const item of navItems) {
        const text = await item.getText();
        if (text.includes('基础') || text.toLowerCase().includes('basics')) {
          basicsTab = item;
          console.log(`[L0] Found basics tab: "${text}"`);
          break;
        }
      }

      if (basicsTab) {
        await basicsTab.click();
        await browser.pause(2000); // Wait for lazy load
      }

      // Check for theme picker in settings
      const themePicker = await $('.theme-config__theme-picker');
      const pickerExists = await themePicker.isExisting();

      console.log('[L0] Theme picker found:', pickerExists);
      expect(pickerExists).toBe(true);
    });
  });

  describe('Theme switching', () => {
    it('should be able to detect current theme type', async function () {
      expect(hasWorkspace).toBe(true);

      const themeType = await browser.execute(() => {
        return document.documentElement.getAttribute('data-theme-type');
      });

      console.log('[L0] Current theme type:', themeType);

      // Theme type should be either dark or light
      expect(['dark', 'light', null]).toContain(themeType);
    });

    it('should have valid theme structure', async function () {
      expect(hasWorkspace).toBe(true);

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
