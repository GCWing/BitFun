// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/i18n', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return { useI18n: () => ({ t: createTestI18nT('common') }) };
});

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({ sessions: new Map() }),
    clearSessionUnreadCompletion: vi.fn(),
  },
}));

import WorkspaceSessionFilterMenu from './WorkspaceSessionFilterMenu';
import { useWorkspaceSessionViewStore } from '../workspaceSessionView';

describe('WorkspaceSessionFilterMenu', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    localStorage.clear();
    const view = useWorkspaceSessionViewStore.getState();
    view.setGrouping('grouped');
    view.setOrdering('updated');
    view.setShow('all');
    view.resetFilters();
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    act(() => root.render(<WorkspaceSessionFilterMenu />));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  });

  it('keeps filtering available while grouping lives in the separate quick toggle', () => {
    const filterButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="nav-session-filter-btn"]',
    );
    expect(filterButton).not.toBeNull();

    act(() => filterButton!.click());
    const workspaceMenu = document.querySelector<HTMLElement>(
      '[data-testid="nav-session-filter-menu"]',
    );
    expect(workspaceMenu).not.toBeNull();
    expect(workspaceMenu?.textContent).not.toContain('Grouping');
    expect(document.querySelector('[data-testid="nav-session-collapse-all"]')).not.toBeNull();

    act(() => filterButton!.click());
    act(() => useWorkspaceSessionViewStore.getState().setGrouping('all'));
    expect(filterButton?.className).not.toContain('is-active');

    act(() => filterButton!.click());
    expect(document.querySelector('[data-testid="nav-session-filter-menu"]')).not.toBeNull();
    expect(document.querySelector('[data-testid="nav-session-collapse-all"]')).toBeNull();
  });
});
