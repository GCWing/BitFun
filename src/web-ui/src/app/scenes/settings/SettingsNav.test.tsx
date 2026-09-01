// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('./settingsRegistry', () => {
  const pages = [
    { id: 'application.general', categoryId: 'application', labelKey: 'General' },
    { id: 'application.appearance', categoryId: 'application', labelKey: 'Appearance' },
    { id: 'ai.models', categoryId: 'ai', labelKey: 'Models' },
  ].map((page) => ({ ...page, descriptionKey: 'Description', keywords: ['settings'], searchPhrases: [] }));
  return {
    DEFAULT_SETTINGS_PAGE_ID: 'application.general',
    SETTINGS_PAGE_MANIFESTS: pages,
    SETTINGS_CATEGORIES: [
      { id: 'application', labelKey: 'Application', pages: pages.slice(0, 2) },
      { id: 'ai', labelKey: 'AI', pages: pages.slice(2) },
    ],
    preloadSettingsPage: vi.fn(async () => undefined),
  };
});

vi.mock('react-i18next', () => {
  const t = (key: string) => key;
  const i18n = { language: 'en-US', getFixedT: () => t };
  return { useTranslation: () => ({ t, i18n }) };
});
vi.mock('@/infrastructure/i18n/hooks/useI18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));
vi.mock('@/shared/utils/motionPreference', () => ({
  getInteractionMotion: () => 'instant',
}));

import SettingsNav from './SettingsNav';
import { useSettingsStore } from './settingsStore';

describe('SettingsNav shared component composition', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    useSettingsStore.setState(useSettingsStore.getInitialState());
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    act(() => root.render(<SettingsNav />));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  async function search(query: string) {
    const input = container.querySelector('input')!;
    act(() => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, query);
      input.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await act(async () => { await vi.advanceTimersByTimeAsync(150); });
    return input;
  }

  function pressKey(target: Element, key: string) {
    target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
  }

  it('keeps the header outside the scroll area and applies content styles to the real content slot', () => {
    const nav = container.querySelector('nav')!;
    const header = nav.querySelector(':scope > [data-bf-part="header"]')!;
    const viewport = nav.querySelector('[data-bf-component="scroll-area"]')!;
    const content = viewport.querySelector('[data-bf-part="content"]')!;
    expect(header.querySelector('.bitfun-settings-nav__title')).not.toBeNull();
    expect(header.querySelector('input')).not.toBeNull();
    expect(viewport.contains(header)).toBe(false);
    expect(content.classList.contains('bitfun-settings-nav__content')).toBe(true);
    expect(content.querySelectorAll(':scope > section')).toHaveLength(2);
    expect(content.querySelectorAll('[data-bf-part="heading-label"]')).toHaveLength(2);
    const caption = content.querySelector('.bitfun-settings-nav__category-label')!;
    expect(caption.parentElement?.getAttribute('data-bf-part')).toBe('heading-label');
    expect(content.querySelectorAll('[data-testid="settings-nav-page"]')).toHaveLength(3);
  });

  it('drives the shared selected state from the active destination', async () => {
    const general = container.querySelector<HTMLButtonElement>('[data-settings-page="application.general"]')!;
    const appearance = container.querySelector<HTMLButtonElement>('[data-settings-page="application.appearance"]')!;
    expect(general.getAttribute('aria-current')).toBe('page');
    expect(general.parentElement?.getAttribute('data-bf-component')).toBe('action-item');
    await act(async () => appearance.click());
    expect(useSettingsStore.getState().activePageId).toBe('application.appearance');
    expect(appearance.getAttribute('aria-current')).toBe('page');
    const selectedLabel = appearance.querySelector('[data-bf-part="label"]')!;
    expect(selectedLabel.matches('.bitfun-settings-nav__item > [data-bf-part="trigger"][aria-current] > [data-bf-part="label"]')).toBe(true);
    expect(selectedLabel.querySelector('.bitfun-settings-nav__item-label')).not.toBeNull();
    expect(general.hasAttribute('aria-current')).toBe(false);
    expect(container.querySelectorAll('[aria-current="page"]')).toHaveLength(1);
    expect(container.querySelector('.is-active')).toBeNull();
  });

  it('keeps two-line search results and keyboard selection working with real navigation items', async () => {
    const input = await search('settings');
    const results = container.querySelector<HTMLDivElement>('[role="listbox"]')!;
    expect(results.querySelectorAll('[role="option"]')).toHaveLength(3);
    const first = results.querySelector('[role="option"]')!;
    const label = first.querySelector('[data-bf-part="label"]')!;
    expect(label.querySelector('.bitfun-settings-nav__search-result-line')).not.toBeNull();
    expect(label.querySelector('.bitfun-settings-nav__search-result-desc')).not.toBeNull();
    expect(first.getAttribute('aria-current')).toBe('page');

    await act(async () => pressKey(input, 'ArrowDown'));
    expect(document.activeElement).toBe(results);
    expect(results.getAttribute('aria-activedescendant')).toBe('settings-nav-result-0');
    act(() => pressKey(results, 'ArrowDown'));
    expect(results.getAttribute('aria-activedescendant')).toBe('settings-nav-result-1');
    expect(results.querySelector('.is-highlighted > button')?.id).toBe('settings-nav-result-1');
    await act(async () => pressKey(results, 'Enter'));
    expect(useSettingsStore.getState().activePageId).toBe('application.appearance');
    expect(input.value).toBe('');
    expect(container.querySelector('[role="listbox"]')).toBeNull();
  });

  it('restores grouped navigation after clearing an empty search', async () => {
    const input = await search('no-matching-page');
    expect(container.querySelector('[role="status"]')).not.toBeNull();
    act(() => pressKey(input, 'Escape'));
    expect(input.value).toBe('');
    expect(container.querySelectorAll('[data-testid="settings-nav-page"]')).toHaveLength(3);
    expect(container.querySelector('[role="status"]')).toBeNull();
  });
});
