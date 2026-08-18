/**
 * Native Desktop coverage for the persistent device overview.
 *
 * This spec intentionally does not replace transport APIs or inject state.
 * Multi-device projections are covered by pure model tests; live
 * phone, peer-device, and dispatch behavior requires real counterparts.
 */

import { $, $$, browser, expect } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';
import { expectPopupCloseContract } from '../helpers/popup-close-contract';
import { saveElementScreenshot, saveStepScreenshot } from '../helpers/screenshot-utils';

async function ensureLightAppearance(): Promise<void> {
  const isLight = await browser.execute(() => (
    document.documentElement.getAttribute('data-bf-appearance') === 'bitfun-light'
  ));
  if (isLight) return;

  const settings = await $('[data-testid="nav-footer-settings-item"]');
  await settings.click();
  const themeConfiguration = await $('[data-testid="nav-settings-theme-item"]');
  await themeConfiguration.waitForDisplayed({ timeout: 10_000 });
  await themeConfiguration.click();

  const picker = await $('[data-testid="appearance-palette-select"]');
  await picker.waitForDisplayed({ timeout: 10_000 });
  await picker.click();
  const lightOption = await $('[data-testid="appearance-palette-option"][data-appearance-id="bitfun-light"]');
  await lightOption.waitForDisplayed({ timeout: 10_000 });
  await lightOption.click();

  await browser.waitUntil(async () => browser.execute(() => (
    document.documentElement.getAttribute('data-bf-appearance') === 'bitfun-light'
  )), {
    timeout: 10_000,
    timeoutMsg: 'The native app did not switch to the light Appearance',
  });
}

async function openLocalOverview(): Promise<WebdriverIO.Element> {
  const trigger = await $('[data-testid="nav-footer-device-status"]');
  await trigger.waitForDisplayed({ timeout: 15_000 });
  await browser.waitUntil(
    async () => (await trigger.getAttribute('data-bf-state')) === 'local',
    {
      timeout: 15_000,
      timeoutMsg: 'The isolated Desktop profile did not settle into local device mode',
    },
  );
  if ((await trigger.getAttribute('aria-expanded')) !== 'true') await trigger.click();

  const popover = await $('[data-testid="nav-device-status-popover"]');
  await popover.waitForDisplayed({ timeout: 10_000 });
  return popover;
}

async function closeOverviewIfOpen(): Promise<void> {
  const popover = await $('[data-testid="nav-device-status-popover"]');
  if (!await popover.isExisting()) return;
  const backdrop = await $('[data-testid="nav-device-status-backdrop"]');
  if (await backdrop.isExisting()) await backdrop.click();
  await popover.waitForExist({ reverse: true, timeout: 5_000 });
}

