/**
 * L0 navigation spec: verifies sidebar navigation panel exists and items are visible.
 * Basic checks that navigation structure is present - no AI interaction needed.
 */

import { browser, expect, $ } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';
import { saveElementScreenshot, saveStepScreenshot } from '../helpers/screenshot-utils';

const NAV_ENTRY_SELECTORS = [
  '.bitfun-nav-panel__item',
  '.bitfun-nav-panel__workspace-item-name-btn',
  '.bitfun-nav-panel__inline-item',
  '.bitfun-nav-panel__workspace-create-main',
  '.bitfun-nav-panel__miniapp-entry',
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

      hasWorkspace = await openWorkspace(undefined, { requireWorkspaceLabel: false });

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
    it('should use the native session-context icon for the session group menu', async function () {
      expect(hasWorkspace).toBe(true);

      const sessionGroupMenuButton = await $('[data-testid="nav-workspace-add-btn"]');
      await sessionGroupMenuButton.waitForDisplayed({ timeout: 10000 });

      const sessionContextIcon = await sessionGroupMenuButton.$(
        'svg[data-bf-icon="navigation-session-context-add"][data-bf-source="bitfun-svg"]',
      );
      const lucideFolderOpenIcon = await sessionGroupMenuButton.$('svg.lucide-folder-open');
      const plainPlusIcon = await sessionGroupMenuButton.$('svg.lucide-plus');
      expect(await sessionContextIcon.isDisplayed()).toBe(true);
      expect(await lucideFolderOpenIcon.isExisting()).toBe(false);
      expect(await plainPlusIcon.isExisting()).toBe(false);
      await saveStepScreenshot('l0-navigation-session-context-add-icon');
    });

    it('should switch the native session-view icon between grouped and all', async function () {
      expect(hasWorkspace).toBe(true);

      const viewToggle = await $('[data-testid="nav-workspace-session-view-toggle"]');
      await viewToggle.waitForDisplayed({ timeout: 10000 });

      if (await viewToggle.getAttribute('data-view-mode') === 'all') {
        await viewToggle.click();
      }
      await browser.waitUntil(
        async () => await viewToggle.getAttribute('data-view-mode') === 'grouped',
        { timeout: 5000, timeoutMsg: 'Session view did not enter grouped mode' },
      );

      expect(await viewToggle.getAttribute('aria-pressed')).toBe('false');
      expect(await viewToggle.$('svg[data-bf-icon="navigation-session-view-grouped"]').isDisplayed()).toBe(true);
      expect(await viewToggle.$('svg[data-bf-icon="navigation-session-view-all"]').isExisting()).toBe(false);
      await saveElementScreenshot(
        '[data-testid="nav-workspace-session-view-toggle"]',
        'l0-navigation-session-view-grouped-icon',
      );

      await viewToggle.click();
      await browser.waitUntil(
        async () => await viewToggle.getAttribute('data-view-mode') === 'all',
        { timeout: 5000, timeoutMsg: 'Session view did not enter all mode' },
      );

      expect(await viewToggle.getAttribute('aria-pressed')).toBe('true');
      expect(await viewToggle.$('svg[data-bf-icon="navigation-session-view-all"]').isDisplayed()).toBe(true);
      expect(await viewToggle.$('svg[data-bf-icon="navigation-session-view-grouped"]').isExisting()).toBe(false);
      expect(await $('.bitfun-nav-panel__workspace-all-sessions-header').isExisting()).toBe(false);
      await saveElementScreenshot(
        '[data-testid="nav-workspace-session-view-toggle"]',
        'l0-navigation-session-view-all-icon',
      );
      await saveStepScreenshot('l0-navigation-session-view-all-icon');

      await viewToggle.click();
      await browser.waitUntil(
        async () => await viewToggle.getAttribute('data-view-mode') === 'grouped',
        { timeout: 5000, timeoutMsg: 'Session view did not restore grouped mode' },
      );
    });

    it('should render the active session group with its filled native selected icon', async function () {
      expect(hasWorkspace).toBe(true);

      const activeGroup = await $('[data-testid="nav-workspace-item"][data-workspace-active="true"]');
      await activeGroup.waitForDisplayed({ timeout: 10000 });

      const selectedIcon = await activeGroup.$(
        'svg[data-bf-source="bitfun-svg"][data-bf-icon$="-selected"]',
      );
      await selectedIcon.waitForDisplayed({ timeout: 10000 });

      const selectedIconName = await selectedIcon.getAttribute('data-bf-icon');
      expect([
        'session-group-assistant-selected',
        'session-group-remote-workspace-selected',
        'session-group-workspace-selected',
      ]).toContain(selectedIconName);
      expect(await activeGroup.$(
        'svg[data-bf-icon="session-group-assistant"], '
        + 'svg[data-bf-icon="session-group-remote-workspace"], '
        + 'svg[data-bf-icon="session-group-workspace"]',
      ).isExisting()).toBe(false);

      await saveElementScreenshot(
        '[data-testid="nav-workspace-item"][data-workspace-active="true"]',
        'l0-navigation-selected-session-group-icon',
      );
    });

    it('should align nested session titles with the active workspace or assistant label', async function () {
      expect(hasWorkspace).toBe(true);

      const activeGroup = await $('[data-testid="nav-workspace-item"][data-workspace-active="true"]');
      const groupLabel = await activeGroup.$('[data-bf-part="label"]');
      const sessionLabel = await activeGroup.$(
        '[data-testid="nav-session-item"][data-session-level="0"] .bitfun-nav-panel__inline-item-label',
      );
      await groupLabel.waitForDisplayed({ timeout: 10000 });
      await sessionLabel.waitForDisplayed({ timeout: 10000 });

      const groupLabelX = await groupLabel.getLocation('x');
      const sessionLabelX = await sessionLabel.getLocation('x');
      console.log('[L0] Grouped session alignment:', { groupLabelX, sessionLabelX });
      expect(Math.abs(sessionLabelX - groupLabelX)).toBeLessThanOrEqual(1);
      await saveStepScreenshot('l0-navigation-grouped-session-alignment');
    });

    it('should align extension child icons with the parent label', async function () {
      expect(hasWorkspace).toBe(true);

      const extensionsEntry = await $('[data-testid="agent-skill-entry"]');
      await extensionsEntry.waitForDisplayed({ timeout: 10000 });
      if (await extensionsEntry.getAttribute('aria-expanded') !== 'true') {
        await extensionsEntry.click();
      }

      await browser.waitUntil(
        async () => await extensionsEntry.getAttribute('aria-expanded') === 'true',
        { timeout: 5000, timeoutMsg: 'Extensions group did not expand' },
      );

      const alignment = await browser.execute(() => {
        const parent = document.querySelector<HTMLElement>('[data-testid="agent-skill-entry"]');
        const parentLabel = parent?.querySelector<HTMLElement>(':scope > span:last-child');
        const childIcons = Array.from(document.querySelectorAll<SVGGraphicsElement>(
          '[data-testid="agent-skill-tabs"] .bitfun-nav-panel__top-action-icon-slot > svg',
        ));
        if (!parentLabel || childIcons.length === 0) {
          return null;
        }

        return {
          parentLabelLeft: parentLabel.getBoundingClientRect().left,
          childIconLefts: childIcons.map(icon => icon.getBoundingClientRect().left),
        };
      });

      expect(alignment).not.toBeNull();
      if (!alignment) {
        return;
      }

      expect(alignment.childIconLefts).toHaveLength(3);
      for (const childIconLeft of alignment.childIconLefts) {
        expect(Math.abs(childIconLeft - alignment.parentLabelLeft)).toBeLessThanOrEqual(1);
      }
      console.log('[L0] Extension child icon alignment:', alignment);
      await saveStepScreenshot('l0-navigation-extension-child-icon-alignment');
    });

    it('should place Task Board below AI Assistant and open it', async function () {
      expect(hasWorkspace).toBe(true);

      const assistantManager = await $('[data-testid="nav-assistant-manager"]');
      const taskBoard = await $('[data-testid="nav-todos-btn"]');
      const miniApps = await $('.bitfun-nav-panel__miniapp-entry');
      await assistantManager.waitForDisplayed({ timeout: 10000 });
      await taskBoard.waitForDisplayed({ timeout: 10000 });
      await miniApps.waitForDisplayed({ timeout: 10000 });

      expect(await assistantManager.getLocation('y')).toBeLessThan(await taskBoard.getLocation('y'));
      expect(await taskBoard.getLocation('y')).toBeLessThan(await miniApps.getLocation('y'));
      expect(['任务看板', 'Task Board', '任務看板']).toContain((await taskBoard.getText()).trim());
      expect(await $('[data-bf-part="footer"] [data-testid="nav-todos-btn"]').isExisting()).toBe(false);

      await taskBoard.click();
      const todosScene = await $('[data-testid="todos-scene"]');
      await todosScene.waitForDisplayed({ timeout: 10000 });
      const taskBoardTitle = await todosScene.$('.bf-todos__title');
      expect(await taskBoard.getAttribute('aria-pressed')).toBe('true');
      expect(await todosScene.isDisplayed()).toBe(true);
      expect(['任务看板', 'Task Board', '任務看板']).toContain((await taskBoardTitle.getText()).trim());
      await saveStepScreenshot('l0-navigation-task-board');
    });

    it('should open assistant management from the item above Task Board', async function () {
      expect(hasWorkspace).toBe(true);

      const assistantManager = await $('[data-testid="nav-assistant-manager"]');
      const taskBoard = await $('[data-testid="nav-todos-btn"]');
      await assistantManager.waitForDisplayed({ timeout: 10000 });
      await taskBoard.waitForDisplayed({ timeout: 10000 });

      expect(await assistantManager.getLocation('y')).toBeLessThan(await taskBoard.getLocation('y'));

      await assistantManager.click();
      const assistantScene = await $('[data-bf-scene="assistant"][data-bf-part="root"]');
      const assistantGallery = await $('[data-bf-component="nursery-gallery"][data-bf-part="root"]');
      await assistantScene.waitForDisplayed({ timeout: 10000 });
      await assistantGallery.waitForDisplayed({ timeout: 10000 });
      expect(await assistantScene.isDisplayed()).toBe(true);
      expect(await assistantGallery.isDisplayed()).toBe(true);
      await saveStepScreenshot('l0-navigation-assistant-management');
    });

    it('should expose the settings utility list without More or Insights', async function () {
      expect(hasWorkspace).toBe(true);

      const deviceStatus = await $('[data-testid="nav-footer-device-status"]');
      const settingsButton = await $('[data-testid="nav-footer-settings-item"]');
      expect(await deviceStatus.isDisplayed()).toBe(true);
      expect(await settingsButton.isDisplayed()).toBe(true);
      expect(await $('[data-testid="shell-panel-entry"]').isExisting()).toBe(false);
      expect(await $('[data-testid="browser-panel-entry"]').isExisting()).toBe(false);
      expect(await $('[data-testid="nav-footer-more-btn"]').isExisting()).toBe(false);

      await settingsButton.click();
      const settingsMenu = await $('[data-testid="nav-settings-menu"]');
      await settingsMenu.waitForDisplayed({ timeout: 10000 });

      const expectedItems = [
        'nav-settings-floating-item',
        'notification-button',
        'nav-settings-theme-item',
        'nav-settings-open-item',
        'nav-settings-about-item',
      ];
      const renderedItems = await settingsMenu.$$('[role="menuitem"]');
      expect(renderedItems).toHaveLength(expectedItems.length);
      for (let index = 0; index < expectedItems.length; index += 1) {
        expect(await renderedItems[index].getAttribute('data-testid')).toBe(expectedItems[index]);
      }

      const menuText = await settingsMenu.getText();
      expect(['洞察', 'Insights'].some(label => menuText.includes(label))).toBe(false);
      await saveStepScreenshot('l0-navigation-settings-utility-list');

      const backdrop = await $('.bitfun-nav-panel__footer-backdrop');
      await backdrop.click();
      await backdrop.waitForExist({ reverse: true, timeout: 2000 });
    });

    it('should keep the session divider full-width and remove the footer divider', async function () {
      expect(hasWorkspace).toBe(true);

      const dividerLayout = await browser.execute(() => {
        const panel = document.querySelector<HTMLElement>('.bitfun-nav-panel');
        const sections = document.querySelector<HTMLElement>('.bitfun-nav-panel__sections');
        const topActions = document.querySelector<HTMLElement>('.bitfun-nav-panel__top-actions');
        const stickyHeader = document.querySelector<HTMLElement>('.bitfun-nav-panel__sticky-section-header');
        const footer = document.querySelector<HTMLElement>('.bitfun-nav-panel__footer');
        if (!panel || !sections || !topActions || !stickyHeader || !footer) {
          return null;
        }

        const panelRect = panel.getBoundingClientRect();
        const sectionsRect = sections.getBoundingClientRect();
        const topActionsRect = topActions.getBoundingClientRect();
        const dividerStyle = window.getComputedStyle(topActions, '::after');
        const stickyDividerStyle = window.getComputedStyle(stickyHeader, '::after');
        const footerStyle = window.getComputedStyle(footer);
        const dividerLeft = topActionsRect.left + Number.parseFloat(dividerStyle.left);
        const dividerRight = dividerLeft + Number.parseFloat(dividerStyle.width);

        return {
          dividerBorderStyle: dividerStyle.borderBottomStyle,
          dividerBorderWidth: dividerStyle.borderBottomWidth,
          dividerLeft,
          dividerRight,
          panelLeft: panelRect.left,
          panelRight: panelRect.right,
          sectionsRight: sectionsRect.right,
          hasVerticalScrollbar: sections.scrollHeight > sections.clientHeight,
          stickyDividerBorderStyle: stickyDividerStyle.borderBottomStyle,
          stickyDividerBorderWidth: stickyDividerStyle.borderBottomWidth,
          footerBorderStyle: footerStyle.borderTopStyle,
          footerBorderWidth: footerStyle.borderTopWidth,
        };
      });

      expect(dividerLayout).not.toBeNull();
      if (!dividerLayout) {
        return;
      }

      expect(dividerLayout.dividerBorderStyle).toBe('dashed');
      expect(dividerLayout.dividerBorderWidth).toBe('1px');
      expect(Math.abs(dividerLayout.dividerLeft - dividerLayout.panelLeft)).toBeLessThanOrEqual(1);
      expect(dividerLayout.dividerRight).toBeGreaterThanOrEqual(dividerLayout.panelRight - 1);
      expect(dividerLayout.dividerRight).toBeGreaterThanOrEqual(dividerLayout.sectionsRight - 1);
      expect(dividerLayout.stickyDividerBorderStyle).toBe('dashed');
      expect(dividerLayout.stickyDividerBorderWidth).toBe('1px');
      expect(dividerLayout.footerBorderStyle).toBe('none');
      expect(dividerLayout.footerBorderWidth).toBe('0px');
      console.log('[L0] Navigation divider layout:', dividerLayout);
    });

    it('should align navigation scroll and resize controls with the scene border', async function () {
      expect(hasWorkspace).toBe(true);

      const boundaryLayout = await browser.execute(() => {
        const navArea = document.querySelector<HTMLElement>('.bitfun-workspace-body__nav-area');
        const panel = document.querySelector<HTMLElement>('.bitfun-nav-panel');
        const sections = document.querySelector<HTMLElement>('.bitfun-nav-panel__sections');
        const divider = document.querySelector<HTMLElement>('.bitfun-workspace-body__nav-divider');
        const sceneViewport = document.querySelector<HTMLElement>('.bitfun-scene-viewport');
        if (!navArea || !panel || !sections || !divider || !sceneViewport) {
          return null;
        }

        const navAreaRect = navArea.getBoundingClientRect();
        const panelRect = panel.getBoundingClientRect();
        const sectionsRect = sections.getBoundingClientRect();
        const dividerRect = divider.getBoundingClientRect();
        const sceneViewportRect = sceneViewport.getBoundingClientRect();
        const dividerLineStyle = window.getComputedStyle(divider, '::after');
        const dividerLineLeft = Number.parseFloat(dividerLineStyle.left);

        return {
          navAreaRight: navAreaRect.right,
          panelRight: panelRect.right,
          sectionsRight: sectionsRect.right,
          dividerLeft: dividerRect.left,
          dividerRight: dividerRect.right,
          dividerLineCenter: dividerRect.left + dividerLineLeft,
          sceneLeft: sceneViewportRect.left,
        };
      });

      expect(boundaryLayout).not.toBeNull();
      if (!boundaryLayout) {
        return;
      }

      // Preserve the workbench breathing room instead of moving the scene
      // border left to meet controls that were positioned against navArea.
      expect(boundaryLayout.sceneLeft - boundaryLayout.navAreaRight).toBeGreaterThan(0);
      expect(Math.abs(boundaryLayout.panelRight - boundaryLayout.sceneLeft)).toBeLessThanOrEqual(1);
      expect(Math.abs(boundaryLayout.sectionsRight - boundaryLayout.sceneLeft)).toBeLessThanOrEqual(1);
      expect(Math.abs(boundaryLayout.dividerLeft - boundaryLayout.sceneLeft)).toBeLessThanOrEqual(1);
      expect(Math.abs(boundaryLayout.dividerLineCenter - boundaryLayout.sceneLeft)).toBeLessThanOrEqual(1);
      expect(boundaryLayout.dividerRight).toBeGreaterThan(boundaryLayout.sceneLeft);
      console.log('[L0] Navigation boundary alignment:', boundaryLayout);
      await saveStepScreenshot('l0-navigation-boundary-alignment');
    });

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
