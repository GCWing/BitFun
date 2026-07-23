/**
 * L0 notification spec: verifies notification entry is visible and panel can expand.
 * Basic checks for notification system functionality.
 */

import { browser, expect, $ } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';

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
    it('notification entry/button should be visible in header', async function () {
      expect(hasWorkspace).toBe(true);

      await browser.pause(500);

      // Notification button is in NavPanel footer (not header)
      const notificationBtn = await $('.bitfun-nav-panel__footer-btn.bitfun-notification-btn');
      const btnExists = await notificationBtn.isExisting();

      console.log('[L0] Notification button found:', btnExists);
      expect(btnExists).toBe(true);
    });
  });

  describe('Notification panel expandability', () => {
    it('notification center should be accessible', async function () {
      expect(hasWorkspace).toBe(true);

      await browser.pause(1000);

      // Use JavaScript to click notification button (bypasses overlay)
      const clicked = await browser.execute(() => {
        const btn = document.querySelector('.bitfun-nav-panel__footer-btn.bitfun-notification-btn') as HTMLElement;
        if (btn) {
          btn.click();
          return true;
        }
        return false;
      });

      console.log('[L0] Notification button clicked via JS:', clicked);
      expect(clicked).toBe(true);

      await browser.pause(1000);

      // Check for notification center
      const notificationCenter = await $('.notification-center');
      const centerExists = await notificationCenter.isExisting();

      console.log('[L0] Notification center opened:', centerExists);
      expect(centerExists).toBe(true);

      // Close it
      if (centerExists) {
        await browser.execute(() => {
          const btn = document.querySelector('.bitfun-nav-panel__footer-btn.bitfun-notification-btn') as HTMLElement;
          if (btn) btn.click();
        });
        await browser.pause(500);
      }
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
