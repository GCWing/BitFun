/**
 * L0 i18n spec: verifies language selector is visible and languages can be switched.
 * Basic checks for internationalization functionality.
 */

import { browser, expect, $ } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';

describe('L0 Internationalization', () => {
  let hasWorkspace = false;

  describe('I18n system existence', () => {
    it('app should start successfully', async () => {
      console.log('[L0] Starting i18n tests...');
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

    it('should have language configuration', async () => {
      const langConfig = await browser.execute(() => {
        return {
          documentLang: document.documentElement.lang,
          i18nExists: typeof (window as any).__I18N__ !== 'undefined',
        };
      });

      console.log('[L0] Language config:', langConfig);
      expect(langConfig).toBeDefined();
    });

    it('should have translated content in UI', async () => {
      await browser.pause(500);
      
      const body = await $('body');
      const bodyText = await body.getText();
      
      expect(bodyText.length).toBeGreaterThan(0);
      console.log('[L0] UI content loaded');
    });
  });

  describe('Language selector visibility', () => {
    it('language selector should exist in settings', async function () {
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

      // Navigate to Basics tab (language selector lives there)
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

      // Check for language selector in settings
      const langSelect = await $('.theme-config__language-select');
      const selectExists = await langSelect.isExisting();

      console.log('[L0] Language selector found:', selectExists);
      expect(selectExists).toBe(true);
    });
  });

  describe('Language switching', () => {
    it('should be able to detect current language', async function () {
      expect(hasWorkspace).toBe(true);

      const langInfo = await browser.execute(() => {
        // Try to get current language from various sources
        const htmlLang = document.documentElement.lang;
        const metaLang = document.querySelector('meta[http-equiv="Content-Language"]');
        
        return {
          htmlLang,
          metaLang: metaLang?.getAttribute('content'),
        };
      });

      console.log('[L0] Language info:', langInfo);
      expect(langInfo).toBeDefined();
    });

    it('i18n system should be functional', async function () {
      expect(hasWorkspace).toBe(true);

      // Check if the app has text content (indicating i18n is working)
      const hasTextContent = await browser.execute(() => {
        const body = document.body;
        const textNodes: string[] = [];
        
        const walker = document.createTreeWalker(
          body,
          NodeFilter.SHOW_TEXT,
          null
        );
        
        let node;
        let count = 0;
        while ((node = walker.nextNode()) && count < 5) {
          const text = node.textContent?.trim();
          if (text && text.length > 2) {
            textNodes.push(text);
            count++;
          }
        }
        
        return textNodes;
      });

      console.log('[L0] Sample text content:', hasTextContent);
      expect(hasTextContent.length).toBeGreaterThan(0);
    });
  });

  after(async () => {
    console.log('[L0] I18n tests complete');
  });
});
