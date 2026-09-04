// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { InteractionMotion } from '@/shared/utils/motionPreference';
import type { SceneTabId } from './types';
import SceneBar from './SceneBar';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const sceneHarness = vi.hoisted(() => ({
  state: {
    openTabs: [
      { id: 'session' as const, lastUsed: 1 },
      { id: 'settings' as const, lastUsed: 2 },
      { id: 'terminal' as const, lastUsed: 3 },
      { id: 'git' as const, lastUsed: 4 },
    ],
    activeTabId: 'session' as SceneTabId,
    navigationMotion: 'instant' as InteractionMotion,
    sessionTitle: undefined as string | undefined,
    tabDefs: [
      { id: 'session' as const, label: 'Session', Icon: () => null, pinned: true, closable: true, singleton: true, defaultOpen: false },
      { id: 'settings' as const, label: 'Settings', pinned: false, singleton: true, defaultOpen: false },
      { id: 'terminal' as const, label: 'Terminal', pinned: false, singleton: true, defaultOpen: false },
      { id: 'git' as const, label: 'Git', pinned: false, singleton: true, defaultOpen: false },
    ],
  },
  activateScene: vi.fn(),
  closeScene: vi.fn(),
}));

vi.mock('../../hooks/useSceneManager', () => ({
  useSceneManager: () => ({
    ...sceneHarness.state,
    activateScene: sceneHarness.activateScene,
    closeScene: sceneHarness.closeScene,
  }),
}));

vi.mock('../../hooks/useCurrentSessionTitle', () => ({
  useCurrentSessionTitle: () => sceneHarness.state.sessionTitle ?? '',
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock('@/infrastructure/runtime', () => ({
  supportsNativeWindowDragging: () => false,
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({ debug: vi.fn() }),
}));

vi.mock('@openbitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@openbitfun/ui')>(),
  Tooltip: ({ children }: { children: React.ReactNode }) => children,
}));

vi.mock('@/app/components/WindowControls', () => ({
  WindowControls: () => null,
}));

