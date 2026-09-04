/**
 * L0 global-search spec: verifies the real desktop app's overview and in-dialog
 * project drilldown using opened workspace records rather than mocked data.
 */

import { browser, expect, $ } from '@wdio/globals';
import { mkdtemp, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { openWorkspaceThroughFrontend } from '../helpers/workspace-helper';
import { saveStepScreenshot } from '../helpers/screenshot-utils';

describe('L0 Global Search', () => {
  let fixtureRoot = '';

  before(async () => {
    fixtureRoot = await mkdtemp(path.join(tmpdir(), 'openbitfun-global-search-e2e-'));
    const fixturePaths = ['project-alpha', 'project-beta', 'project-gamma']
      .map((name) => path.join(fixtureRoot, name));

    for (const fixturePath of fixturePaths) {
      await mkdir(fixturePath, { recursive: true });
      await openWorkspaceThroughFrontend(fixturePath);
    }
  });

  it('presents the left-navigation search trigger as secondary hint text', async () => {
    const searchTrigger = await $('[data-testid="nav-search-trigger"]');
    await searchTrigger.waitForDisplayed({ timeout: 10000 });

    const searchLabel = await searchTrigger.$('.openbitfun-nav-panel__search-trigger__label');
    expect(await searchLabel.getText()).toMatch(/^(搜索一切|搜尋一切|Search everything)$/);

    const shortcutHint = await searchTrigger.$('[data-testid="nav-search-shortcut"]');
    expect(await shortcutHint.isDisplayed()).toBe(true);
    expect(await shortcutHint.getText()).toMatch(/^(Ctrl\+K|⌘K)$/);
    expect(await shortcutHint.getAttribute('aria-hidden')).toBe('true');

    const presentation = await browser.execute(() => {
      const label = document.querySelector<HTMLElement>('.openbitfun-nav-panel__search-trigger__label');
      if (!label) return null;

      const style = window.getComputedStyle(label);
      return {
        color: style.color,
        fontWeight: style.fontWeight,
      };
    });

    expect(presentation).not.toBeNull();
    expect(presentation?.color).toBeTruthy();
    expect(presentation?.fontWeight).toBe('400');
    await saveStepScreenshot('l0-global-search-trigger-secondary');
  });

  it('shows two projects by default and opens the complete project list in the same dialog', async () => {
    const searchTrigger = await $('[data-testid="nav-search-trigger"]');
    await searchTrigger.click();

    const dialog = await $('[data-testid="global-search-dialog"]');
    await dialog.waitForDisplayed({ timeout: 10000 });
    const overlayPresentation = await browser.execute(() => {
      const overlay = document.querySelector<HTMLElement>('.modal-overlay.global-search-overlay');
      if (!overlay) return null;

      const style = window.getComputedStyle(overlay);
      return {
        backgroundColor: style.backgroundColor,
        backdropFilter: style.backdropFilter,
      };
    });
    expect(overlayPresentation).not.toBeNull();
    expect(['rgba(0, 0, 0, 0)', 'transparent']).toContain(overlayPresentation?.backgroundColor);
    expect(overlayPresentation?.backdropFilter).toBe('none');
    expect(await dialog.$('.global-search__prefix-hint').isExisting()).toBe(false);
    expect(await dialog.$('.global-search__footer-status').isExisting()).toBe(false);

    const overviewGroup = await $('[data-testid="global-search-group-workspaces"]');
    await overviewGroup.waitForDisplayed({ timeout: 10000 });
    expect((await overviewGroup.$$('.global-search__result')).length).toBe(2);

    const projectDrilldown = await $('[data-testid="global-search-group-drilldown-workspaces"]');
    expect(await projectDrilldown.isDisplayed()).toBe(true);
    await saveStepScreenshot('l0-global-search-project-overview');

    await projectDrilldown.click();
    const projectPage = await $('[data-testid="global-search-group-page-workspaces"]');
    await projectPage.waitForDisplayed({ timeout: 10000 });
    expect((await projectPage.$$('.global-search__result')).length).toBeGreaterThanOrEqual(3);
    expect(await dialog.isDisplayed()).toBe(true);
    await saveStepScreenshot('l0-global-search-project-list');

    const backButton = await $('[data-testid="global-search-group-back"]');
    await backButton.click();
    await overviewGroup.waitForDisplayed({ timeout: 10000 });
    expect((await overviewGroup.$$('.global-search__result')).length).toBe(2);
  });

  after(async () => {
    if (!fixtureRoot) return;

    await browser.execute(async (fixturePrefix: string) => {
      const invoke = window.__TAURI__?.core?.invoke;
      if (typeof invoke !== 'function') return;

      const workspaces = await invoke('get_opened_workspaces', { request: {} }) as Array<{
        id?: string;
        rootPath?: string;
      }>;
      for (const workspace of workspaces) {
        if (workspace.id && workspace.rootPath?.startsWith(fixturePrefix)) {
          await invoke('close_workspace', { request: { workspaceId: workspace.id } });
        }
      }
    }, fixtureRoot).catch(() => undefined);

    await rm(fixtureRoot, { recursive: true, force: true });
  });
});
