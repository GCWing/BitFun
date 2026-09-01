/**
 * L1 Chat input contract controls: validates the native Desktop composer order
 * and Assistant-specific Harness visibility without replacing product state.
 */

import { browser, expect, $, $$ } from '@wdio/globals';
import { Header } from '../page-objects/components/Header';
import { StartupPage } from '../page-objects/StartupPage';
import { openWorkspace } from '../helpers/workspace-helper';
import { saveFailureScreenshot, saveStepScreenshot } from '../helpers/screenshot-utils';

describe('L1 Chat Input Contract Controls', () => {
  let hasWorkspace = false;
  let projectWorkspaceId: string | null = null;

  const openWorkspaceComposer = async (
    target: 'project' | 'assistant',
    preferredWorkspaceId?: string,
  ): Promise<boolean> => {
    const workspaceItems = await $$('[data-testid="nav-workspace-item"]');
    for (const workspaceItem of workspaceItems) {
      if (!(await workspaceItem.isDisplayed())) continue;

      const workspaceKind = (await workspaceItem.getAttribute('data-workspace-kind'))?.toLowerCase();
      const matchesTarget = target === 'assistant'
        ? workspaceKind === 'assistant'
        : Boolean(workspaceKind && workspaceKind !== 'assistant');
      if (!matchesTarget) continue;

      const workspaceId = await workspaceItem.getAttribute('data-workspace-id');
      if (preferredWorkspaceId && workspaceId !== preferredWorkspaceId) continue;
      if ((await workspaceItem.getAttribute('data-workspace-active')) !== 'true') {
        let workspaceName = await workspaceItem.$('[data-testid="nav-workspace-name-btn"]');
        await workspaceName.click();
        await browser.waitUntil(
          async () => (await workspaceItem.getAttribute('data-workspace-active')) === 'true',
          {
            timeout: 10000,
            interval: 200,
            timeoutMsg: `${target} workspace did not become active`,
          },
        );
        workspaceName = await workspaceItem.$('[data-testid="nav-workspace-name-btn"]');
      }

      // The row intentionally reveals its actions through hover/focus-within.
      // Give the now-active row real keyboard focus before using its menu.
      const activeWorkspaceName = await workspaceItem.$('[data-testid="nav-workspace-name-btn"]');
      await activeWorkspaceName.click();
      const workspaceMenuButton = await workspaceItem.$('[data-testid="nav-workspace-menu-btn"]');
      await workspaceMenuButton.waitForDisplayed({ timeout: 10000 });
      await workspaceMenuButton.click();
      const workspaceMenu = await $('[data-testid="nav-workspace-item-menu"]');
      await workspaceMenu.waitForDisplayed({ timeout: 10000 });
      if ((await workspaceMenu.getAttribute('data-workspace-id')) !== workspaceId) {
        throw new Error(`Workspace menu owner mismatch while opening the ${target} composer`);
      }
      const createSession = await workspaceMenu.$('[data-testid="nav-workspace-menu-create-session"]');
      await createSession.waitForDisplayed({ timeout: 10000 });
      await createSession.click();
      await $('[data-testid="chat-input-container"]').waitForDisplayed({ timeout: 15000 });

      if (target === 'project') projectWorkspaceId = workspaceId;
      return true;
    }

    return false;
  };

  before(async () => {
    const header = new Header();
    const startupPage = new StartupPage();

    await browser.pause(3000);
    await header.waitForLoad();

    hasWorkspace = !(await startupPage.isVisible());
    if (!hasWorkspace) {
      hasWorkspace = await openWorkspace();
    }

    if (hasWorkspace) hasWorkspace = await openWorkspaceComposer('project');
  });

  it('orders add, Harness, and the selected Agent mode from left to right', async function () {
    if (!hasWorkspace) {
      this.skip();
      return;
    }

    const addTrigger = await $('[data-testid="chat-input-agent-boost-trigger"]');
    const harnessTrigger = await $('[data-testid="harness-profile-selector"]');
    await addTrigger.waitForDisplayed({ timeout: 10000 });
    await harnessTrigger.waitForDisplayed({ timeout: 10000 });

    await addTrigger.click();
    const modeOptions = await $$([
      '[data-bf-boost-item-kind="mode"]',
      ':not([aria-disabled="true"])',
    ].join(''));
    let selectedAlternativeMode = false;
    for (const option of modeOptions) {
      const modeId = await option.getAttribute('data-bf-mode-id');
      if (modeId?.toLowerCase() === 'agentic') continue;
      await option.click();
      selectedAlternativeMode = true;
      break;
    }
    expect(selectedAlternativeMode).toBe(true);

    const modeChip = await $('[data-testid="chat-input-agent-mode-chip"]');
    await modeChip.waitForDisplayed({ timeout: 10000 });

    const layout = await browser.execute(() => {
      const add = document.querySelector<HTMLElement>('[data-testid="chat-input-agent-boost-trigger"]');
      const harness = document.querySelector<HTMLElement>('[data-testid="harness-profile-selector"]');
      const mode = document.querySelector<HTMLElement>('[data-testid="chat-input-agent-mode-chip"]');
      if (!add || !harness || !mode) return null;

      const follows = (left: Node, right: Node) =>
        Boolean(left.compareDocumentPosition(right) & Node.DOCUMENT_POSITION_FOLLOWING);
      return {
        semanticOrder: follows(add, harness) && follows(harness, mode),
        addX: add.getBoundingClientRect().left,
        harnessX: harness.getBoundingClientRect().left,
        modeX: mode.getBoundingClientRect().left,
      };
    });

    expect(layout).not.toBeNull();
    expect(layout?.semanticOrder).toBe(true);
    expect(layout!.addX).toBeLessThan(layout!.harnessX);
    expect(layout!.harnessX).toBeLessThan(layout!.modeX);
    await saveStepScreenshot('l1-chat-input-contract-controls-order');

    const removeMode = await modeChip.$('[data-bf-part="modeChipRemove"]');
    if (await removeMode.isExisting()) {
      await removeMode.click();
      await modeChip.waitForExist({ timeout: 10000, reverse: true });
    }
  });

  it('activates Creative Harness with the standard catalog icon', async function () {
    if (!hasWorkspace) {
      this.skip();
      return;
    }

    const harnessTrigger = await $('[data-testid="harness-profile-selector"]');
    await harnessTrigger.waitForDisplayed({ timeout: 10000 });
    await harnessTrigger.click();

    const creativeProfile = await $('[data-testid="harness-profile-creative"]');
    await creativeProfile.waitForDisplayed({ timeout: 10000 });
    expect(await creativeProfile.getAttribute('data-bf-state')).toBe('available');

    const creativeIcon = await creativeProfile.$('[data-bf-name="creative"]');
    expect(await creativeIcon.isDisplayed()).toBe(true);
    await saveStepScreenshot('l1-chat-input-contract-controls-creative-harness');

    await creativeProfile.click();
    await creativeProfile.waitForExist({ timeout: 10000, reverse: true });
    expect(await harnessTrigger.getText()).toContain('Creative');
  });

  it('does not expose Harness configuration in an Assistant composer', async function () {
    if (!hasWorkspace) {
      this.skip();
      return;
    }

    if (!(await openWorkspaceComposer('assistant'))) {
      console.log('[L1] No Assistant workspace is available; skipping Assistant composer assertion');
      this.skip();
      return;
    }

    const chatInput = await $('[data-testid="chat-input-container"]');
    await chatInput.waitForDisplayed({ timeout: 15000 });
    await browser.waitUntil(
      async () => !(await $('[data-testid="harness-profile-selector"]').isExisting()),
      {
        timeout: 10000,
        interval: 200,
        timeoutMsg: 'Harness selector remained visible in the Assistant composer',
      },
    );

    expect(await $('[data-testid="chat-input-agent-boost-trigger"]').isDisplayed()).toBe(true);
    expect(await $('[data-testid="harness-profile-selector"]').isExisting()).toBe(false);
    await saveStepScreenshot('l1-chat-input-contract-controls-assistant');
  });

  afterEach(async function () {
    if (this.currentTest?.state === 'failed') {
      await saveFailureScreenshot(`l1-chat-input-contract-controls-${this.currentTest.title}`);
    }
  });

  after(async () => {
    if (!projectWorkspaceId) return;

    const projectWorkspace = await $([
      '[data-testid="nav-workspace-item"]',
      `[data-workspace-id="${projectWorkspaceId}"]`,
    ].join(''));
    if (await projectWorkspace.isExisting()) {
      await openWorkspaceComposer('project', projectWorkspaceId);
    }
  });
});
