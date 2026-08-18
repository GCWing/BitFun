/**
 * L0 Appearance spec: verifies the global Appearance runtime and settings flow.
 */

import { browser, expect, $ } from '@wdio/globals';
import { saveStepScreenshot } from '../helpers/screenshot-utils';

async function waitForDisplayed(selector: string, timeout = 15000) {
  const element = await $(selector);
  await element.waitForDisplayed({ timeout });
  return element;
}

async function openAppearanceSettings(): Promise<void> {
  const existingPicker = await $('[data-testid="appearance-palette-select"]');
  if (await existingPicker.isDisplayed().catch(() => false)) {
    return;
  }

  const settingsItem = await waitForDisplayed('[data-testid="nav-footer-settings-item"]');
  await settingsItem.click();

  const themeConfiguration = await waitForDisplayed('[data-testid="nav-settings-theme-item"]');
  await themeConfiguration.click();

  await waitForDisplayed('[data-testid="settings-scene"]');
  await waitForDisplayed('[data-testid="appearance-config"]');
  await waitForDisplayed('[data-testid="appearance-palette-select"]');
}

async function selectAppearance(appearanceId: string): Promise<void> {
  await openAppearanceSettings();

  const picker = await $('[data-testid="appearance-palette-select"]');
  await picker.click();

  const option = await waitForDisplayed(
    `[data-testid="appearance-palette-option"][data-appearance-id="${appearanceId}"]`,
  );
  await option.click();

  await browser.waitUntil(async () => {
    return browser.execute((expectedId: string) => {
      return document.documentElement.getAttribute('data-bf-appearance') === expectedId;
    }, appearanceId);
  }, {
    timeout: 10000,
    interval: 100,
    timeoutMsg: `Appearance runtime did not apply ${appearanceId}`,
  });
}

