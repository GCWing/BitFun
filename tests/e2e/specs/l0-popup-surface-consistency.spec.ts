/**
 * Native Desktop coverage for the two popup surface contracts.
 *
 * The device overview is the floating reference surface. The About dialog is
 * the centered dialog reference surface. Border, radius, and shadow must stay
 * identical, while the centered dialog background must remain fully opaque.
 */

import { $, browser, expect } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';
import { expectPopupCloseContract } from '../helpers/popup-close-contract';
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

interface AboutLayoutEvidence {
  brandRight: number;
  branchHorizontalOverflow: number;
  dotCount: number;
  dotMatrixHeight: number;
  dotMatrixWidth: number;
  headerCloseCenterOffset: number;
  headerHeight: number;
  headerLogoCount: number;
  headerTitle: string;
  horizontalOverflow: number;
  metadataLeft: number;
  modalHeight: number;
  modalRight: number;
  modalWidth: number;
  starBackgroundColor: string;
  starBorderRadius: string;
  starColor: string;
  starRight: number;
  titleColor: string;
  viewportWidth: number;
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

async function readAboutLayoutEvidence(): Promise<AboutLayoutEvidence> {
  const evidence = await browser.execute(() => {
    const modal = document.querySelector<HTMLElement>('[data-testid="about-dialog-modal"]');
    const root = document.querySelector<HTMLElement>(
      '[data-bf-component="about-dialog"][data-bf-part="root"]',
    );
    const brand = document.querySelector<HTMLElement>('.bitfun-about-dialog__brand');
    const metadata = document.querySelector<HTMLElement>('.bitfun-about-dialog__metadata');
    const header = modal?.querySelector<HTMLElement>('.modal__header-shell');
    const headerTitle = modal?.querySelector<HTMLElement>('.modal__title');
    const closeButton = modal?.querySelector<HTMLElement>('[data-bf-role="popup-close"]');
    const title = document.querySelector<HTMLElement>('.bitfun-about-dialog__title');
    const branch = document.querySelector<HTMLElement>('[data-testid="about-branch-value"]');
    const dotMatrix = document.querySelector<HTMLElement>('[data-testid="about-dot-matrix"]');
    const starButton = document.querySelector<HTMLElement>('[data-testid="about-github-star"]');
    if (!modal || !root || !brand || !metadata || !header || !headerTitle || !closeButton || !title || !branch || !dotMatrix || !starButton) return null;

    const modalRect = modal.getBoundingClientRect();
    const brandRect = brand.getBoundingClientRect();
    const metadataRect = metadata.getBoundingClientRect();
    const headerRect = header.getBoundingClientRect();
    const closeRect = closeButton.getBoundingClientRect();
    const dotMatrixRect = dotMatrix.getBoundingClientRect();
    const starRect = starButton.getBoundingClientRect();
    const starStyle = window.getComputedStyle(starButton);
    return {
      brandRight: brandRect.right,
      branchHorizontalOverflow: branch.scrollWidth - branch.clientWidth,
      dotCount: dotMatrix.children.length,
      dotMatrixHeight: dotMatrixRect.height,
      dotMatrixWidth: dotMatrixRect.width,
      headerCloseCenterOffset: Math.abs(
        (headerRect.top + headerRect.height / 2) - (closeRect.top + closeRect.height / 2),
      ),
      headerHeight: headerRect.height,
      headerLogoCount: modal.querySelectorAll('[data-testid="about-header-logo"]').length,
      headerTitle: headerTitle.textContent?.trim() ?? '',
      horizontalOverflow: root.scrollWidth - root.clientWidth,
      metadataLeft: metadataRect.left,
      modalHeight: modalRect.height,
      modalRight: modalRect.right,
      modalWidth: modalRect.width,
      starBackgroundColor: starStyle.backgroundColor,
      starBorderRadius: starStyle.borderRadius,
      starColor: starStyle.color,
      starRight: starRect.right,
      titleColor: window.getComputedStyle(title).color,
      viewportWidth: window.innerWidth,
    };
  });

  expect(evidence).not.toBeNull();
  return evidence as AboutLayoutEvidence;
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
    await expectPopupCloseContract('[data-testid="about-dialog-modal"]');
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

    const aboutLayout = await readAboutLayoutEvidence();
    expect(aboutLayout.headerLogoCount).toBe(0);
    expect(aboutLayout.headerTitle).toContain('BitFun');
    expect(aboutLayout.headerHeight).toBeGreaterThanOrEqual(44);
    expect(aboutLayout.headerHeight).toBeLessThanOrEqual(49);
    expect(aboutLayout.headerCloseCenterOffset).toBeLessThanOrEqual(1);
    expect(aboutLayout.horizontalOverflow).toBeLessThanOrEqual(1);
    expect(aboutLayout.branchHorizontalOverflow).toBeLessThanOrEqual(1);
    expect(aboutLayout.dotCount).toBe(91);
    expect(aboutLayout.dotMatrixWidth).toBeGreaterThanOrEqual(190);
    expect(aboutLayout.dotMatrixWidth).toBeLessThanOrEqual(220);
    expect(aboutLayout.dotMatrixHeight).toBeGreaterThanOrEqual(90);
    expect(aboutLayout.dotMatrixHeight).toBeLessThanOrEqual(110);
    expect(Math.abs(aboutLayout.brandRight - aboutLayout.metadataLeft)).toBeLessThanOrEqual(1);
    expect(aboutLayout.starBackgroundColor).toBe(aboutLayout.titleColor);
    expect(aboutLayout.starColor).not.toBe(aboutLayout.starBackgroundColor);
    expect(Number.parseFloat(aboutLayout.starBorderRadius)).toBeGreaterThanOrEqual(20);
    expect(aboutLayout.starRight).toBeLessThanOrEqual(aboutLayout.modalRight + 1);
    if (aboutLayout.viewportWidth >= 1040) {
      expect(aboutLayout.modalWidth).toBeGreaterThanOrEqual(900);
      expect(aboutLayout.modalWidth).toBeLessThanOrEqual(980);
      expect(aboutLayout.modalHeight).toBeLessThanOrEqual(700);
      expect(aboutLayout.modalWidth / aboutLayout.modalHeight).toBeGreaterThan(1.35);
    }

    await saveElementScreenshot(
      '[data-testid="about-dialog-modal"]',
      'l0-popup-surface-about-dialog',
    );
    await saveStepScreenshot('l0-popup-surface-consistency');
  });
});
