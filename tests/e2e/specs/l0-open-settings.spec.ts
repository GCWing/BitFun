/**
 * L0 open settings spec: verifies settings panel can be opened.
 * Tests basic navigation to settings/config panel.
 */

import { browser, expect, $ } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';
import { saveStepScreenshot } from '../helpers/screenshot-utils';

describe('L0 Settings Panel', () => {
  let hasWorkspace = false;

  describe('Initial setup', () => {
    it('app should start', async () => {
      console.log('[L0] Initializing settings test...');
      await browser.pause(2000);
      const title = await browser.getTitle();
      console.log('[L0] App title:', title);
      expect(title).toBeDefined();
    });

    it('should open workspace if needed', async () => {
      await browser.pause(2000);

      hasWorkspace = await openWorkspace();

      console.log('[L0] Workspace opened:', hasWorkspace);
      expect(hasWorkspace).toBe(true);
      if (hasWorkspace) {
        await saveStepScreenshot('l0-settings-workspace-ready');
      }
    });
  });

  describe('Settings button location', () => {
    it('should find settings/config button', async function () {
      expect(hasWorkspace).toBe(true);

      await browser.pause(1500);

      // Settings is now in NavPanel footer menu (not header)
      const moreBtn = await $('.bitfun-nav-panel__footer-btn--icon');
      const moreBtnExists = await moreBtn.isExisting();

      console.log('[L0] More options button found:', moreBtnExists);
      expect(moreBtnExists).toBe(true);

      // Click to open menu
      await moreBtn.click();
      await browser.pause(500);
      await saveStepScreenshot('l0-settings-menu-opened');

      // Find settings menu item
      const menuItems = await $$('.bitfun-nav-panel__footer-menu-item');
      console.log(`[L0] Found ${menuItems.length} menu items`);
      expect(menuItems.length).toBeGreaterThan(0);

      // Find the settings item (has Settings icon)
      let settingsItem = null;
      for (const item of menuItems) {
        const html = await item.getHTML();
        if (html.includes('Settings') || html.includes('settings')) {
          settingsItem = item;
          break;
        }
      }

      expect(settingsItem).not.toBeNull();
      console.log('[L0] Settings menu item found');

      // Close menu
      const backdrop = await $('.bitfun-nav-panel__footer-backdrop');
      if (await backdrop.isExisting()) {
        await backdrop.click();
        await browser.pause(500);
      }
    });
  });

  describe('Settings panel interaction', () => {
    it('should open and close settings panel', async function () {
      expect(hasWorkspace).toBe(true);

      // Open more options menu
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

      console.log('[L0] Opening settings...');
      await settingsItem!.click();
      await browser.pause(2000);

      // Check for settings scene
      const settingsScene = await $('.bitfun-settings-scene');
      const sceneExists = await settingsScene.isExisting();

      console.log('[L0] Settings scene opened:', sceneExists);
      expect(sceneExists).toBe(true);
      if (sceneExists) {
        await saveStepScreenshot('l0-settings-panel-opened');
      }
    });
  });

  describe('UI stability after settings interaction', () => {
    it('UI should remain responsive', async function () {
      expect(hasWorkspace).toBe(true);

      console.log('[L0] Checking UI responsiveness...');
      await browser.pause(2000);

      const body = await $('body');
      const elementCount = await body.$$('*').then(els => els.length);
      
      expect(elementCount).toBeGreaterThan(10);
      console.log('[L0] UI responsive, element count:', elementCount);
    });
  });
});
