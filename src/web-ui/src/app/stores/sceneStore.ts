/**
 * sceneStore — SceneBar tab lifecycle + scene navigation history.
 *
 * Tab rules:
 *   - Every explicitly opened scene stays in openTabs until the user closes it.
 *   - Pinned tabs stay ahead of regular tabs; closeability is an independent
 *     scene-definition capability.
 *   - The app starts with no tabs. SceneViewport owns the tabless welcome
 *     surface until the first scene is explicitly opened.
 *
 * Navigation history (navHistory / navCursor):
 *   - Records the sequence of activeTabId changes.
 *   - goBack / goForward move the cursor and change activeTabId.
 *   - Both skip entries whose tabs have since been closed.
 *   - closeScene removes all history entries for the closed tab,
 *     so forward can never point to a closed tab.
 */

import { create } from 'zustand';
import {
  SCENE_TAB_REGISTRY,
  getSceneDef,
  getMiniAppSceneDef,
  isSceneTabClosable,
} from '../scenes/registry';
import { getSceneNav } from '../scenes/nav-registry';
import { useNavSceneStore } from './navSceneStore';
import {
  getInteractionMotion,
  type InteractionMotion,
} from '@/shared/utils/motionPreference';
import type { SceneTab, SceneTabId } from '../components/SceneBar/types';

function getSceneDefOrMiniapp(id: SceneTabId) {
  const d = getSceneDef(id);
  if (d) return d;
  if (typeof id === 'string' && id.startsWith('miniapp:')) {
    const appId = (id as string).slice('miniapp:'.length);
    return getMiniAppSceneDef(appId);
  }
  return undefined;
}

function isClosableScene(id: SceneTabId): boolean {
  return isSceneTabClosable(getSceneDefOrMiniapp(id));
}

function buildSceneTab(id: SceneTabId, now: number): SceneTab {
  return { id, lastUsed: now };
}

function resolveNavSceneId(sceneId: SceneTabId | null): SceneTabId | null {
  if (sceneId === null) return null;
  return getSceneNav(sceneId) ? sceneId : null;
}

interface SceneState {
  openTabs: SceneTab[];
  activeTabId: SceneTabId | null;
  /** Ordered history of activeTabId values. */
  navHistory: SceneTabId[];
  /** Index of the current position in navHistory. */
  navCursor: number;
  /** Input source for the latest active-scene change. */
  navigationMotion: InteractionMotion;
  navigationSequence: number;

  openScene:    (id: SceneTabId) => void;
  activateScene:(id: SceneTabId) => void;
  closeScene:   (id: SceneTabId) => void;
  goBack:       () => void;
  goForward:    () => void;
  /** Reset tabs/history when entering or exiting Peer Device Mode. */
  resetForPeerSwitch: () => void;
}

function buildDefaultTabs(): SceneTab[] {
  const now = Date.now();
  return SCENE_TAB_REGISTRY
    .filter(d => d.defaultOpen)
    .map(d => buildSceneTab(d.id, now));
}

/**
 * Keeps pinned tabs ahead of regular tabs without opening or protecting them.
 */
