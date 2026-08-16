/**
 * L0 Appearance spec: verifies the global Appearance runtime and settings flow.
 */

import { browser, expect, $ } from '@wdio/globals';

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

  await waitForDisplayed('[data-testid="settings-scene"]');
  const appearanceTab = await waitForDisplayed(
    '[data-testid="settings-nav-tab"][data-settings-tab="appearance"]',
  );
  await appearanceTab.click();

  await waitForDisplayed('[data-testid="appearance-config"]');
  await waitForDisplayed('[data-testid="appearance-palette-select"]');
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
