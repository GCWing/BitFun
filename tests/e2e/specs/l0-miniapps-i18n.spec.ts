/**
 * L0 mini apps i18n spec: reproduces the bug where Mini Apps stays Chinese
 * even after the app language is switched to English.
 */

import { browser, expect, $, $$ } from '@wdio/globals';
import { saveStepScreenshot } from '../helpers/screenshot-utils';

const CHINESE_TEXT_RE = /[\u4e00-\u9fff]/;

async function isWorkspaceOpen(): Promise<boolean> {
  const navPanel = await $('.bitfun-nav-panel');
  return navPanel.isExisting();
}

async function ensureWorkspaceOpen(): Promise<void> {
  if (await isWorkspaceOpen()) {
    return;
  }

  const recentItems = await $$('.welcome-scene__recent-item');
  const recentItemCount = await recentItems.length;
  if (recentItemCount === 0) {
    throw new Error('No open workspace and no recent workspace entry was found.');
  }

  await recentItems[0].click();
  await browser.waitUntil(
    async () => isWorkspaceOpen(),
    {
      timeout: 20000,
      timeoutMsg: 'Workspace did not open from the recent workspace list.',
    }
  );
}

async function openSettings(): Promise<void> {
  const moreButton = await $('.bitfun-nav-panel__footer-btn--icon');
  await moreButton.waitForClickable({ timeout: 10000 });
  await moreButton.click();

  await browser.waitUntil(
    async () => (await (await $$('.bitfun-nav-panel__footer-menu-item')).length) > 0,
    {
      timeout: 5000,
      timeoutMsg: 'Footer menu items did not appear.',
    }
  );

  const menuItems = await $$('.bitfun-nav-panel__footer-menu-item');
  for (const item of menuItems) {
    const text = (await item.getText()).trim();
    const html = await item.getHTML();
    const ariaLabel = (await item.getAttribute('aria-label')) || '';
    if (
      text.includes('Settings') ||
      text.includes('设置') ||
      html.includes('Settings') ||
      html.includes('settings') ||
      html.includes('设置') ||
      ariaLabel.includes('Settings') ||
      ariaLabel.includes('设置')
    ) {
      await item.click();
      await browser.waitUntil(
        async () => (await $('.bitfun-settings-scene')).isExisting(),
        {
          timeout: 10000,
          timeoutMsg: 'Settings scene did not open.',
        }
      );
      return;
    }
  }

  throw new Error('Could not find the Settings menu item.');
}

async function openAppearanceTab(): Promise<void> {
  const navItems = await $$('.bitfun-settings-nav__item');
  for (const item of navItems) {
    const text = (await item.getText()).trim();
    if (
      text.includes('Appearance') ||
      text.includes('Theme') ||
      text.includes('外观') ||
      text.includes('主题')
    ) {
      await item.click();
      await browser.waitUntil(
        async () => (await $('.theme-config__language-select .select__trigger')).isExisting(),
        {
          timeout: 10000,
          timeoutMsg: 'Appearance tab did not render the language selector.',
        }
      );
      return;
    }
  }

  throw new Error('Could not find the Appearance tab in settings.');
}

async function switchLanguageToEnglish(): Promise<void> {
  await openSettings();
  await openAppearanceTab();

  const languageSelect = await $('.theme-config__language-select');
  const currentValueLabel = await languageSelect.$('.select__value-label');
  const hasCurrentLabel = await currentValueLabel.isExisting();
  const currentLabel = hasCurrentLabel ? (await currentValueLabel.getText()).trim() : '';

  if (currentLabel !== 'English') {
    const trigger = await languageSelect.$('.select__trigger');
    await trigger.waitForClickable({ timeout: 10000 });
    await trigger.click();

    await browser.waitUntil(
      async () => (await (await $$('.select__option')).length) > 0,
      {
        timeout: 5000,
        timeoutMsg: 'Language options did not appear.',
      }
    );

    const options = await $$('.select__option');
    for (const option of options) {
      const text = (await option.getText()).trim();
      if (text.includes('English')) {
        await option.click();
        break;
      }
    }
  }

  await browser.waitUntil(
    async () => {
      const lang = await browser.execute(() => document.documentElement.lang);
      return lang === 'en-US';
    },
    {
      timeout: 15000,
      timeoutMsg: 'App language did not switch to English.',
    }
  );

  await browser.waitUntil(
    async () => {
      const settingsTitle = await $('.bitfun-settings-nav__title');
      return (await settingsTitle.getText()).trim().includes('Settings');
    },
    {
      timeout: 10000,
      timeoutMsg: 'Settings page did not update to English after language switch.',
    }
  );
}

async function openMiniAppsPage(): Promise<void> {
  const miniAppsEntry = await $('.bitfun-nav-panel__miniapp-entry');
  await miniAppsEntry.waitForExist({ timeout: 10000 });
  await browser.execute((element: HTMLElement) => {
    element.scrollIntoView({ block: 'nearest' });
    element.click();
  }, miniAppsEntry);

  await browser.waitUntil(
    async () => (await $('.miniapp-gallery .gallery-page-header__title')).isExisting(),
    {
      timeout: 10000,
      timeoutMsg: 'Mini Apps gallery did not open.',
    }
  );
}

async function getMiniAppsCopy() {
  const title = await $('.miniapp-gallery .gallery-page-header__title');
  const subtitle = await $('.miniapp-gallery .gallery-page-header__subtitle');
  const searchInput = await $('.miniapp-gallery .search__input');
  const zoneTitleEls = await $$('.miniapp-gallery .gallery-zone__title');
  const actionButtons = await $$('.miniapp-gallery .gallery-action-btn');

  const zoneTitles: string[] = [];
  for (const element of zoneTitleEls) {
    zoneTitles.push((await element.getText()).trim());
  }

  const actionTitles: string[] = [];
  for (const button of actionButtons) {
    const titleAttr = await button.getAttribute('title');
    if (titleAttr) {
      actionTitles.push(titleAttr.trim());
    }
  }

  return {
    lang: await browser.execute(() => document.documentElement.lang),
    title: (await title.getText()).trim(),
    subtitle: (await subtitle.getText()).trim(),
    searchPlaceholder: ((await searchInput.getAttribute('placeholder')) || '').trim(),
    zoneTitles,
    actionTitles,
  };
}

describe('L0 Mini Apps i18n', () => {
  it('should show Mini Apps page text in English when app language is English', async () => {
    await browser.pause(3000);
    await ensureWorkspaceOpen();
    await switchLanguageToEnglish();
    await openMiniAppsPage();

    const copy = await getMiniAppsCopy();
    console.log('[L0] Mini Apps copy snapshot:', copy);

    await saveStepScreenshot('miniapps-english-mode-bug');

    expect(copy.lang).toBe('en-US');
    expect(copy.title).not.toMatch(CHINESE_TEXT_RE);
    expect(copy.subtitle).not.toMatch(CHINESE_TEXT_RE);
    expect(copy.searchPlaceholder).not.toMatch(CHINESE_TEXT_RE);
    expect(copy.zoneTitles.join(' | ')).not.toMatch(CHINESE_TEXT_RE);
    expect(copy.actionTitles.join(' | ')).not.toMatch(CHINESE_TEXT_RE);
  });
});
