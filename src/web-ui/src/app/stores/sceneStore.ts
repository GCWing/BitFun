/**
 * sceneStore — SceneBar tab lifecycle + scene navigation history.
 *
 * Tab rules:
 *   - Max MAX_OPEN_SCENES tabs total (pinned + dynamic).
 *   - Pinned tabs cannot be auto-evicted; users can still close manually.
 *   - Dynamic tabs are evicted by LRU when limit is exceeded.
 *   - 'welcome' tab is the default initial tab; it auto-closes the first time
 *     any other scene is explicitly opened.
 *
 * Navigation history (navHistory / navCursor):
 *   - Records the sequence of activeTabId changes.
 *   - goBack / goForward move the cursor and change activeTabId.
 *   - Both skip entries whose tabs have since been closed.
 *   - closeScene removes all history entries for the closed tab,
 *     so forward can never point to a closed tab.
 */

import { create } from 'zustand';
import { SCENE_TAB_REGISTRY, MAX_OPEN_SCENES, getSceneDef } from '../scenes/registry';
import { getSceneNav } from '../scenes/nav-registry';
import { useNavSceneStore } from './navSceneStore';
import type { SceneTab, SceneTabId } from '../components/SceneBar/types';

const AGENT_SCENE_ID: SceneTabId = 'session';
const WELCOME_SCENE_ID: SceneTabId = 'welcome';

interface SceneState {
  openTabs: SceneTab[];
  activeTabId: SceneTabId;
  /** Ordered history of activeTabId values. */
  navHistory: SceneTabId[];
  /** Index of the current position in navHistory. */
  navCursor: number;

  openScene:    (id: SceneTabId) => void;
  activateScene:(id: SceneTabId) => void;
  closeScene:   (id: SceneTabId) => void;
  goBack:       () => void;
  goForward:    () => void;
}

function buildDefaultTabs(): SceneTab[] {
  return SCENE_TAB_REGISTRY
    .filter(d => d.defaultOpen)
    .map(d => ({ id: d.id, lastUsed: Date.now() }));
}

/**
 * Ensures the session tab is first in the display order when present,
 * but never auto-adds it — session only appears when explicitly opened.
 */
function ensureAgentFirst(tabs: SceneTab[]): SceneTab[] {
  const agentTab = tabs.find(tab => tab.id === AGENT_SCENE_ID);
  if (!agentTab) return tabs;
  return [agentTab, ...tabs.filter(tab => tab.id !== AGENT_SCENE_ID)];
}

/** Push id to history, trimming any forward entries. Deduplicates consecutive same id. */
function pushHistory(history: SceneTabId[], cursor: number, id: SceneTabId) {
  const trimmed = history.slice(0, cursor + 1);
  if (trimmed[trimmed.length - 1] === id) {
    return { navHistory: trimmed, navCursor: trimmed.length - 1 };
  }
  return { navHistory: [...trimmed, id], navCursor: trimmed.length };
}

/** Remove all occurrences of id from history and recalculate cursor. */
function removeFromHistory(
  history: SceneTabId[],
  cursor: number,
  removedId: SceneTabId,
  newActiveId: SceneTabId,
) {
  const newHistory = history.filter(h => h !== removedId);
  if (newHistory.length === 0) return { navHistory: [] as SceneTabId[], navCursor: -1 };
  const idx = newHistory.lastIndexOf(newActiveId);
  const newCursor = idx !== -1 ? idx : Math.min(cursor, newHistory.length - 1);
  return { navHistory: newHistory, navCursor: newCursor };
}

const initialTabs = buildDefaultTabs();
const initialActiveId: SceneTabId = initialTabs[0]?.id ?? WELCOME_SCENE_ID;

