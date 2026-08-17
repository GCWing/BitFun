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

      const settingsButton = await $('[data-testid="nav-footer-settings-item"]');
      const settingsButtonVisible = await settingsButton.isDisplayed();

      console.log('[L0] Persistent settings button visible:', settingsButtonVisible);
      expect(settingsButtonVisible).toBe(true);
      await saveStepScreenshot('l0-settings-footer-entry');
    });
  });

  describe('Settings panel interaction', () => {
    it('should open the settings list and then the settings panel', async function () {
      expect(hasWorkspace).toBe(true);

      console.log('[L0] Opening settings list...');
      const settingsButton = await $('[data-testid="nav-footer-settings-item"]');
      await settingsButton.click();
      const settingsMenu = await $('[data-testid="nav-settings-menu"]');
      await settingsMenu.waitForDisplayed({ timeout: 10000 });
      expect(await settingsMenu.isDisplayed()).toBe(true);

      const openSettingsItem = await $('[data-testid="nav-settings-open-item"]');
      await openSettingsItem.click();

      // Check for settings scene
      const settingsScene = await $('.bitfun-settings-scene');
      await settingsScene.waitForDisplayed({ timeout: 10000 });
      const sceneExists = await settingsScene.isDisplayed();

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
