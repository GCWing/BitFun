/**
 * L0 navigation spec: verifies sidebar navigation panel exists and items are visible.
 * Basic checks that navigation structure is present - no AI interaction needed.
 */

import { browser, expect, $ } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';

const NAV_ENTRY_SELECTORS = [
  '.bitfun-nav-panel__item',
  '.bitfun-nav-panel__workspace-item-name-btn',
  '.bitfun-nav-panel__inline-item',
  '.bitfun-nav-panel__workspace-create-main',
  '.bitfun-nav-panel__live-app-entry',
];

async function getNavigationEntries() {
  const entries = [];

  for (const selector of NAV_ENTRY_SELECTORS) {
    const matched = await browser.$$(selector);
    entries.push(...matched);
  }

  return entries;
}

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

      hasWorkspace = await openWorkspace();

      console.log('[L0] Workspace opened:', hasWorkspace);
      expect(hasWorkspace).toBe(true);
    });

    it('should have navigation panel or sidebar when workspace is open', async function () {
      expect(hasWorkspace).toBe(true);

      await browser.pause(1000);

      // Use the correct selector from NavPanel.tsx
      const navPanel = await $('.bitfun-nav-panel');
      const navExists = await navPanel.isExisting();

      console.log('[L0] Navigation panel found:', navExists);
      expect(navExists).toBe(true);
    });
  });

  describe('Navigation items visibility', () => {
    it('navigation items should be present if workspace is open', async function () {
      expect(hasWorkspace).toBe(true);

      await browser.pause(500);

      const navItems = await getNavigationEntries();
      const itemCount = navItems.length;

      console.log(`[L0] Found ${itemCount} navigation items`);
      expect(itemCount).toBeGreaterThan(0);
    });

    it('navigation sections should be present', async function () {
      expect(hasWorkspace).toBe(true);

      // Use correct selector from MainNav.tsx
      const sections = await $('.bitfun-nav-panel__sections');
      const sectionsExist = await sections.isExisting();

      console.log('[L0] Navigation sections found:', sectionsExist);
      expect(sectionsExist).toBe(true);
    });
  });

  describe('Navigation interactivity', () => {
    it('navigation items should be clickable', async function () {
      expect(hasWorkspace).toBe(true);

      const navItems = await getNavigationEntries();

      expect(navItems.length).toBeGreaterThan(0);

      let firstItem = null;
      for (const item of navItems) {
        try {
          if (await item.isClickable()) {
            firstItem = item;
            break;
          }
        } catch {
          // Continue to the next navigation entry.
        }
      }

      expect(firstItem).not.toBeNull();
      if (!firstItem) {
        return;
      }

      const isClickable = await firstItem.isClickable();
      console.log('[L0] First nav item clickable:', isClickable);

      expect(isClickable).toBe(true);
    });
  });

  after(async () => {
    console.log('[L0] Navigation tests complete');
  });
});
