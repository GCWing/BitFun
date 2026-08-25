/**
 * L0 design-system Button integration: verifies that product scenes do not
 * collapse the component-owned inline spacing.
 */

import { $, browser, expect } from '@wdio/globals';

interface ButtonSpacing {
  component: string | null;
  variant: string | null;
  size: string | null;
  paddingLeft: string;
  paddingRight: string;
  paddingToken: string;
  contentInset: number | null;
  height: number;
}

async function waitForDisplayed(selector: string, timeout = 20_000) {
  const element = await $(selector);
  await element.waitForDisplayed({ timeout });
  return element;
}

async function openExtensions(): Promise<void> {
  const entry = await waitForDisplayed('[data-testid="agent-skill-entry"]');
  if (await entry.getAttribute('aria-expanded') !== 'true') {
    await entry.click();
  }
  await waitForDisplayed('[data-testid="agent-tab"]');
}

async function readButtonSpacing(selector: string): Promise<ButtonSpacing | null> {
  return browser.execute((targetSelector: string) => {
    const button = document.querySelector<HTMLElement>(targetSelector);
    if (!button) return null;

    const styles = window.getComputedStyle(button);
    const content = button.lastElementChild as HTMLElement | null;
    const buttonRect = button.getBoundingClientRect();
    const contentRect = content?.getBoundingClientRect() ?? null;
    const borderInline = Number.parseFloat(styles.borderLeftWidth)
      + Number.parseFloat(styles.borderRightWidth);

    return {
      component: button.getAttribute('data-bf-component'),
      variant: button.getAttribute('data-bf-variant'),
      size: button.getAttribute('data-size'),
      paddingLeft: styles.paddingLeft,
      paddingRight: styles.paddingRight,
      paddingToken: styles.getPropertyValue('--_button-padding-inline').trim(),
      contentInset: contentRect
        ? Math.round((buttonRect.width - contentRect.width - borderInline) * 100) / 100
        : null,
      height: buttonRect.height,
    };
  }, selector);
}

function expectSmallFilledButton(spacing: ButtonSpacing | null): void {
  expect(spacing).not.toBeNull();
  expect(spacing).toMatchObject({
    component: 'button',
    variant: 'fill',
    size: 'sm',
    paddingLeft: '16px',
    paddingRight: '16px',
    paddingToken: '16px',
    height: 26,
  });
  expect(spacing?.contentInset).toBe(32);
}

describe('L0 design-system Button integration', () => {
  it('keeps component-owned spacing for New Agent and Add Skill actions', async () => {
    await openExtensions();

    await $('[data-testid="agent-tab"]').click();
    await waitForDisplayed('[data-testid="agents-create-agent-btn"]');
    const newAgentSpacing = await readButtonSpacing('[data-testid="agents-create-agent-btn"]');
    console.log('[L0] New Agent Button spacing:', newAgentSpacing);
    expectSmallFilledButton(newAgentSpacing);

    await $('[data-testid="skill-tab"]').click();
    await waitForDisplayed('[data-bf-scene="skills"][data-bf-part="root"]');
    const allSkillsCategory = await waitForDisplayed(
      '[data-bf-scene="skills"][data-bf-part="sidebarItem"][data-bf-category="all"]',
    );
    await allSkillsCategory.click();
    await waitForDisplayed('[data-testid="skills-add-skill-btn"]');
    const addSkillSpacing = await readButtonSpacing('[data-testid="skills-add-skill-btn"]');
    console.log('[L0] Add Skill Button spacing:', addSkillSpacing);
    expectSmallFilledButton(addSkillSpacing);
  });

  it('keeps the Diagnostics action at medium height with centered icon and text', async () => {
    const settingsItem = await waitForDisplayed('[data-testid="nav-footer-settings-item"]');
    await settingsItem.click();

    const diagnosticsPage = await waitForDisplayed(
      '[data-testid="settings-nav-page"][data-settings-page="data.diagnostics"]',
    );
    await diagnosticsPage.click();
    await waitForDisplayed('[data-testid="diagnostics-export-button"]');

    const metrics = await browser.execute(() => {
      const button = document.querySelector<HTMLElement>('[data-testid="diagnostics-export-button"]');
      const content = button?.lastElementChild as HTMLElement | null;
      const icon = content?.firstElementChild as HTMLElement | null;
      const label = content?.lastElementChild as HTMLElement | null;
      if (!button || !icon || !label) return null;

      const buttonRect = button.getBoundingClientRect();
      const iconRect = icon.getBoundingClientRect();
      const labelRect = label.getBoundingClientRect();
      const center = (rect: DOMRect) => rect.top + rect.height / 2;

      return {
        size: button.getAttribute('data-size'),
        height: buttonRect.height,
        paddingLeft: window.getComputedStyle(button).paddingLeft,
        centerDelta: Math.abs(center(iconRect) - center(labelRect)),
      };
    });

    console.log('[L0] Diagnostics Button metrics:', metrics);
    expect(metrics).not.toBeNull();
    expect(metrics?.size).toBe('md');
    expect(metrics?.height).toBe(30);
    expect(metrics?.paddingLeft).toBe('20px');
    expect(metrics?.centerDelta).toBeLessThanOrEqual(0.5);
  });
});