describe('L0 Device Overview', () => {
  it('presents local work as a complete, compact state', async () => {
    expect(await openWorkspace(undefined, { requireWorkspaceLabel: false })).toBe(true);
    await ensureLightAppearance();

    const trigger = await $('[data-testid="nav-footer-device-status"]');
    const settings = await $('[data-testid="nav-footer-settings-item"]');
    const footerLayout = await browser.execute(() => {
      const deviceStatus = document.querySelector<HTMLElement>('[data-testid="nav-footer-device-status"]');
      const settingsEntry = document.querySelector<HTMLElement>('[data-testid="nav-footer-settings-item"]');
      if (!deviceStatus || !settingsEntry) return null;
      const deviceRect = deviceStatus.getBoundingClientRect();
      const settingsRect = settingsEntry.getBoundingClientRect();
      return {
        horizontalGap: settingsRect.left - deviceRect.right,
        verticalCenterDelta: Math.abs(
          (settingsRect.top + (settingsRect.height / 2))
          - (deviceRect.top + (deviceRect.height / 2)),
        ),
        textAlign: window.getComputedStyle(deviceStatus).textAlign,
      };
    });

    expect(footerLayout).not.toBeNull();
    expect(footerLayout?.horizontalGap).toBeGreaterThanOrEqual(0);
    expect(footerLayout?.horizontalGap).toBeLessThanOrEqual(8);
    expect(footerLayout?.verticalCenterDelta).toBeLessThanOrEqual(1);
    expect(footerLayout?.textAlign).toBe('left');
    expect(await trigger.$('.lucide-chevron-down').isExisting()).toBe(false);
    expect(await settings.$('.lucide-chevron-up').isExisting()).toBe(false);

    const popover = await openLocalOverview();
    expect(await popover.getAttribute('data-bf-state')).toBe('local');
    expect(await $('[data-testid="nav-device-status-summary"]').isDisplayed()).toBe(true);
    expect(await $('[data-testid="nav-device-connection-service"]').isExisting()).toBe(false);
    expect(await $('[data-testid="nav-device-status-connected-devices"]').isExisting()).toBe(false);
    expect(await $('[data-testid="nav-device-status-manage"]').isDisplayed()).toBe(true);

    const overviewText = (await popover.getText()).toLowerCase();
    expect(overviewText).not.toContain('connection service');
    expect(overviewText).not.toContain('连接服务');
    expect(overviewText).not.toContain('連線服務');
    expect(overviewText).not.toContain('not connected');
    expect(overviewText).not.toContain('未连接');
    expect(overviewText).not.toContain('未連線');

    const actionLayout = await browser.execute(() => {
      const popoverElement = document.querySelector<HTMLElement>('[data-testid="nav-device-status-popover"]');
      const action = document.querySelector<HTMLElement>('[data-testid="nav-device-status-manage"]');
      if (!popoverElement || !action) return null;
      const popoverRect = popoverElement.getBoundingClientRect();
      const actionRect = action.getBoundingClientRect();
      return {
        outerLeftGap: actionRect.left - popoverRect.left,
        backgroundColor: window.getComputedStyle(action).backgroundColor,
        hovered: action.matches(':hover'),
        focusVisible: action.matches(':focus-visible'),
        justifyContent: window.getComputedStyle(action).justifyContent,
      };
    });
    expect(actionLayout?.outerLeftGap).toBeGreaterThanOrEqual(20);
    if (!actionLayout?.hovered && !actionLayout?.focusVisible) {
      expect(actionLayout?.backgroundColor).toBe('rgba(0, 0, 0, 0)');
    }
    expect(actionLayout?.justifyContent).toBe('center');

    await saveElementScreenshot(
      '[data-testid="nav-device-status-popover"]',
      'l0-device-overview-local-popover',
    );
    await saveStepScreenshot('l0-device-overview-local');
    await closeOverviewIfOpen();
  });

  it('opens the full connection configuration dialog without SSH', async () => {
    expect(await openWorkspace(undefined, { requireWorkspaceLabel: false })).toBe(true);
    await ensureLightAppearance();
    await openLocalOverview();

    await $('[data-testid="nav-device-status-manage"]').click();

    const agreeDisclaimer = await $('[data-testid="remote-connect-disclaimer-agree"]');
    if (await agreeDisclaimer.isExisting()) {
      await agreeDisclaimer.waitForDisplayed({ timeout: 5_000 });
      await agreeDisclaimer.click();
    }

    const dialog = await $('[data-bf-component="remote-connect-dialog"][data-bf-part="root"]');
    await dialog.waitForDisplayed({
      timeout: 10_000,
      timeoutMsg: 'The full Remote Connect dialog did not open',
    });
    await expectPopupCloseContract(
      '[data-bf-component="remote-connect-dialog"][data-bf-part="root"]',
    );
    expect(await $('[data-testid="nav-device-status-popover"]').isExisting()).toBe(false);

    const groupTabs = await $$('[data-bf-component="remote-connect-dialog"][data-bf-part="groupTab"]');
    expect(groupTabs).toHaveLength(3);
    expect(await $('#remote-connect-group-network').isDisplayed()).toBe(true);
    expect(await $('#remote-connect-group-bot').isDisplayed()).toBe(true);
    expect(await $('#remote-connect-group-account').isDisplayed()).toBe(true);

    expect(await $('#remote-connect-network-tab-lan').isDisplayed()).toBe(true);
    expect(await $('#remote-connect-network-tab-ngrok').isDisplayed()).toBe(true);
    expect(await $('#remote-connect-network-tab-bitfun_server').isDisplayed()).toBe(true);
    expect(await $('#remote-connect-network-tab-custom_server').isDisplayed()).toBe(true);

    await $('#remote-connect-group-bot').click();
    expect(await $('#remote-connect-bot-tab-telegram').isDisplayed()).toBe(true);
    expect(await $('#remote-connect-bot-tab-feishu').isDisplayed()).toBe(true);
    expect(await $('#remote-connect-bot-tab-weixin').isDisplayed()).toBe(true);

    await $('#remote-connect-group-account').click();
    const accountPanel = await $('[data-bf-component="remote-account-panel"]');
    await accountPanel.waitForDisplayed({ timeout: 10_000 });
    const accountView = await accountPanel.getAttribute('data-bf-view');
    if (accountView === 'login') {
      expect(await accountPanel.$('input[type="url"]').isDisplayed()).toBe(true);
      expect(await accountPanel.$('.account-panel__deploy-entry').isDisplayed()).toBe(true);
    } else {
      expect(accountView).toBe('devices');
      expect(await accountPanel.$('[data-bf-part="server"]').isDisplayed()).toBe(true);
    }

    expect((await dialog.getText()).toLowerCase()).not.toContain('ssh');

    await saveElementScreenshot(
      '[data-bf-component="remote-connect-dialog"][data-bf-part="root"]',
      'l0-device-overview-full-connect-dialog',
    );
    await saveStepScreenshot('l0-device-overview-full-connect-dialog');

    const severeLogs = (await browser.getLogs('browser'))
      .filter(entry => entry.level === 'SEVERE');
    expect(severeLogs.map(entry => entry.message)).toEqual([]);
  });
});