export const useSceneStore = create<SceneState>((set, get) => ({
  openTabs:    initialTabs,
  activeTabId: initialActiveId,
  navHistory:  [initialActiveId],
  navCursor:   0,

  openScene: (id) => {
    // Auto-close welcome tab when any other scene is explicitly opened
    if (id !== WELCOME_SCENE_ID) {
      const state = get();
      if (state.openTabs.some(t => t.id === WELCOME_SCENE_ID)) {
        const tabsWithoutWelcome = state.openTabs.filter(t => t.id !== WELCOME_SCENE_ID);
        const histWithoutWelcome = state.navHistory.filter(h => h !== WELCOME_SCENE_ID);

        // If the first opened scene is not session, companion-open session alongside it
        let companionTabs = tabsWithoutWelcome;
        if (id !== AGENT_SCENE_ID && !tabsWithoutWelcome.some(t => t.id === AGENT_SCENE_ID)) {
          companionTabs = [{ id: AGENT_SCENE_ID, lastUsed: 0 }, ...tabsWithoutWelcome];
        }

        set({
          openTabs:   ensureAgentFirst(companionTabs),
          navHistory: histWithoutWelcome,
          navCursor:  Math.max(0, histWithoutWelcome.length - 1),
        });
      }
    }

    const { openTabs, activeTabId, navHistory, navCursor } = get();

    // Already active — re-sync left nav in case user navigated back to MainNav
    if (id === activeTabId) {
      const hasNav = !!getSceneNav(id);
      const navStore = useNavSceneStore.getState();
      if (hasNav && !navStore.showSceneNav) {
        navStore.openNavScene(id);
      }
      return;
    }

    const histUpdate = pushHistory(navHistory, navCursor, id);

    // Already open → just activate
    if (openTabs.find(t => t.id === id)) {
      set(state => ({
        activeTabId: id,
        openTabs: state.openTabs.map(t =>
          t.id === id ? { ...t, lastUsed: Date.now() } : t
        ),
        ...histUpdate,
      }));
      return;
    }

    const def = getSceneDef(id);
    if (!def) return;

    let next = [...openTabs];

    // LRU eviction if at capacity
    if (next.length >= MAX_OPEN_SCENES) {
      const evictable = next
        .filter(t => {
          const d = getSceneDef(t.id);
          return !d?.pinned && t.id !== activeTabId;
        })
        .sort((a, b) => a.lastUsed - b.lastUsed);

      if (evictable.length === 0) return;
      const evictedId = evictable[0].id;
      next = next.filter(t => t.id !== evictedId);
      const afterEvict = removeFromHistory(
        histUpdate.navHistory,
        histUpdate.navCursor,
        evictedId,
        id,
      );
      Object.assign(histUpdate, afterEvict);
    }

    next.push({ id, lastUsed: Date.now() });
    set({ openTabs: ensureAgentFirst(next), activeTabId: id, ...histUpdate });
  },

  activateScene: (id) => {
    get().openScene(id);
  },

  closeScene: (id) => {
    const { openTabs, activeTabId, navHistory, navCursor } = get();
    const def = getSceneDef(id);
    if (def?.pinned) return;

    const nextTabs = openTabs.filter(t => t.id !== id);

    let newActiveId = activeTabId;
    if (id === activeTabId) {
      if (nextTabs.length === 0) {
        set({ openTabs: [], activeTabId: '' as SceneTabId, navHistory: [], navCursor: -1 });
        return;
      }
      newActiveId = [...nextTabs].sort((a, b) => b.lastUsed - a.lastUsed)[0].id;
    }

    const histUpdate = removeFromHistory(navHistory, navCursor, id, newActiveId);
    set({ openTabs: ensureAgentFirst(nextTabs), activeTabId: newActiveId, ...histUpdate });
  },

  goBack: () => {
    const { navHistory, navCursor, openTabs } = get();
    for (let i = navCursor - 1; i >= 0; i--) {
      const targetId = navHistory[i];
      if (openTabs.some(t => t.id === targetId)) {
        set(state => ({
          navCursor: i,
          activeTabId: targetId,
          openTabs: state.openTabs.map(t =>
            t.id === targetId ? { ...t, lastUsed: Date.now() } : t
          ),
        }));
        return;
      }
    }
  },

  goForward: () => {
    const { navHistory, navCursor, openTabs } = get();
    for (let i = navCursor + 1; i < navHistory.length; i++) {
      const targetId = navHistory[i];
      if (openTabs.some(t => t.id === targetId)) {
        set(state => ({
          navCursor: i,
          activeTabId: targetId,
          openTabs: state.openTabs.map(t =>
            t.id === targetId ? { ...t, lastUsed: Date.now() } : t
          ),
        }));
        return;
      }
    }
  },
}));

/** Whether there's a valid back destination in history. */
export function selectCanGoBack(state: SceneState): boolean {
  const { navHistory, navCursor, openTabs } = state;
  for (let i = navCursor - 1; i >= 0; i--) {
    if (openTabs.some(t => t.id === navHistory[i])) return true;
  }
  return false;
}

/** Whether there's a valid forward destination in history. */
export function selectCanGoForward(state: SceneState): boolean {
  const { navHistory, navCursor, openTabs } = state;
  for (let i = navCursor + 1; i < navHistory.length; i++) {
    if (openTabs.some(t => t.id === navHistory[i])) return true;
  }
  return false;
}

if (typeof window !== 'undefined') {
  window.addEventListener('scene:open', (e: Event) => {
    const detail = (e as CustomEvent<{ sceneId: SceneTabId }>).detail;
    const sceneId = detail?.sceneId;
    if (sceneId) {
      useSceneStore.getState().openScene(sceneId);
    }
  });
}

// ── Sync right-side scene → left-side nav ─────────────────────────────────
{
  let prev = useSceneStore.getState().activeTabId;
  useSceneStore.subscribe((state) => {
    if (state.activeTabId !== prev) {
      prev = state.activeTabId;
      const hasNav = !!getSceneNav(state.activeTabId);
      const navStore = useNavSceneStore.getState();
      if (hasNav) {
        navStore.openNavScene(state.activeTabId);
      } else {
        navStore.closeNavScene();
      }
    }
  });
}
