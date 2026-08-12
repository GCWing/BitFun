/**
 * @vitest-environment jsdom
 *
 * useTabLifecycle: expanded-grid (slot4..slot16) tab close coverage.
 *
 * Regression tests for the "multi-cell expanded window close does nothing"
 * bug: handleCloseWithDirtyCheck / handleCloseAllWithDirtyCheck used to
 * decode only primary/secondary/tertiary, so any tab living in a 4x4
 * extended cell (slot4..slot16) could never be found -> the close silently
 * returned without removing the tab. Both handlers now resolve the group
 * through the shared GROUP_STATE_KEY mapping (same single source of truth
 * as canvasStore), which covers all 16 slots.
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

type LifecycleApi = ReturnType<typeof useTabLifecycle>;

/** Mount the hook inside a real component (hooks must run inside a render). */
function mountLifecycle(): LifecycleApi {
  let api: LifecycleApi | null = null;
  const container = document.createElement('div');
  const Harness = () => {
    api = useTabLifecycle();
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

// addTab redirects to primary unless grid9 mode is active (canvasStore
// single-column guard), so every test enters grid9 first: slots become
// addressable exactly like the 4x4 expanded canvas they model.
function enterGrid9() {
  useAgentCanvasStore.getState().applyGrid9Template(4, 4);
}

describe('useTabLifecycle close handlers on expanded slots (slot4..slot16)', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it.each([
    'slot4', 'slot5', 'slot6', 'slot7', 'slot8', 'slot9',
    'slot10', 'slot11', 'slot12', 'slot13', 'slot14', 'slot15', 'slot16',
  ] as EditorGroupId[])(
    'handleCloseWithDirtyCheck closes a tab living in %s',
    async (slot) => {
      enterGrid9();
      const lifecycle = mountLifecycle();
      addTab('A', slot);
      const { id } = groupOf(slot).tabs[0];

      let closed = false;
      await act(async () => {
        closed = await lifecycle.handleCloseWithDirtyCheck(id, slot);
      });

      expect(closed).toBe(true);
      expect(groupOf(slot).tabs.some(t => t.id === id)).toBe(false);
    }
  );

  it('closes a tab in the active slot after switching to it (Ctrl+W path)', async () => {
    enterGrid9();
    const lifecycle = mountLifecycle();
    addTab('X', 'primary');
    addTab('Y', 'slot9');
    // Switch to slot9 so it becomes the active group (mirrors clicking into an
    // expanded cell then hitting Ctrl+W).
    const tabY = groupOf('slot9').tabs.find(t => t.title === 'Y')!;
    useAgentCanvasStore.getState().switchToTab(tabY.id, 'slot9');

    let closed = false;
    await act(async () => {
      closed = await lifecycle.handleCloseWithDirtyCheck(tabY.id, 'slot9');
    });

    expect(closed).toBe(true);
    expect(groupOf('slot9').tabs.some(t => t.id === tabY.id)).toBe(false);
    expect(groupOf('primary').tabs.some(t => t.title === 'X')).toBe(true);
  });

  it.each([
    'slot4', 'slot8', 'slot12', 'slot16',
  ] as EditorGroupId[])(
    'handleCloseAllWithDirtyCheck closes all unpinned tabs in %s',
    async (slot) => {
      enterGrid9();
      const lifecycle = mountLifecycle();
      addTab('A', slot);
      addTab('B', slot);
      useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title: 'P', data: {} }, 'pinned', slot);

      let closedAll = false;
      await act(async () => {
        closedAll = await lifecycle.handleCloseAllWithDirtyCheck(slot);
      });

      expect(closedAll).toBe(true);
      const tabs = groupOf(slot).tabs;
      expect(tabs.some(t => t.title === 'A')).toBe(false);
      expect(tabs.some(t => t.title === 'B')).toBe(false);
      // Pinned tabs survive (same semantics as closeAllTabs).
      expect(tabs.some(t => t.title === 'P')).toBe(true);
    }
  );

  it('close handlers no-op safely for an unknown tab id', async () => {
    enterGrid9();
    const lifecycle = mountLifecycle();
    addTab('A', 'slot7');

    let closed = false;
    await act(async () => {
      closed = await lifecycle.handleCloseWithDirtyCheck('missing-id', 'slot7');
    });

    expect(closed).toBe(true);
    expect(groupOf('slot7').tabs.some(t => t.title === 'A')).toBe(true);
  });
});
