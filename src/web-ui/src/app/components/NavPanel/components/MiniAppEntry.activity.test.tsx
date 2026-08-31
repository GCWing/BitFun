// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useMiniAppStore } from '@/app/scenes/miniapps/miniAppStore';
import MiniAppEntry from './MiniAppEntry';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/infrastructure/i18n/hooks/useI18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock('@/app/scenes/miniapps/utils/miniAppIcons', () => ({
  getMiniAppIconGradient: () => 'none',
  renderMiniAppIcon: () => null,
}));

function app(id: string): MiniAppMeta {
  return {
    id,
    name: id,
    description: '',
    icon: 'box',
    category: 'test',
    tags: [],
    version: 1,
    created_at: 1,
    updated_at: 1,
    permissions: { node: { enabled: false } },
  };
}

describe('MiniAppEntry activity', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    useMiniAppStore.setState({
      apps: [app('scene-only')],
      runningWorkerIds: [],
      customizingAppIds: [],
    });
    useSceneStore.setState({
      openTabs: [{ id: 'miniapp:scene-only', lastUsed: 1 }],
      activeTabId: 'miniapp:scene-only',
      navHistory: ['miniapp:scene-only'],
      navCursor: 0,
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    useMiniAppStore.setState({ apps: [], runningWorkerIds: [], customizingAppIds: [] });
    useSceneStore.setState({
      openTabs: [],
      activeTabId: null,
      navHistory: [],
      navCursor: -1,
    });
  });

  it('shows a node-less MiniApp while its Runner scene is open', () => {
    act(() => {
      root.render(
        <MiniAppEntry
          isActive
          activeMiniAppId="scene-only"
          onOpenMiniApps={() => {}}
          onOpenMiniApp={() => {}}
        />,
      );
    });

    const entry = container.querySelector('[data-testid="nav-miniapps-entry"]');
    const activity = container.querySelector(
      '[data-testid="nav-miniapp-activity-item"][data-miniapp-id="scene-only"]',
    );
    expect(entry?.classList.contains('has-running-apps')).toBe(true);
    expect(activity).not.toBeNull();
    expect(activity?.classList.contains('is-active')).toBe(true);
  });

  it('keeps an inactive open Runner visible as background activity', () => {
    useSceneStore.setState({
      openTabs: [
        { id: 'miniapp:scene-only', lastUsed: 1 },
        { id: 'miniapps', lastUsed: 2 },
      ],
      activeTabId: 'miniapps',
      navHistory: ['miniapps'],
      navCursor: 0,
    });

    act(() => {
      root.render(
        <MiniAppEntry
          isActive
          onOpenMiniApps={() => {}}
          onOpenMiniApp={() => {}}
        />,
      );
    });

    expect(container.querySelector(
      '[data-testid="nav-miniapp-activity-item"][data-miniapp-id="scene-only"]',
    )).not.toBeNull();
  });
});
