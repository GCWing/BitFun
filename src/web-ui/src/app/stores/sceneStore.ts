/**
 * sceneStore — SceneBar tab lifecycle + scene navigation history.
 *
 * Tab rules:
 *   - Max MAX_OPEN_SCENES visible tabs total (including fixed agent).
 *   - Fixed tabs (e.g. session/agent) are never auto-evicted; closable controls manual close.
 *   - When over capacity, the oldest replaceable tab (by openedAt, FIFO) is evicted.
 *   - Scenes with retainOnAutoEvict stay mounted in the background until an
 *     explicit close. MiniApp Runners use this so tab capacity cannot stop an app.
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
import { SCENE_TAB_REGISTRY, MAX_OPEN_SCENES, getSceneDef, getMiniAppSceneDef } from '../scenes/registry';
import { getSceneNav } from '../scenes/nav-registry';
import { useNavSceneStore } from './navSceneStore';
import {
  getInteractionMotion,
  type InteractionMotion,
} from '@/shared/utils/motionPreference';
import type { SceneTab, SceneTabId } from '../components/SceneBar/types';

const AGENT_SCENE_ID: SceneTabId = 'session';
const WELCOME_SCENE_ID: SceneTabId = 'welcome';

function getSceneDefOrMiniapp(id: SceneTabId) {
  const d = getSceneDef(id);
  if (d) return d;
  if (typeof id === 'string' && id.startsWith('miniapp:')) {
    const appId = (id as string).slice('miniapp:'.length);
    return getMiniAppSceneDef(appId);
  }
  return undefined;
}

function isFixedScene(id: SceneTabId): boolean {
  return getSceneDefOrMiniapp(id)?.fixed === true;
}

function isClosableScene(id: SceneTabId): boolean {
  const def = getSceneDefOrMiniapp(id);
  return (def?.closable ?? !def?.pinned) !== false;
}

function shouldRetainOnAutoEvict(id: SceneTabId): boolean {
  return getSceneDefOrMiniapp(id)?.retainOnAutoEvict === true;
}

/** Pick the oldest replaceable tab by openedAt (FIFO). Fixed tabs are never replaceable. */
function selectOldestReplaceableTab(tabs: SceneTab[]): SceneTab | undefined {
  const replaceable = tabs
    .filter(t => !isFixedScene(t.id))
    .sort((a, b) => a.openedAt - b.openedAt);
  return replaceable[0];
}

function buildSceneTab(id: SceneTabId, now: number): SceneTab {
  return { id, openedAt: now, lastUsed: now };
}

function resolveNavSceneId(sceneId: SceneTabId): SceneTabId | null {
  if (sceneId === 'terminal') {
    return 'shell';
  }

  return getSceneNav(sceneId) ? sceneId : null;
}

interface SceneState {
  openTabs: SceneTab[];
  /** Mounted scenes hidden by automatic visible-tab eviction. */
  retainedScenes: SceneTab[];
  activeTabId: SceneTabId;
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
  retainedScenes: [],
  activeTabId: initialActiveId,
  navHistory:  [initialActiveId],
  navCursor:   0,
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
    const retainedScene = state.retainedScenes.find(scene => scene.id === id);
    const def = getSceneDef(id);
    const isMiniappTab = typeof id === 'string' && id.startsWith('miniapp:');
    if (!isAlreadyOpen && !retainedScene && !def && !isMiniappTab) return;

    let openTabs = state.openTabs;
    let retainedScenes = state.retainedScenes;
    let navHistory = state.navHistory;
    let navCursor = state.navCursor;

    // Compute welcome removal and the target activation as one store update.
    // Publishing the intermediate "welcome is active but no longer mounted"
    // snapshot gives React a blank viewport that looks like a full page refresh.
    if (id !== WELCOME_SCENE_ID && openTabs.some(tab => tab.id === WELCOME_SCENE_ID)) {
      openTabs = openTabs.filter(tab => tab.id !== WELCOME_SCENE_ID);
      navHistory = navHistory.filter(historyId => historyId !== WELCOME_SCENE_ID);
      navCursor = Math.max(0, navHistory.length - 1);

      // If the first opened scene is not session, companion-open session alongside it.
      if (id !== AGENT_SCENE_ID && !openTabs.some(tab => tab.id === AGENT_SCENE_ID)) {
        openTabs = [buildSceneTab(AGENT_SCENE_ID, 0), ...openTabs];
      }
    }

    const histUpdate = pushHistory(navHistory, navCursor, id);

    // Already open → just activate
    if (openTabs.some(tab => tab.id === id)) {
      const activatedAt = Date.now();
      set({
        activeTabId: id,
        openTabs: ensureAgentFirst(openTabs.map(tab =>
          tab.id === id ? { ...tab, lastUsed: activatedAt } : tab
        )),
        navigationMotion,
        navigationSequence: state.navigationSequence + 1,
        ...histUpdate,
      });
      return;
    }

    let next = [...openTabs];

    // Eviction: over capacity → remove oldest replaceable tab (FIFO); fixed (e.g. agent) never evicted
    if (next.length >= MAX_OPEN_SCENES) {
      const victim = selectOldestReplaceableTab(next);
      if (!victim) return;
      const evictedId = victim.id;
      next = next.filter(tab => tab.id !== evictedId);
      if (
        shouldRetainOnAutoEvict(evictedId)
        && !retainedScenes.some(scene => scene.id === evictedId)
      ) {
        retainedScenes = [...retainedScenes, victim];
      }
      const afterEvict = removeFromHistory(
        histUpdate.navHistory,
        histUpdate.navCursor,
        evictedId,
        id,
      );
      Object.assign(histUpdate, afterEvict);
    }

    const openedAt = Date.now();
    if (retainedScene) {
      retainedScenes = retainedScenes.filter(scene => scene.id !== id);
    }
    next.push(buildSceneTab(id, openedAt));
    set({
      openTabs: ensureAgentFirst(next),
      retainedScenes,
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
    const { openTabs, retainedScenes, activeTabId, navHistory, navCursor } = state;
    if (!isClosableScene(id)) return;

    const nextTabs = openTabs.filter(t => t.id !== id);
    const nextRetainedScenes = retainedScenes.filter(scene => scene.id !== id);

    let newActiveId = activeTabId;
    if (id === activeTabId) {
      if (nextTabs.length === 0) {
        set({
          openTabs: [],
          retainedScenes: nextRetainedScenes,
          activeTabId: '' as SceneTabId,
          navHistory: [],
          navCursor: -1,
          navigationMotion: getInteractionMotion(),
          navigationSequence: state.navigationSequence + 1,
        });
        return;
      }
      newActiveId = [...nextTabs].sort((a, b) => b.lastUsed - a.lastUsed)[0].id;
    }

    const histUpdate = removeFromHistory(navHistory, navCursor, id, newActiveId);
    set({
      openTabs: ensureAgentFirst(nextTabs),
      retainedScenes: nextRetainedScenes,
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
    const activeTabId: SceneTabId = tabs[0]?.id ?? WELCOME_SCENE_ID;
    set({
      openTabs: tabs,
      retainedScenes: [],
      activeTabId,
      navHistory: [activeTabId],
      navCursor: 0,
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
