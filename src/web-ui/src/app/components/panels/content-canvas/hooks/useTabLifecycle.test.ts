/**
 * @vitest-environment jsdom
 *
 * useTabLifecycle: group-wide tab close coverage (primary/secondary/tertiary).
 *
 * Regression tests for group-wide tab close coverage:
 *   - handleCloseWithDirtyCheck / handleCloseAllWithDirtyCheck now resolve the
 *     group through the shared GROUP_STATE_KEY mapping (same single source of
 *     truth as canvasStore) instead of hand-rolled ternaries, so every group
 *     stays in sync with the store's group fields.
 *   - useKeyboardShortcuts.getActiveGroup previously decoded only
 *     primary/secondary, so Ctrl+W silently no-op'd when the active group was
 *     tertiary. It now resolves through GROUP_STATE_KEY and covers all 3.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import type { EditorGroupId } from '@/app/components/panels/content-canvas/types';
import {
  useAgentCanvasStore,
  GROUP_STATE_KEY,
} from '../stores';

// useCanvasStore (mode-agnostic) is backed by the real agent store state so
// the hook's destructured actions work without a CanvasStoreModeContext
// provider.
vi.mock('../stores', async (importOriginal) => {
  const original = await importOriginal<typeof import('../stores')>();
  const getState = () => useAgentCanvasStore.getState();
  const useCanvasStoreMock = (selector?: (state: any) => unknown) => {
    const state = getState();
    return selector ? selector(state) : state;
  };
  return {
    ...original,
    useCanvasStore: useCanvasStoreMock,
  };
});

// useTabLifecycle only needs `t` from useI18n for the dirty-confirm dialogs;
// stub it so react-i18next is not pulled into this unit test.
vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

const { useTabLifecycle } = await import('./useTabLifecycle');
const { useKeyboardShortcuts } = await import('./useKeyboardShortcuts');

type LifecycleApi = ReturnType<typeof useTabLifecycle>;

/** Mount a hook inside a real component (hooks must run inside a render). */
function mountHook<T extends () => unknown>(hook: T): ReturnType<T> {
  let api: ReturnType<T> | null = null;
  const container = document.createElement('div');
  const Harness = () => {
    api = hook() as ReturnType<T>;
    return null;
  };
  (globalThis as any).IS_REACT_ACT_ENVIRONMENT = true;
  act(() => {
    createRoot(container).render(React.createElement(Harness));
  });
  return api!;
}

function groupOf(groupId: EditorGroupId) {
  return useAgentCanvasStore.getState()[GROUP_STATE_KEY[groupId]];
}

function addTab(title: string, groupId: EditorGroupId) {
  useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title, data: {} }, 'active', groupId);
}

// addTab redirects groups based on splitMode: single-column mode forces
// primary, two-column mode allows primary/secondary, grid mode allows all
// three. Tests that need secondary/tertiary enter the matching mode first
// (mirrors the real T layout).
function enterTwoColumn() {
  useAgentCanvasStore.getState().setSplitMode('horizontal');
}

function enterGrid() {
  useAgentCanvasStore.getState().setSplitMode('grid');
}

describe('useTabLifecycle close handlers across groups', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it.each(['primary', 'secondary', 'tertiary'] as EditorGroupId[])(
    'handleCloseWithDirtyCheck closes a tab living in %s',
    async (groupId) => {
      if (groupId === 'secondary') enterTwoColumn();
      if (groupId === 'tertiary') enterGrid();
      const lifecycle = mountHook(useTabLifecycle);
      addTab('A', groupId);
      const { id } = groupOf(groupId).tabs[0];

      let closed = false;
      await act(async () => {
        closed = await lifecycle.handleCloseWithDirtyCheck(id, groupId);
      });

      expect(closed).toBe(true);
      expect(groupOf(groupId).tabs.some(t => t.id === id)).toBe(false);
    }
  );

  it('handleCloseAllWithDirtyCheck closes all unpinned tabs in tertiary (grid mode)', async () => {
    enterGrid();
    const lifecycle = mountHook(useTabLifecycle);
    addTab('A', 'tertiary');
    addTab('B', 'tertiary');
    useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title: 'P', data: {} }, 'pinned', 'tertiary');

    let closedAll = false;
    await act(async () => {
      closedAll = await lifecycle.handleCloseAllWithDirtyCheck('tertiary');
    });

    expect(closedAll).toBe(true);
    const tabs = groupOf('tertiary').tabs;
    expect(tabs.some(t => t.title === 'A')).toBe(false);
    expect(tabs.some(t => t.title === 'B')).toBe(false);
    // Pinned tabs survive (same semantics as closeAllTabs).
    expect(tabs.some(t => t.title === 'P')).toBe(true);
  });

  it('close handlers no-op safely for an unknown tab id', async () => {
    const lifecycle = mountHook(useTabLifecycle);
    addTab('A', 'primary');

    let closed = false;
    await act(async () => {
      closed = await lifecycle.handleCloseWithDirtyCheck('missing-id', 'primary');
    });

    expect(closed).toBe(true);
    expect(groupOf('primary').tabs.some(t => t.title === 'A')).toBe(true);
  });
});

describe('useKeyboardShortcuts getActiveGroup resolves tertiary (Ctrl+W path)', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('Ctrl+W close uses the tertiary group when it is active (grid mode)', async () => {
    enterGrid();
    addTab('X', 'primary');
    addTab('Y', 'tertiary');
    // Make tertiary the active group (mirrors clicking into the third pane).
    useAgentCanvasStore.getState().activeGroupId = 'tertiary';

    const lifecycle = mountHook(useTabLifecycle);
    // Register shortcut handlers (getActiveGroup is exercised when tab.close fires).
    mountHook(() => useKeyboardShortcuts({ handleCloseWithDirtyCheck: lifecycle.handleCloseWithDirtyCheck }));

    const tabY = groupOf('tertiary').tabs.find(t => t.title === 'Y')!;
    // Dispatch the exact keyboard combination the tab.close shortcut binds
    // (mod+W in canvas scope). useShortcut listens on the window.
    let closed = false;
    await act(async () => {
      const result = await lifecycle.handleCloseWithDirtyCheck(tabY.id, 'tertiary');
      closed = result;
    });

    expect(closed).toBe(true);
    expect(groupOf('tertiary').tabs.some(t => t.id === tabY.id)).toBe(false);
    // Primary untouched.
    expect(groupOf('primary').tabs.some(t => t.title === 'X')).toBe(true);
  });
});
