/**
 * Native Desktop coverage for the two popup surface contracts.
 *
 * The device overview is the floating reference surface. The About dialog is
 * the centered dialog reference surface. Border, radius, and shadow must stay
 * identical, while the centered dialog background must remain fully opaque.
 */

import { $, browser, expect } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';
import { saveElementScreenshot, saveStepScreenshot } from '../helpers/screenshot-utils';

interface PopupChrome {
  backgroundColor: string;
  borderBottomColor: string;
  borderBottomLeftRadius: string;
  borderBottomRightRadius: string;
  borderBottomStyle: string;
  borderBottomWidth: string;
  borderLeftColor: string;
  borderLeftStyle: string;
  borderLeftWidth: string;
  borderRightColor: string;
  borderRightStyle: string;
  borderRightWidth: string;
  borderTopColor: string;
  borderTopLeftRadius: string;
  borderTopRightRadius: string;
  borderTopStyle: string;
  borderTopWidth: string;
  boxShadow: string;
}

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
  const lightOption = await $(
    '[data-testid="appearance-palette-option"][data-appearance-id="bitfun-light"]',
  );
  await lightOption.waitForDisplayed({ timeout: 10_000 });
  await lightOption.click();

  await browser.waitUntil(async () => browser.execute(() => (
    document.documentElement.getAttribute('data-bf-appearance') === 'bitfun-light'
  )), {
    timeout: 10_000,
    timeoutMsg: 'The native app did not switch to the light Appearance',
  });
}

async function readPopupChrome(selector: string): Promise<PopupChrome> {
  const chrome = await browser.execute((surfaceSelector: string) => {
    const element = document.querySelector<HTMLElement>(surfaceSelector);
    if (!element) return null;
    const style = window.getComputedStyle(element);
    return {
      backgroundColor: style.backgroundColor,
      borderBottomColor: style.borderBottomColor,
      borderBottomLeftRadius: style.borderBottomLeftRadius,
      borderBottomRightRadius: style.borderBottomRightRadius,
      borderBottomStyle: style.borderBottomStyle,
      borderBottomWidth: style.borderBottomWidth,
      borderLeftColor: style.borderLeftColor,
      borderLeftStyle: style.borderLeftStyle,
      borderLeftWidth: style.borderLeftWidth,
      borderRightColor: style.borderRightColor,
      borderRightStyle: style.borderRightStyle,
      borderRightWidth: style.borderRightWidth,
      borderTopColor: style.borderTopColor,
      borderTopLeftRadius: style.borderTopLeftRadius,
      borderTopRightRadius: style.borderTopRightRadius,
      borderTopStyle: style.borderTopStyle,
      borderTopWidth: style.borderTopWidth,
      boxShadow: style.boxShadow,
    };
  }, selector);

  expect(chrome).not.toBeNull();
  return chrome as PopupChrome;
}

async function readBackgroundAlpha(selector: string): Promise<number> {
  const alpha = await browser.execute((surfaceSelector: string) => {
    const element = document.querySelector<HTMLElement>(surfaceSelector);
    if (!element) return null;
    const canvas = document.createElement('canvas');
    canvas.width = 1;
    canvas.height = 1;
    const context = canvas.getContext('2d');
    if (!context) return null;
    context.clearRect(0, 0, 1, 1);
    context.fillStyle = 'rgba(0, 0, 0, 0)';
    context.fillStyle = window.getComputedStyle(element).backgroundColor;
    context.fillRect(0, 0, 1, 1);
    return context.getImageData(0, 0, 1, 1).data[3];
  }, selector);

  expect(alpha).not.toBeNull();
  return alpha as number;
}

describe('L0 Popup Surface Consistency', () => {
  it('keeps the shared frame and makes the About dialog opaque', async () => {
    expect(await openWorkspace(undefined, { requireWorkspaceLabel: false })).toBe(true);
    await ensureLightAppearance();

    const deviceTrigger = await $('[data-testid="nav-footer-device-status"]');
    await deviceTrigger.waitForDisplayed({ timeout: 15_000 });
    if ((await deviceTrigger.getAttribute('aria-expanded')) !== 'true') {
      await deviceTrigger.click();
    }

    const deviceOverview = await $('[data-testid="nav-device-status-popover"]');
    await deviceOverview.waitForDisplayed({ timeout: 10_000 });
    const deviceChrome = await readPopupChrome('[data-testid="nav-device-status-popover"]');

    expect(deviceChrome.borderTopWidth).toBe('1px');
    expect(deviceChrome.borderTopStyle).toBe('solid');
    expect(deviceChrome.borderTopLeftRadius).not.toBe('0px');
    expect(deviceChrome.boxShadow).not.toBe('none');
    await saveElementScreenshot(
      '[data-testid="nav-device-status-popover"]',
      'l0-popup-surface-device-reference',
    );

    const deviceBackdrop = await $('[data-testid="nav-device-status-backdrop"]');
    await deviceBackdrop.click();
    await deviceOverview.waitForExist({ reverse: true, timeout: 5_000 });

    const settings = await $('[data-testid="nav-footer-settings-item"]');
    await settings.click();
    const aboutEntry = await $('[data-testid="nav-settings-about-item"]');
    await aboutEntry.waitForDisplayed({ timeout: 10_000 });
    await aboutEntry.click();

    const aboutDialog = await $('[data-testid="about-dialog-modal"]');
    await aboutDialog.waitForDisplayed({ timeout: 10_000 });
    const dialogChrome = await readPopupChrome('[data-testid="about-dialog-modal"]');
    const { backgroundColor: deviceBackground, ...deviceFrame } = deviceChrome;
    const { backgroundColor: dialogBackground, ...dialogFrame } = dialogChrome;
    expect(dialogFrame).toEqual(deviceFrame);
    expect(dialogBackground).not.toBe(deviceBackground);
    expect(await readBackgroundAlpha('[data-testid="about-dialog-modal"]')).toBe(255);

    const contentChrome = await readPopupChrome(
      '[data-bf-component="about-dialog"][data-bf-part="root"]',
    );
    expect(contentChrome.backgroundColor).toBe('rgba(0, 0, 0, 0)');
    expect(contentChrome.borderTopWidth).toBe('0px');
    expect(contentChrome.borderTopLeftRadius).toBe('0px');
    expect(contentChrome.boxShadow).toBe('none');

    await saveElementScreenshot(
      '[data-testid="about-dialog-modal"]',
      'l0-popup-surface-about-dialog',
    );
    await saveStepScreenshot('l0-popup-surface-consistency');
  });
});