function orderPinnedTabsFirst(tabs: SceneTab[]): SceneTab[] {
  const pinnedTabs = tabs.filter(tab => getSceneDefOrMiniapp(tab.id)?.pinned);
  if (pinnedTabs.length === 0) return tabs;
  return [
    ...pinnedTabs,
    ...tabs.filter(tab => !getSceneDefOrMiniapp(tab.id)?.pinned),
  ];
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
const initialActiveId = initialTabs[0]?.id ?? null;

export const useSceneStore = create<SceneState>((set, get) => ({
  openTabs:    initialTabs,
  activeTabId: initialActiveId,
  navHistory:  initialActiveId ? [initialActiveId] : [],
  navCursor:   initialActiveId ? 0 : -1,
  navigationMotion: 'instant',
  navigationSequence: 0,

  openScene: (id) => {
    const state = get();
    const { activeTabId } = state;
    const navigationMotion = getInteractionMotion();

    // Already active — re-sync left nav in case user navigated back to MainNav
    if (id === activeTabId) {
      const navSceneId = resolveNavSceneId(id);
      const navStore = useNavSceneStore.getState();
      if (navSceneId && (!navStore.showSceneNav || navStore.navSceneId !== navSceneId)) {
        navStore.openNavScene(navSceneId);
      }
      return;
    }

    const isAlreadyOpen = state.openTabs.some(tab => tab.id === id);
    const def = getSceneDef(id);
    const isMiniappTab = typeof id === 'string' && id.startsWith('miniapp:');
    if (!isAlreadyOpen && !def && !isMiniappTab) return;

    const { openTabs, navHistory, navCursor } = state;

    const histUpdate = pushHistory(navHistory, navCursor, id);

    // Already open → just activate
    if (openTabs.some(tab => tab.id === id)) {
      const activatedAt = Date.now();
      set({
        activeTabId: id,
        openTabs: orderPinnedTabsFirst(openTabs.map(tab =>
          tab.id === id ? { ...tab, lastUsed: activatedAt } : tab
        )),
        navigationMotion,
        navigationSequence: state.navigationSequence + 1,
        ...histUpdate,
      });
      return;
    }

    const next = [...openTabs, buildSceneTab(id, Date.now())];
    set({
      openTabs: orderPinnedTabsFirst(next),
      activeTabId: id,
      navigationMotion,
      navigationSequence: state.navigationSequence + 1,
      ...histUpdate,
    });
  },

  activateScene: (id) => {
    get().openScene(id);
  },

  closeScene: (id) => {
    const state = get();
    const { openTabs, activeTabId, navHistory, navCursor } = state;
    if (!openTabs.some(tab => tab.id === id) || !isClosableScene(id)) return;

    const nextTabs = openTabs.filter(t => t.id !== id);
    if (nextTabs.length === 0) {
      set({
        openTabs: [],
        activeTabId: null,
        navHistory: [],
        navCursor: -1,
        navigationMotion: getInteractionMotion(),
        navigationSequence: state.navigationSequence + 1,
      });
      return;
    }

    const fallbackTabId = [...nextTabs].sort((a, b) => b.lastUsed - a.lastUsed)[0].id;
    const newActiveId = id === activeTabId
      ? fallbackTabId
      : activeTabId && nextTabs.some(tab => tab.id === activeTabId)
        ? activeTabId
        : fallbackTabId;

    const histUpdate = removeFromHistory(navHistory, navCursor, id, newActiveId);
    set({
      openTabs: orderPinnedTabsFirst(nextTabs),
      activeTabId: newActiveId,
      navigationMotion: getInteractionMotion(),
      navigationSequence: state.navigationSequence + 1,
      ...histUpdate,
    });
  },

  goBack: () => {
    const state = get();
    const { navHistory, navCursor, openTabs } = state;
    for (let i = navCursor - 1; i >= 0; i--) {
      const targetId = navHistory[i];
      if (openTabs.some(t => t.id === targetId)) {
        set(state => ({
          navCursor: i,
          activeTabId: targetId,
          navigationMotion: getInteractionMotion(),
          navigationSequence: state.navigationSequence + 1,
          openTabs: state.openTabs.map(t =>
            t.id === targetId ? { ...t, lastUsed: Date.now() } : t
          ),
        }));
        return;
      }
    }
  },

  goForward: () => {
    const state = get();
    const { navHistory, navCursor, openTabs } = state;
    for (let i = navCursor + 1; i < navHistory.length; i++) {
      const targetId = navHistory[i];
      if (openTabs.some(t => t.id === targetId)) {
        set(state => ({
          navCursor: i,
          activeTabId: targetId,
          navigationMotion: getInteractionMotion(),
          navigationSequence: state.navigationSequence + 1,
          openTabs: state.openTabs.map(t =>
            t.id === targetId ? { ...t, lastUsed: Date.now() } : t
          ),
        }));
        return;
      }
    }
  },

  resetForPeerSwitch: () => {
    const state = get();
    const tabs = buildDefaultTabs();
    const activeTabId = tabs[0]?.id ?? null;
    set({
      openTabs: tabs,
      activeTabId,
      navHistory: activeTabId ? [activeTabId] : [],
      navCursor: activeTabId ? 0 : -1,
      navigationMotion: 'instant',
      navigationSequence: state.navigationSequence + 1,
    });
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
      const navSceneId = resolveNavSceneId(state.activeTabId);
      const navStore = useNavSceneStore.getState();
      if (navSceneId) {
        navStore.openNavScene(navSceneId);
      } else {
        navStore.closeNavScene();
      }
    }
  });
}
