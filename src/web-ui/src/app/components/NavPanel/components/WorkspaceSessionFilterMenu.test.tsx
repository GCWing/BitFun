// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@openbitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@openbitfun/ui')>(),
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
    vi.useFakeTimers();
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
    vi.useRealTimers();
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

  it.each([
    { side: 'right', parentLeft: 30, submenuLeft: 255, gapX: 252 },
    { side: 'left', parentLeft: 300, submenuLeft: 75, gapX: 298 },
  ])('preserves the $side Portal submenu across a pause in the gap in both directions', ({ parentLeft, submenuLeft, gapX }) => {
    act(() => container.querySelector<HTMLButtonElement>('[data-testid="nav-session-filter-btn"]')!.click());
    const menu = document.querySelector<HTMLElement>('[data-testid="nav-session-filter-menu"]')!;
    const ordering = menu.querySelector<HTMLButtonElement>('[aria-haspopup="menu"]')!;
    act(() => ordering.click());
    const submenu = document.querySelector<HTMLElement>('[data-testid="nav-session-filter-ordering-menu"]')!;
    expect(container.contains(submenu)).toBe(false);

    for (const [element, left] of [[menu, parentLeft], [submenu, submenuLeft]] as const) {
      element.getBoundingClientRect = () => ({
        left, right: left + 220, top: 20, bottom: 220,
        width: 220, height: 200, x: left, y: 20, toJSON: () => ({}),
      });
    }

    const pauseInGapFrom = (element: HTMLElement) => {
      act(() => {
        element.dispatchEvent(new MouseEvent('pointerout', {
          bubbles: true, relatedTarget: document.body, clientX: gapX, clientY: 80,
        }));
        document.body.dispatchEvent(new MouseEvent('pointermove', {
          bubbles: true, clientX: gapX, clientY: 80,
        }));
      });
      act(() => vi.advanceTimersByTime(1000));
      expect(document.querySelector('[data-testid="nav-session-filter-ordering-menu"]')).toBe(submenu);
    };

    pauseInGapFrom(ordering);
    act(() => submenu.dispatchEvent(new MouseEvent('pointerover', {
      bubbles: true, relatedTarget: document.body, clientX: submenuLeft + 20, clientY: 80,
    })));
    pauseInGapFrom(submenu);

    act(() => document.body.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true, clientX: gapX, clientY: 400,
    })));
    act(() => vi.advanceTimersByTime(300));
    expect(document.querySelector('[data-testid="nav-session-filter-ordering-menu"]')).toBeNull();
  });
});
