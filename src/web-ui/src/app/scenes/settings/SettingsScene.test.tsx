// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';

vi.mock('./settingsRegistry', () => {
  const pages = {
    'application.general': {
      id: 'application.general',
      categoryId: 'application',
      component: ({ viewId }: { viewId?: string }) => <div data-testid="general-page" data-view={viewId} />,
    },
    'application.appearance': {
      id: 'application.appearance',
      categoryId: 'application',
      component: () => <div data-testid="appearance-page" />,
    },
    'tools.automation': {
      id: 'tools.automation',
      categoryId: 'tools',
      component: ({ viewId }: { viewId?: string }) => <div data-testid="automation-page" data-view={viewId} />,
    },
  };
  return {
    DEFAULT_SETTINGS_PAGE_ID: 'application.general',
    getSettingsPageManifest: (pageId: keyof typeof pages) => pages[pageId] ?? pages['application.general'],
    isSettingsPageReady: () => true,
    preloadSettingsPage: vi.fn(async () => undefined),
  };
});

import SettingsScene from './SettingsScene';
import { useSettingsStore } from './settingsStore';

describe('SettingsScene canonical page routing', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    useSettingsStore.setState({
      activePageId: 'application.general',
      activeViewId: null,
      navigationRequestId: 0,
      pageTransitionTarget: null,
      pageTransitionMotion: 'instant',
      pageTransitionSequence: 0,
      searchQuery: '',
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('renders the active canonical page', async () => {
    await act(async () => root.render(<SettingsScene />));
    expect(container.querySelector('[data-testid="general-page"]')).not.toBeNull();
    expect(container.querySelector('[data-settings-page="application.general"]')).not.toBeNull();
  });

  it('passes an internal view destination without creating a sidebar page', async () => {
    useSettingsStore.getState().openDestination({
      pageId: 'tools.automation',
      viewId: 'hooks',
    });
    await act(async () => root.render(<SettingsScene />));
    expect(container.querySelector('[data-testid="automation-page"]')?.getAttribute('data-view')).toBe('hooks');
  });

  it('switches pages without retaining the outgoing page for instant navigation', async () => {
    await act(async () => root.render(<SettingsScene />));
    await act(async () => useSettingsStore.getState().openPage('application.appearance'));
    expect(container.querySelector('[data-testid="appearance-page"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="general-page"]')).toBeNull();
  });
});
