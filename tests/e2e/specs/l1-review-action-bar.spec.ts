/**
 * L1 Review Action Bar spec: validates review action bar minimize/restore
 * and continuation behavior for code review / deep review sessions.
 */

import { browser, expect, $ } from '@wdio/globals';
import { Header } from '../page-objects/components/Header';
import { StartupPage } from '../page-objects/StartupPage';
import { ensureWorkspaceOpen } from '../helpers/workspace-utils';

describe('L1 Review Action Bar', () => {
  let header: Header;
  let startupPage: StartupPage;
  let hasWorkspace = false;

  before(async () => {
    header = new Header();
    startupPage = new StartupPage();
    await browser.pause(3000);
    await header.waitForLoad();
    hasWorkspace = await ensureWorkspaceOpen(startupPage);
    if (!hasWorkspace) {
      console.log('[L1] No workspace available - review action bar tests will be skipped');
    }
  });

  describe('Minimize and restore', () => {
    it('should minimize review action bar when close button is clicked', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      // Wait for any review action bar to appear
      const actionBar = await $('.deep-review-action-bar');
      if (!(await actionBar.isExisting())) {
        console.log('[L1] No review action bar visible - skipping minimize test');
        this.skip();
        return;
      }

      const closeButton = await actionBar.$('.deep-review-action-bar__close');
      expect(await closeButton.isExisting()).toBe(true);

      await closeButton.click();
      await browser.pause(500);

      // Action bar should be hidden (minimized)
      expect(await actionBar.isDisplayed()).toBe(false);

      // Minimized indicator should appear
      const indicator = await $('.btw-session-panel__minimized-indicator');
      expect(await indicator.isExisting()).toBe(true);
      expect(await indicator.isDisplayed()).toBe(true);
    });

    it('should restore review action bar when minimized indicator is clicked', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const indicator = await $('.btw-session-panel__minimized-indicator');
      if (!(await indicator.isExisting())) {
        console.log('[L1] No minimized indicator visible - skipping restore test');
        this.skip();
        return;
      }

      const restoreButton = await indicator.$('button');
      await restoreButton.click();
      await browser.pause(500);

      // Action bar should be visible again
      const actionBar = await $('.deep-review-action-bar');
      expect(await actionBar.isDisplayed()).toBe(true);

      // Indicator should be hidden
      expect(await indicator.isDisplayed()).toBe(false);
    });

    it('should show remaining count in minimized indicator', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      // This test requires an active review action bar
      const actionBar = await $('.deep-review-action-bar');
      if (!(await actionBar.isExisting())) {
        console.log('[L1] No review action bar visible - skipping count test');
        this.skip();
        return;
      }

      const closeButton = await actionBar.$('.deep-review-action-bar__close');
      await closeButton.click();
      await browser.pause(500);

      const indicator = await $('.btw-session-panel__minimized-indicator');
      const countEl = await indicator.$('.btw-session-panel__minimized-count');
      expect(await countEl.isExisting()).toBe(true);

      const countText = await countEl.getText();
      // Should be in format "X/Y"
      expect(countText).toMatch(/^\d+\/\d+$/);

      // Restore for other tests
      const restoreButton = await indicator.$('button');
      await restoreButton.click();
      await browser.pause(500);
    });
  });

  describe('Completed items state', () => {
    it('should mark completed items with strikethrough and disabled checkbox', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      const actionBar = await $('.deep-review-action-bar');
      if (!(await actionBar.isExisting())) {
        console.log('[L1] No review action bar visible - skipping completed items test');
        this.skip();
        return;
      }

      // Look for completed items
      const completedItems = await actionBar.$$('.deep-review-action-bar__remediation-item--completed');
      if (completedItems.length === 0) {
        console.log('[L1] No completed items visible - skipping');
        this.skip();
        return;
      }

      // Verify completed item styling
      const firstCompleted = completedItems[0];
      const checkbox = await firstCompleted.$('input[type="checkbox"]');
      expect(await checkbox.getAttribute('disabled')).toBe('true');

      const text = await firstCompleted.$('.deep-review-action-bar__remediation-text');
      const textDecoration = await text.getCSSProperty('text-decoration');
      expect(textDecoration.value).toContain('line-through');
    });
  });

  describe('Fix interruption continuation', () => {
    it('should show continue fix button when fix was interrupted', async function () {
      if (!hasWorkspace) {
        this.skip();
        return;
      }

      // This test requires the action bar to be in fix_interrupted state
      // In a real scenario, this would be triggered by an interrupted fix
      const actionBar = await $('.deep-review-action-bar');
      if (!(await actionBar.isExisting())) {
        console.log('[L1] No review action bar visible - skipping interruption test');
        this.skip();
        return;
      }

      // Check if the action bar has interruption notice
      const interruptionNotice = await actionBar.$('.deep-review-action-bar__interruption-notice');
      if (!(await interruptionNotice.isExisting())) {
        console.log('[L1] No interruption notice visible - skipping');
        this.skip();
        return;
      }

      // Verify continue fix button exists
      const continueButton = await actionBar.$('button*=Continue fixing');
      expect(await continueButton.isExisting()).toBe(true);

      // Verify skip button exists
      const skipButton = await actionBar.$('button*=Skip remaining');
      expect(await skipButton.isExisting()).toBe(true);
    });
  });
});