describe('L0 Appearance', () => {
  it('app should start with an active Appearance runtime', async () => {
    console.log('[L0] Starting Appearance tests...');
    await browser.waitUntil(async () => {
      return browser.execute(() => {
        const root = document.documentElement;
        return document.readyState === 'complete'
          && root.getAttribute('data-bf-appearance-root') === 'true'
          && root.getAttribute('data-bf-appearance') !== null
          && root.getAttribute('data-bf-appearance-mode') !== null;
      });
    }, {
      timeout: 20000,
      interval: 200,
      timeoutMsg: 'Appearance runtime did not become active after app startup',
    });

    const title = await browser.getTitle();
    expect(title).toBeDefined();
  });

  it('should expose the root Appearance contract', async () => {
    const appearance = await browser.execute(() => {
      const root = document.documentElement;
      return {
        id: root.getAttribute('data-bf-appearance'),
        mode: root.getAttribute('data-bf-appearance-mode'),
        revision: root.getAttribute('data-bf-appearance-revision'),
        isRoot: root.getAttribute('data-bf-appearance-root'),
      };
    });

    console.log('[L0] Appearance root contract:', appearance);
    expect(appearance.id).toBeTruthy();
    expect(['dark', 'light']).toContain(appearance.mode);
    expect(appearance.revision).toBeTruthy();
    expect(appearance.isRoot).toBe('true');
  });

  it('should expose compiled Appearance tokens', async () => {
    const appearanceStyles = await browser.execute(() => {
      const styles = window.getComputedStyle(document.documentElement);
      const appearanceVariables = Array.from(styles)
        .filter(property => property.startsWith('--bf-appearance-'));

      return {
        variableCount: appearanceVariables.length,
        background: styles.getPropertyValue('--bf-appearance-token-color-bg-primary').trim(),
        text: styles.getPropertyValue('--bf-appearance-token-color-text-primary').trim(),
        accent: styles.getPropertyValue('--bf-appearance-token-color-accent-500').trim(),
      };
    });

    console.log('[L0] Appearance token contract:', appearanceStyles);
    expect(appearanceStyles.variableCount).toBeGreaterThan(0);
    expect(appearanceStyles.background).not.toBe('');
    expect(appearanceStyles.text).not.toBe('');
    expect(appearanceStyles.accent).not.toBe('');
  });

  it('should project the neutral and navy light palette into the native app', async () => {
    await selectAppearance('bitfun-light');

    const lightNavigation = await browser.execute(() => {
      const styles = window.getComputedStyle(document.documentElement);
      const navPanel = document.querySelector<HTMLElement>('[data-testid="nav-panel"]');
      return {
        primary: styles.getPropertyValue('--bf-appearance-token-color-bg-primary').trim(),
        scene: styles.getPropertyValue('--bf-appearance-token-color-bg-scene').trim(),
        softSurface: styles.getPropertyValue('--bf-appearance-token-color-accent-100').trim(),
        text: styles.getPropertyValue('--bf-appearance-token-color-text-primary').trim(),
        mutedText: styles.getPropertyValue('--bf-appearance-token-color-text-muted').trim(),
        accent: styles.getPropertyValue('--bf-appearance-token-color-accent-500').trim(),
        primaryButton: styles.getPropertyValue('--bf-appearance-token-btn-primary-bg').trim(),
        successBackground: styles.getPropertyValue('--bf-appearance-token-color-success-bg').trim(),
        errorBackground: styles.getPropertyValue('--bf-appearance-token-color-error-bg').trim(),
        border: styles.getPropertyValue('--bf-appearance-token-border-base').trim(),
        navBackground: navPanel ? window.getComputedStyle(navPanel).backgroundColor : null,
      };
    });

    expect(lightNavigation).toMatchObject({
      primary: '#fdfdfd',
      scene: '#ffffff',
      softSurface: '#f3f3f5',
      text: '#1c1c1f',
      mutedText: '#6a6a6a',
      accent: '#101a27',
      primaryButton: '#101a27',
      successBackground: '#e1fbe9',
      errorBackground: 'rgba(167, 67, 82, 0.12)',
      border: 'rgba(16, 26, 39, 0.15)',
      navBackground: 'rgb(253, 253, 253)',
    });
  });

  it('should compile inverse new-session hover colors for the light appearance', async () => {
    await selectAppearance('bitfun-light');

    const hoverContract = await browser.execute(() => {
      const styleRules: CSSStyleRule[] = [];
      const collectStyleRules = (rules: CSSRuleList): void => {
        for (const rule of Array.from(rules)) {
          if (rule instanceof CSSStyleRule) {
            styleRules.push(rule);
          }
          if ('cssRules' in rule) {
            collectStyleRules((rule as CSSGroupingRule).cssRules);
          }
        }
      };

      for (const styleSheet of Array.from(document.styleSheets)) {
        try {
          collectStyleRules(styleSheet.cssRules);
        } catch {
          // Ignore stylesheets that the WebView does not expose through CSSOM.
        }
      }

      const hasSelector = (rule: CSSStyleRule, selector: string): boolean => {
        return rule.selectorText.split(',').some(candidate => candidate.trim() === selector);
      };
      const buttonRule = styleRules.find(rule => hasSelector(
        rule,
        '.bitfun-nav-panel__utility-action:hover',
      ));
      const iconRule = styleRules.find(rule => hasSelector(
        rule,
        '.bitfun-nav-panel__utility-action:hover > svg',
      ));
      const rootStyle = window.getComputedStyle(document.documentElement);

      return {
        appearanceMode: document.documentElement.getAttribute('data-bf-appearance-mode'),
        textPrimary: rootStyle.getPropertyValue('--bf-appearance-token-color-text-primary').trim(),
        scene: rootStyle.getPropertyValue('--bf-appearance-token-color-bg-scene').trim(),
        buttonBackground: buttonRule?.style.background ?? null,
        buttonBorder: buttonRule?.style.borderColor ?? null,
        iconColor: iconRule?.style.color ?? null,
        iconStroke: iconRule?.style.stroke ?? null,
      };
    });

    expect(hoverContract).toMatchObject({
      appearanceMode: 'light',
      textPrimary: '#1c1c1f',
      scene: '#ffffff',
      buttonBackground: 'var(--bf-appearance-token-color-text-primary)',
      buttonBorder: 'var(--bf-appearance-token-color-text-primary)',
      iconColor: 'var(--bf-appearance-token-color-bg-scene)',
      iconStroke: 'currentcolor',
    });
  });

  it('should lift only the scene viewport above the light navigation shell', async () => {
    await selectAppearance('bitfun-light');
    await waitForDisplayed('.bitfun-workspace-body__scene-area');
    await waitForDisplayed('.bitfun-scene-viewport');

    const sceneSurface = await browser.execute(() => {
      const workbench = document.querySelector<HTMLElement>('.bitfun-workspace-body');
      const sceneArea = document.querySelector<HTMLElement>('.bitfun-workspace-body__scene-area');
      const viewport = document.querySelector<HTMLElement>('.bitfun-scene-viewport');

      if (!workbench || !sceneArea || !viewport) {
        return null;
      }

      const workbenchStyle = window.getComputedStyle(workbench);
      const sceneStyle = window.getComputedStyle(sceneArea);
      const viewportStyle = window.getComputedStyle(viewport);

      return {
        workbenchBackground: workbenchStyle.backgroundColor,
        sceneBackground: sceneStyle.backgroundColor,
        sceneBorderWidth: sceneStyle.borderTopWidth,
        sceneRadius: sceneStyle.borderTopLeftRadius,
        sceneShadow: sceneStyle.boxShadow,
        viewportBackground: viewportStyle.backgroundColor,
        viewportBorderWidth: viewportStyle.borderTopWidth,
        viewportBorderColor: viewportStyle.borderTopColor,
        viewportRadius: viewportStyle.borderTopLeftRadius,
        viewportShadow: viewportStyle.boxShadow,
      };
    });

    expect(sceneSurface).not.toBeNull();
    expect(sceneSurface).toMatchObject({
      workbenchBackground: 'rgb(253, 253, 253)',
      sceneBackground: 'rgba(0, 0, 0, 0)',
      sceneBorderWidth: '0px',
      sceneRadius: '0px',
      sceneShadow: 'none',
      viewportBackground: 'rgb(255, 255, 255)',
      viewportBorderWidth: '1px',
      viewportRadius: '12px',
    });
    expect(sceneSurface?.viewportBorderColor).not.toBe('rgba(0, 0, 0, 0)');
    expect(sceneSurface?.viewportShadow).not.toBe('none');
    await saveStepScreenshot('l0-appearance-light-floating-scene');
  });

  it('should keep Mini Apps and the selected navigation surface visually light', async () => {
    await selectAppearance('bitfun-light');

    const navBack = await waitForDisplayed(
      '[data-bf-component="nav-bar"][data-bf-part="back"]:not(.is-inactive)',
    );
    await navBack.click();

    const miniAppsEntry = await waitForDisplayed('[data-testid="nav-miniapps-entry"]');
    await miniAppsEntry.click();
    await waitForDisplayed('[data-bf-scene="miniapp-gallery"]');
    await waitForDisplayed('.miniapp-gallery-scene__tabs .bitfun-tabs__tab--active');
    await waitForDisplayed('[data-bf-component="mini-app-card"]', 20000);

    const miniAppPresentation = await browser.execute(() => {
      const selectedNavigation = document.querySelector<HTMLElement>('[data-testid="nav-miniapps-entry"]');
      const selectedTab = document.querySelector<HTMLElement>(
        '.miniapp-gallery-scene__tabs .bitfun-tabs__tab--active',
      );
      const scene = document.querySelector<HTMLElement>('[data-bf-scene="miniapp-gallery"]');
      const card = document.querySelector<HTMLElement>('[data-bf-component="mini-app-card"]');
      const cardFooter = card?.querySelector<HTMLElement>('.miniapp-card__footer') ?? null;
      const cardPrimaryAction = card?.querySelector<HTMLElement>('.miniapp-card__action-btn--primary') ?? null;
      const emptyRunningZone = document.querySelector<HTMLElement>(
        '.miniapp-gallery__running-zone.is-empty',
      );

      return {
        selectedNavigation: selectedNavigation
          ? {
              background: window.getComputedStyle(selectedNavigation).backgroundColor,
              border: window.getComputedStyle(selectedNavigation).borderColor,
            }
          : null,
        selectedTabBackground: selectedTab
          ? window.getComputedStyle(selectedTab).backgroundColor
          : null,
        sceneBackground: scene ? window.getComputedStyle(scene).backgroundColor : null,
        emptyRunningZoneHeight: emptyRunningZone?.getBoundingClientRect().height ?? null,
        card: card
          ? {
              background: window.getComputedStyle(card).backgroundColor,
              borderStyle: window.getComputedStyle(card).borderStyle,
              footerBackground: cardFooter ? window.getComputedStyle(cardFooter).backgroundColor : null,
              primaryActionBackground: cardPrimaryAction
                ? window.getComputedStyle(cardPrimaryAction).backgroundColor
                : null,
            }
          : null,
      };
    });

    expect(miniAppPresentation.selectedNavigation).toEqual({
      background: 'rgb(243, 243, 245)',
      border: 'rgba(0, 0, 0, 0)',
    });
    expect(miniAppPresentation.selectedTabBackground).toBe('rgb(243, 243, 245)');
    expect(miniAppPresentation.sceneBackground).toBe('rgb(255, 255, 255)');
    expect(miniAppPresentation.emptyRunningZoneHeight).not.toBeNull();
    expect(miniAppPresentation.emptyRunningZoneHeight!).toBeLessThanOrEqual(40);
    expect(miniAppPresentation.card).toEqual({
      background: 'rgb(243, 243, 245)',
      borderStyle: 'none',
      footerBackground: 'rgba(0, 0, 0, 0)',
      primaryActionBackground: 'rgb(16, 26, 39)',
    });
  });

  it('should render monochrome structural chrome against a white workspace', async () => {
    await selectAppearance('bitfun-monochrome');
    await waitForDisplayed('[data-testid="nav-panel"]');
    await waitForDisplayed('[data-testid="settings-nav"]');
    await waitForDisplayed('.bitfun-scene-bar');
    await waitForDisplayed('.bitfun-scene-viewport');

    const contrastPresentation = await browser.execute(() => {
      const rootStyles = window.getComputedStyle(document.documentElement);
      const bodyStyles = window.getComputedStyle(document.body);
      const workbench = document.querySelector<HTMLElement>('.bitfun-workspace-body');
      const navPanel = document.querySelector<HTMLElement>('[data-testid="nav-panel"]');
      const settingsTitle = document.querySelector<HTMLElement>('.bitfun-settings-nav__title');
      const settingsActiveItem = document.querySelector<HTMLElement>('.bitfun-settings-nav__item.is-active');
      const settingsSectionBody = document.querySelector<HTMLElement>(
        '[data-testid="appearance-settings-section"] .bitfun-config-page-section__body',
      );
      const settingsRows = document.querySelectorAll<HTMLElement>(
        '[data-testid="appearance-settings-section"] .bitfun-config-page-row',
      );
      const settingsRowDivider = settingsRows.item(1);
      const appearanceSelect = document.querySelector<HTMLElement>(
        '[data-testid="appearance-palette-select"]',
      );
      const sceneBar = document.querySelector<HTMLElement>('.bitfun-scene-bar');
      const viewport = document.querySelector<HTMLElement>('.bitfun-scene-viewport');

      if (
        !workbench ||
        !navPanel ||
        !settingsTitle ||
        !settingsActiveItem ||
        !settingsSectionBody ||
        !settingsRowDivider ||
        !appearanceSelect ||
        !sceneBar ||
        !viewport
      ) {
        return null;
      }

      const workbenchStyles = window.getComputedStyle(workbench);
      const navStyles = window.getComputedStyle(navPanel);
      const sceneBarStyles = window.getComputedStyle(sceneBar);
      const viewportStyles = window.getComputedStyle(viewport);

      return {
        appearanceId: document.documentElement.getAttribute('data-bf-appearance'),
        appearanceMode: document.documentElement.getAttribute('data-bf-appearance-mode'),
        contentBackgroundToken: rootStyles.getPropertyValue('--bf-appearance-token-color-bg-primary').trim(),
        contentTextToken: rootStyles.getPropertyValue('--bf-appearance-token-color-text-primary').trim(),
        contentSecondaryTextToken: rootStyles.getPropertyValue('--bf-appearance-token-color-text-secondary').trim(),
        contentBorderToken: rootStyles.getPropertyValue('--bf-appearance-token-border-subtle').trim(),
        contentSurfaceToken: rootStyles.getPropertyValue('--bf-appearance-token-element-bg-subtle').trim(),
        configSectionBackgroundToken: rootStyles.getPropertyValue('--bf-appearance-token-config-page-section-bg').trim(),
        configSectionBorderToken: rootStyles.getPropertyValue('--bf-appearance-token-config-page-section-border').trim(),
        configSectionBorderWidthToken: rootStyles.getPropertyValue('--bf-appearance-token-config-page-section-border-width').trim(),
        configDividerToken: rootStyles.getPropertyValue('--bf-appearance-token-config-page-divider').trim(),
        chromeBackgroundToken: rootStyles.getPropertyValue('--bf-appearance-token-chrome-bg-primary').trim(),
        chromeTextToken: rootStyles.getPropertyValue('--bf-appearance-token-chrome-text-primary').trim(),
        bodyBackground: bodyStyles.backgroundColor,
        workbenchBackground: workbenchStyles.backgroundColor,
        navBackground: navStyles.backgroundColor,
        navTextToken: navStyles.getPropertyValue('--bf-appearance-token-color-text-primary').trim(),
        settingsTitleColor: window.getComputedStyle(settingsTitle).color,
        settingsActiveItemColor: window.getComputedStyle(settingsActiveItem).color,
        settingsSectionBackground: window.getComputedStyle(settingsSectionBody).backgroundColor,
        settingsSectionBorder: window.getComputedStyle(settingsSectionBody).borderTopColor,
        settingsSectionBorderWidth: window.getComputedStyle(settingsSectionBody).borderTopWidth,
        settingsSectionShadow: window.getComputedStyle(settingsSectionBody).boxShadow,
        settingsRowDivider: window.getComputedStyle(settingsRowDivider).borderTopColor,
        appearanceSelectBorder: window.getComputedStyle(appearanceSelect).borderTopColor,
        sceneBarTextToken: sceneBarStyles.getPropertyValue('--bf-appearance-token-color-text-primary').trim(),
        viewportBackground: viewportStyles.backgroundColor,
        viewportTextToken: viewportStyles.getPropertyValue('--bf-appearance-token-color-text-primary').trim(),
        viewportRadius: viewportStyles.borderTopLeftRadius,
      };
    });

    expect(contrastPresentation).toEqual({
      appearanceId: 'bitfun-monochrome',
      appearanceMode: 'light',
      contentBackgroundToken: '#ffffff',
      contentTextToken: '#1c1c1f',
      contentSecondaryTextToken: '#555555',
      contentBorderToken: 'rgba(16, 26, 39, 0.08)',
      contentSurfaceToken: 'rgba(16, 26, 39, 0.03)',
      configSectionBackgroundToken: '#f3f3f5',
      configSectionBorderToken: 'transparent',
      configSectionBorderWidthToken: '0',
      configDividerToken: 'rgba(16, 26, 39, 0.08)',
      chromeBackgroundToken: '#1c1c1f',
      chromeTextToken: '#f3f3f5',
      bodyBackground: 'rgb(28, 28, 31)',
      workbenchBackground: 'rgb(28, 28, 31)',
      navBackground: 'rgb(28, 28, 31)',
      navTextToken: '#f3f3f5',
      settingsTitleColor: 'rgb(243, 243, 245)',
      settingsActiveItemColor: 'rgb(243, 243, 245)',
      settingsSectionBackground: 'rgb(243, 243, 245)',
      settingsSectionBorder: 'rgba(0, 0, 0, 0)',
      settingsSectionBorderWidth: '0px',
      settingsSectionShadow: 'none',
      settingsRowDivider: 'rgba(16, 26, 39, 0.08)',
      appearanceSelectBorder: 'rgba(16, 26, 39, 0.15)',
      sceneBarTextToken: '#f3f3f5',
      viewportBackground: 'rgb(255, 255, 255)',
      viewportTextToken: '#1c1c1f',
      viewportRadius: '12px',
    });
    await saveStepScreenshot('l0-appearance-monochrome-contrast');
  });

  it('should expose the Appearance selector in settings', async () => {
    await openAppearanceSettings();

    const section = await $('[data-testid="appearance-settings-section"]');
    const picker = await $('[data-testid="appearance-palette-select"]');
    expect(await section.isDisplayed()).toBe(true);
    expect(await picker.isDisplayed()).toBe(true);
  });

  it('should switch to another built-in Appearance', async () => {
    await openAppearanceSettings();

    const before = await browser.execute(() => ({
      id: document.documentElement.getAttribute('data-bf-appearance'),
      revision: document.documentElement.getAttribute('data-bf-appearance-revision'),
    }));

    const picker = await $('[data-testid="appearance-palette-select"]');
    await picker.click();

    await browser.waitUntil(async () => {
      const options = await $$('[data-testid="appearance-palette-option"]');
      return await options.length >= 2;
    }, {
      timeout: 10000,
      interval: 100,
      timeoutMsg: 'Appearance options did not open',
    });

    const options = await $$('[data-testid="appearance-palette-option"]');
    let targetId: string | null = null;
    for (const option of options) {
      const optionId = await option.getAttribute('data-appearance-id');
      if (optionId && optionId !== 'system' && optionId !== before.id) {
        targetId = optionId;
        await option.click();
        break;
      }
    }

    expect(targetId).toBeTruthy();
    await browser.waitUntil(async () => {
      return browser.execute((expectedId: string) => {
        return document.documentElement.getAttribute('data-bf-appearance') === expectedId;
      }, targetId!);
    }, {
      timeout: 10000,
      interval: 100,
      timeoutMsg: `Appearance runtime did not apply ${targetId}`,
    });

    const after = await browser.execute(() => {
      const root = document.documentElement;
      const styles = window.getComputedStyle(root);
      return {
        id: root.getAttribute('data-bf-appearance'),
        mode: root.getAttribute('data-bf-appearance-mode'),
        revision: root.getAttribute('data-bf-appearance-revision'),
        background: styles.getPropertyValue('--bf-appearance-token-color-bg-primary').trim(),
      };
    });

    console.log('[L0] Appearance switched:', { before, after });
    expect(after.id).toBe(targetId);
    expect(['dark', 'light']).toContain(after.mode);
    expect(after.revision).toBeTruthy();
    expect(after.background).not.toBe('');
  });

  after(() => {
    console.log('[L0] Appearance tests complete');
  });
});
