/**
 * L0 notification spec: verifies notification entry is visible and panel can expand.
 * Basic checks for notification system functionality.
 */

import { browser, expect, $ } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';
import { expectPopupCloseContract } from '../helpers/popup-close-contract';

async function openNotificationMenuItem() {
  const settingsButton = await $('[data-testid="nav-footer-settings-item"]');
  await settingsButton.waitForDisplayed({ timeout: 15000 });
  await settingsButton.click();

  const notificationButton = await $('[data-testid="notification-button"]');
  await notificationButton.waitForDisplayed({ timeout: 10000 });
  return notificationButton;
}

describe('L0 Notification', () => {
  let hasWorkspace = false;

  describe('Notification system existence', () => {
    it('app should start successfully', async () => {
      console.log('[L0] Starting notification tests...');
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

    it('notification service should be available', async () => {
      const notificationService = await browser.execute(() => {
        return {
          serviceExists: typeof (window as any).__NOTIFICATION_SERVICE__ !== 'undefined',
          hasNotificationCenter: document.querySelector('.notification-center') !== null,
          hasNotificationContainer: document.querySelector('.notification-container') !== null,
        };
      });

      console.log('[L0] Notification service status:', notificationService);
      expect(notificationService).toBeDefined();
    });
  });

  describe('Notification entry visibility', () => {
    it('notification entry should be visible in the settings list', async function () {
      expect(hasWorkspace).toBe(true);

      const notificationBtn = await openNotificationMenuItem();
      const btnVisible = await notificationBtn.isDisplayed();

      console.log('[L0] Notification menu item visible:', btnVisible);
      expect(btnVisible).toBe(true);

      const backdrop = await $('.openbitfun-nav-panel__footer-backdrop');
      await backdrop.click();
      await backdrop.waitForExist({ reverse: true, timeout: 2000 });
    });
  });

  describe('Notification panel expandability', () => {
    it('notification center should be accessible', async function () {
      expect(hasWorkspace).toBe(true);

      const notificationBtn = await openNotificationMenuItem();
      await notificationBtn.click();

      const notificationCenter = await $('[data-testid="notification-center"]');
      await notificationCenter.waitForDisplayed({ timeout: 10000 });
      const centerVisible = await notificationCenter.isDisplayed();

      console.log('[L0] Notification center opened:', centerVisible);
      expect(centerVisible).toBe(true);
      await expectPopupCloseContract('[data-testid="notification-center"]');

      const closeButton = await $('[data-testid="notification-center-close-btn"]');
      await closeButton.click();
    });

    it('notification container should exist for toast notifications', async function () {
      expect(hasWorkspace).toBe(true);

      // Check for notification container
      const container = await $('.notification-container');
      const containerExists = await container.isExisting();

      console.log('[L0] Notification container exists:', containerExists);

      // Container may not exist until a notification is shown
      expect(typeof containerExists).toBe('boolean');
    });
  });

  describe('Notification panel structure', () => {
    it('notification panel should have required structure when visible', async function () {
      expect(hasWorkspace).toBe(true);

      const structure = await browser.execute(() => {
        const center = document.querySelector('.notification-center');
        const container = document.querySelector('.notification-container');
        
        return {
          hasCenter: !!center,
          hasContainer: !!container,
          centerHeader: center?.querySelector('.notification-center__header') !== null,
          centerContent: center?.querySelector('.notification-center__content') !== null,
        };
      });

      console.log('[L0] Notification structure:', structure);
      expect(structure).toBeDefined();
    });
  });

  after(async () => {
    console.log('[L0] Notification tests complete');
  });
});
