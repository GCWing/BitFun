/**
 * App launch smoke spec: app starts and renders basic UI.
 */

import { browser, expect } from '@wdio/globals';
import { Header } from '../../page-objects/components/Header';
import { StartupPage } from '../../page-objects/StartupPage';
import { isTauriAvailable, getWindowInfo } from '../../helpers/tauri-utils';
import { saveScreenshot, saveFailureScreenshot } from '../../helpers/screenshot-utils';

describe('BitFun app launch', () => {
  const header = new Header();
  const startupPage = new StartupPage();

  before(async () => {
    await browser.pause(3000);
  });

  describe('Window and basic render', () => {
    it('app window should open', async () => {
      const tauriAvailable = await isTauriAvailable();
      expect(tauriAvailable).toBe(true);

      const handle = await browser.getWindowHandle();
      expect(handle).toBeDefined();

      const visibilityState = await browser.execute(() => document.visibilityState);
      expect(visibilityState).toBe('visible');

      const windowInfo = await getWindowInfo();
      if (windowInfo) {
        expect(windowInfo.isVisible).toBe(true);
      }
    });

    it('should get window title', async () => {
      const title = await browser.getTitle();
      expect(title).toBeDefined();
      console.log('[Test] Window title:', title);
    });

    it('Header component should render', async () => {
      await header.waitForLoad();
      const isVisible = await header.isVisible();
      expect(isVisible).toBe(true);
    });

    it('window control buttons should be visible', async () => {
      const controlsVisible = await header.areWindowControlsVisible();
      expect(controlsVisible).toBe(true);
    });
  });

  describe('Startup page (no workspace)', () => {
    it('startup page should display', async () => {
      await startupPage.waitForLoad();
      const isVisible = await startupPage.isVisible();
      if (isVisible) {
        console.log('[Test] App in startup mode (no workspace)');
        const openFolderVisible = await startupPage.isOpenFolderButtonVisible();
        expect(openFolderVisible).toBe(true);
      } else {
        console.log('[Test] Workspace already open, skipping startup checks');
      }
    });
  });

  describe('Basic interaction', () => {
    it('clicking maximize should toggle window state', async () => {
      const maximizeVisible = await header.isMaximizeButtonVisible();
      if (!maximizeVisible) {
        expect(await header.areWindowControlsVisible()).toBe(true);
        return;
      }

      const initialInfo = await getWindowInfo();
      const initialTitle = await browser.getTitle();

      await header.clickMaximize();
      await browser.pause(500);

      const newInfo = await getWindowInfo();

      if (initialInfo && newInfo) {
        expect(newInfo.isMaximized).toBe(!initialInfo.isMaximized);
      } else {
        expect(await header.isVisible()).toBe(true);
        expect(await browser.getTitle()).toBe(initialTitle);
      }

      await header.clickMaximize();
      await browser.pause(500);
    });
  });

  afterEach(async function () {
    if (this.currentTest?.state === 'failed') {
      await saveFailureScreenshot(this.currentTest.title);
    }
  });

  after(async () => {
    await saveScreenshot('app-launch-complete');
  });
});