describe('SceneBar overflow navigation', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    sceneHarness.state.activeTabId = 'session';
    sceneHarness.state.navigationMotion = 'instant';
    sceneHarness.state.sessionTitle = undefined;
    sceneHarness.activateScene.mockReset();
    sceneHarness.closeScene.mockReset();
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  function renderSceneBar() {
    act(() => root.render(<SceneBar />));
  }

  function setOverflowMetrics(tabs: HTMLElement, region: HTMLElement) {
    Object.defineProperty(region, 'clientWidth', { configurable: true, value: 240 });
    Object.defineProperty(tabs, 'clientWidth', { configurable: true, value: 180 });
    Object.defineProperty(tabs, 'scrollWidth', { configurable: true, value: 620 });
    Object.defineProperty(tabs, 'scrollLeft', { configurable: true, value: 0, writable: true });
  }

  it('uses the real session title as the single label and keeps Settings static', () => {
    sceneHarness.state.sessionTitle = 'Investigate top tabs';
    renderSceneBar();

    const sessionTab = container.querySelector<HTMLElement>('[role="tab"][data-openbitfun-value="session"]')!;
    const settingsTab = container.querySelector<HTMLElement>('[role="tab"][data-openbitfun-value="settings"]')!;

    expect(sessionTab.querySelector('.openbitfun-scene-bar__tab-title')?.textContent)
      .toBe('Investigate top tabs');
    expect(settingsTab.querySelector('.openbitfun-scene-bar__tab-title')?.textContent)
      .toBe('Settings');
    expect(sessionTab.querySelector('[data-openbitfun-part="icon"]')).toBeNull();
    expect(sessionTab.closest('[data-openbitfun-part="item"]')?.getAttribute('data-has-icon'))
      .toBe('false');
    expect(container.querySelector('[data-scene-bar-part="tabs"]')?.getAttribute('data-size'))
      .toBe('sm');
    expect(container.querySelector('.openbitfun-scene-bar__tab-subtitle')).toBeNull();
    expect(container.querySelector('.openbitfun-scene-bar__tab-separator')).toBeNull();
  });

  it('delegates arrow and Home/End navigation to TabGroup', () => {
    renderSceneBar();
    const sessionTab = container.querySelector<HTMLElement>('[role="tab"][data-openbitfun-value="session"]');
    const settingsTab = container.querySelector<HTMLElement>('[role="tab"][data-openbitfun-value="settings"]');
    expect(sessionTab).not.toBeNull();
    expect(settingsTab).not.toBeNull();

    sessionTab!.focus();
    act(() => {
      sessionTab!.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'ArrowRight',
        bubbles: true,
        cancelable: true,
      }));
    });

    expect(sceneHarness.activateScene).toHaveBeenCalledWith('settings');
    expect(document.activeElement).toBe(settingsTab);

    act(() => {
      settingsTab!.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'End',
        bubbles: true,
        cancelable: true,
      }));
    });
    expect(sceneHarness.activateScene).toHaveBeenLastCalledWith('git');
  });

  it('exposes overflow controls and translates a vertical wheel into horizontal movement', () => {
    renderSceneBar();
    const region = container.querySelector<HTMLElement>('[data-openbitfun-component="scene-bar"][data-openbitfun-part="tabs"]')!;
    const tabs = container.querySelector<HTMLElement>('[data-scene-bar-part="tabs"]')!;
    setOverflowMetrics(tabs, region);

    act(() => tabs.dispatchEvent(new Event('scroll')));

    expect(region.dataset.overflow).toBe('true');
    expect(container.querySelector('[data-openbitfun-part="scrollPrevious"]')).not.toBeNull();
    expect(container.querySelector('[data-openbitfun-part="scrollNext"]')).not.toBeNull();

    act(() => {
      tabs.dispatchEvent(new WheelEvent('wheel', {
        deltaY: 72,
        bubbles: true,
        cancelable: true,
      }));
    });

    expect(tabs.scrollLeft).toBe(72);
    expect(container.querySelector<HTMLButtonElement>('[data-openbitfun-part="scrollPrevious"]')?.disabled).toBe(false);
  });

  it('scrolls a newly active off-screen tab into view', () => {
    renderSceneBar();
    const region = container.querySelector<HTMLElement>('[data-openbitfun-component="scene-bar"][data-openbitfun-part="tabs"]')!;
    const tabs = container.querySelector<HTMLElement>('[data-scene-bar-part="tabs"]')!;
    const gitTab = container.querySelector<HTMLElement>('[role="tab"][data-openbitfun-value="git"]')!;
    const gitItem = gitTab.closest<HTMLElement>('[data-openbitfun-part="item"]')!;
    setOverflowMetrics(tabs, region);
    Object.defineProperty(gitItem, 'offsetLeft', { configurable: true, value: 420 });
    Object.defineProperty(gitItem, 'offsetWidth', { configurable: true, value: 100 });
    const scrollTo = vi.fn(({ left }: ScrollToOptions) => {
      tabs.scrollLeft = left ?? tabs.scrollLeft;
    });
    Object.defineProperty(tabs, 'scrollTo', { configurable: true, value: scrollTo });

    sceneHarness.state.activeTabId = 'git';
    sceneHarness.state.navigationMotion = 'pointer';
    act(() => root.render(<SceneBar />));

    expect(scrollTo).toHaveBeenCalledWith({ left: 340, behavior: 'smooth' });
  });

  it('renders close actions from closeability metadata, including the leading session tab', () => {
    renderSceneBar();
    const sessionTab = container.querySelector<HTMLElement>('[role="tab"][data-openbitfun-value="session"]')!;
    const sessionItem = sessionTab.closest<HTMLElement>('[data-openbitfun-part="item"]')!;
    const sessionCloseButton = sessionItem.querySelector<HTMLButtonElement>('[data-scene-bar-part="closeTab"]')!;
    const settingsTab = container.querySelector<HTMLElement>('[role="tab"][data-openbitfun-value="settings"]')!;
    const settingsItem = settingsTab.closest<HTMLElement>('[data-openbitfun-part="item"]')!;
    const closeButton = settingsItem.querySelector<HTMLButtonElement>('[data-scene-bar-part="closeTab"]')!;

    expect(sessionCloseButton).not.toBeNull();
    act(() => sessionCloseButton.click());
    expect(sceneHarness.closeScene).toHaveBeenCalledWith('session');

    expect(settingsTab.contains(closeButton)).toBe(false);
    act(() => closeButton.click());
    expect(sceneHarness.closeScene).toHaveBeenCalledWith('settings');
  });

  it('keeps close targets out of press transforms so pointer hit testing stays stable', () => {
    renderSceneBar();
    const closeButtons = container.querySelectorAll<HTMLButtonElement>('[data-scene-bar-part="closeTab"]');

    expect(closeButtons.length).toBeGreaterThan(0);
    for (const closeButton of closeButtons) {
      expect(closeButton.dataset.motion).toBe('none');
    }
  });

  it('supports standard middle-click and Delete-key close interactions for session', () => {
    renderSceneBar();
    const sessionTab = container.querySelector<HTMLElement>('[role="tab"][data-openbitfun-value="session"]')!;

    act(() => {
      sessionTab.dispatchEvent(new MouseEvent('auxclick', {
        button: 1,
        bubbles: true,
        cancelable: true,
      }));
    });
    expect(sceneHarness.closeScene).toHaveBeenCalledWith('session');

    sceneHarness.closeScene.mockReset();
    act(() => {
      sessionTab.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'Delete',
        bubbles: true,
        cancelable: true,
      }));
    });
    expect(sceneHarness.closeScene).toHaveBeenCalledWith('session');
  });
});
