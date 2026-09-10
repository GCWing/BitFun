/**
 * sceneStore — SceneBar tab lifecycle + scene navigation history.
 *
 * Tab rules:
 *   - Every explicitly opened scene stays in openTabs until the user closes it.
 *     Session tabs reference one session per workspace; the shell-owned
 *     sessionSceneLifecycle retires their tabs when that resource disappears.
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
import {
  getSceneViewId,
  getSessionSceneTabId,
  isSessionSceneId,
  type SceneTab,
  type SceneTabId,
  type SessionSceneTarget,
} from '../components/SceneBar/types';
import {
  abandonSettingsDraftsForContextSwitch,
  requestAllSettingsDraftsExit,
} from '@/infrastructure/config/settingsDraftRegistry';

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

/** Shell adapter: the tab owner never loads history or changes runtime state. */
export interface SessionSceneNavigation {
  current: () => SessionSceneTarget | null;
  isActive: (target: SessionSceneTarget) => boolean;
  activate: (target: SessionSceneTarget, isCurrent: () => boolean) => Promise<boolean>;
}

let sessionNavigation: SessionSceneNavigation | undefined;
let navigationRequest = 0;

function resolveNavSceneId(sceneId: SceneTabId | null): SceneTabId | null {
  if (sceneId === null) return null;
  return getSceneNav(sceneId) ? sceneId : null;
}

interface SceneState {
  openTabs: SceneTab[];
  activeTabId: SceneTabId | null;
  pendingTabId: SceneTabId | null;
  /** Ordered history of activeTabId values. */
  navHistory: SceneTabId[];
  /** Index of the current position in navHistory. */
  navCursor: number;
  /** Input source for the latest active-scene change. */
  navigationMotion: InteractionMotion;
  navigationSequence: number;

  openScene:    (id: SceneTabId) => void;
  /** Called after authoritative session selection, replaces only its workspace slot. */
  openSessionScene: (target: SessionSceneTarget) => void;
  updateSessionScene: (target: SessionSceneTarget) => void;
  /** Resource reconciliation never deletes or stops the referenced sessions. */
  reconcileSessionScenes: (targets: ReadonlyMap<string, SessionSceneTarget>) => void;
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
  if (history[cursor] !== removedId) {
    return { navHistory: newHistory, navCursor: history.slice(0, cursor + 1).filter(id => id !== removedId).length - 1 };
  }
  const idx = newHistory.lastIndexOf(newActiveId);
  const newCursor = idx !== -1 ? idx : Math.min(cursor, newHistory.length - 1);
  return { navHistory: newHistory, navCursor: newCursor };
}

const initialTabs = buildDefaultTabs();
const initialActiveId = initialTabs[0]?.id ?? null;

export const useSceneStore = create<SceneState>((set, get) => ({
  openTabs:    initialTabs,
  activeTabId: initialActiveId,
  pendingTabId: null,
  navHistory:  initialActiveId ? [initialActiveId] : [],
  navCursor:   initialActiveId ? 0 : -1,
  navigationMotion: 'instant',
  navigationSequence: 0,

  openScene: (requestedId) => {
    const target = requestedId === 'session' ? sessionNavigation?.current() : undefined;
    if (requestedId === 'session' && !target) return;
    openSceneTarget(target ? getSessionSceneTabId(target) : requestedId, target ?? undefined);
  },

  openSessionScene: (target) => openSceneTarget(getSessionSceneTabId(target), target),

  updateSessionScene: (target) => {
    const id = getSessionSceneTabId(target);
    const state = get();
    if (!state.openTabs.some(tab => tab.id === id && tab.session?.sessionId !== target.sessionId)) return;
    set({ openTabs: state.openTabs.map(tab => tab.id === id ? { ...tab, session: target } : tab) });
  },

  reconcileSessionScenes: (targets) => {
    const state = get();
    const remapped = new Map<SceneTabId, SceneTabId>();
    const tabs = new Map<SceneTabId, SceneTab>();
    const activeSessionId = state.openTabs.find(tab => tab.id === state.activeTabId)?.session?.sessionId;
    for (const tab of state.openTabs) {
      if (!isSessionSceneId(tab.id)) {
        tabs.set(tab.id, tab);
        continue;
      }
      const target = tab.session && targets.get(tab.session.sessionId);
      if (!target) continue;
      const id = getSessionSceneTabId(target);
      remapped.set(tab.id, id);
      const existing = tabs.get(id);
      if (!existing || tab.id === state.activeTabId
        || (existing.session?.sessionId !== activeSessionId && tab.lastUsed > existing.lastUsed)) {
        tabs.set(id, id === tab.id && target.workspaceKey === tab.session?.workspaceKey
          ? tab : { ...tab, id, session: target });
      }
    }
    const nextTabs = [...tabs.values()];
    if (nextTabs.length === state.openTabs.length && nextTabs.every((tab, i) => tab === state.openTabs[i])) return;
    const activeId = state.activeTabId && (remapped.get(state.activeTabId) ?? state.activeTabId);
    const retainedActiveId = activeId && tabs.has(activeId) ? activeId : null;
    const mappedHistory = state.navHistory.map(id => remapped.get(id) ?? id);
    const navHistory = mappedHistory.filter(id => tabs.has(id));
    const retiredPendingTab = state.pendingTabId !== null
      && state.openTabs.some(tab => tab.id === state.pendingTabId)
      && !remapped.has(state.pendingTabId);
    if (retiredPendingTab) navigationRequest++;
    set({
      ...(retiredPendingTab ? { pendingTabId: null } : {}),
      openTabs: nextTabs,
      activeTabId: retainedActiveId,
      navHistory,
      navCursor: retainedActiveId
        ? mappedHistory.slice(0, state.navCursor + 1).filter(id => tabs.has(id)).length - 1 : -1,
    });
    if (!retainedActiveId && state.activeTabId) {
      const fallback = [...nextTabs].sort((a, b) => b.lastUsed - a.lastUsed)[0];
      if (fallback) get().activateScene(fallback.id);
    }
  },

  activateScene: (id) => {
    get().openScene(id);
  },

  closeScene: (id) => {
    const performClose = () => {
      const state = get();
      const { openTabs, activeTabId } = state;
      if (!openTabs.some(tab => tab.id === id) || !isClosableScene(id)) return;

      const nextTabs = openTabs.filter(t => t.id !== id);
      if (nextTabs.length === 0) {
        navigationRequest++;
        set({
          openTabs: [], activeTabId: null, pendingTabId: null,
          navHistory: [], navCursor: -1,
          navigationMotion: getInteractionMotion(),
          navigationSequence: state.navigationSequence + 1,
        });
        return;
      }

      const fallbackTabId = [...nextTabs].sort((a, b) => b.lastUsed - a.lastUsed)[0].id;
      const newActiveId = id === activeTabId ? fallbackTabId : activeTabId ?? fallbackTabId;
      navigateToScene(newActiveId, () => {
        const current = get();
        set({
          openTabs: orderPinnedTabsFirst(current.openTabs.filter(tab => tab.id !== id)),
          activeTabId: newActiveId,
          navigationMotion: getInteractionMotion(),
          navigationSequence: current.navigationSequence + 1,
          ...removeFromHistory(current.navHistory, current.navCursor, id, newActiveId),
        });
      });
    };

    if (id === 'settings') {
      const settingsWasActive = get().activeTabId === 'settings';
      const closedImmediately = requestAllSettingsDraftsExit(performClose);
      if (!closedImmediately && !settingsWasActive) get().openScene('settings');
      return;
    }
    performClose();
  },

  goBack: () => navigateHistory(-1),
  goForward: () => navigateHistory(1),

  resetForPeerSwitch: () => {
    navigationRequest++;
    abandonSettingsDraftsForContextSwitch();
    useNavSceneStore.getState().closeNavScene();
    const state = get();
    const tabs = buildDefaultTabs();
    const activeTabId = tabs[0]?.id ?? null;
    set({
      openTabs: tabs,
      activeTabId,
      pendingTabId: null,
      navHistory: activeTabId ? [activeTabId] : [],
      navCursor: activeTabId ? 0 : -1,
      navigationMotion: 'instant',
      navigationSequence: state.navigationSequence + 1,
    });
  },
}));

export function registerSessionSceneNavigation(adapter: SessionSceneNavigation): () => void {
  sessionNavigation = adapter;
  return () => {
    if (sessionNavigation === adapter) {
      sessionNavigation = undefined;
      navigationRequest++;
      if (useSceneStore.getState().pendingTabId !== null) useSceneStore.setState({ pendingTabId: null });
    }
  };
}

/** Every navigation path, including history and close fallback, uses this gate. */
function navigateToScene(id: SceneTabId | null, commit: () => void, session?: SessionSceneTarget): void {
  const hadPendingNavigation = useSceneStore.getState().pendingTabId !== null;
  const request = ++navigationRequest;
  const adapter = sessionNavigation;
  const target = session ?? useSceneStore.getState().openTabs.find(tab => tab.id === id)?.session;
  const isCurrent = () => request === navigationRequest && adapter === sessionNavigation;
  if (!target || !adapter || (!hadPendingNavigation && adapter.isActive(target))) {
    if (useSceneStore.getState().pendingTabId !== null) useSceneStore.setState({ pendingTabId: null });
    commit();
    return;
  }

  useSceneStore.setState({ pendingTabId: id });
  void adapter.activate(target, isCurrent).then(activated => {
    if (activated && isCurrent()) commit();
  }).finally(() => {
    if (isCurrent()) useSceneStore.setState({ pendingTabId: null });
  });
}

function openSceneTarget(id: SceneTabId, session?: SessionSceneTarget): void {
  const get = useSceneStore.getState;
  const set = useSceneStore.setState;
  const performOpen = () => {
    const state = get();
    const { activeTabId } = state;
    const navigationMotion = getInteractionMotion();

    // Already active — re-sync left nav in case user navigated back to MainNav
    if (id === activeTabId) {
      if (session && state.openTabs.find(tab => tab.id === id)?.session?.sessionId !== session.sessionId) {
        set({ openTabs: state.openTabs.map(tab => tab.id === id ? { ...tab, session, lastUsed: Date.now() } : tab) });
      }
      const navSceneId = resolveNavSceneId(id);
      const navStore = useNavSceneStore.getState();
      if (navSceneId && (!navStore.showSceneNav || navStore.navSceneId !== navSceneId)) {
        navStore.openNavScene(navSceneId);
      }
      return;
    }

    const isAlreadyOpen = state.openTabs.some(tab => tab.id === id);
    if (isSessionSceneId(id) && !isAlreadyOpen && !session) return;
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
          tab.id === id ? { ...tab, ...(session ? { session } : {}), lastUsed: activatedAt } : tab
        )),
        navigationMotion,
        navigationSequence: state.navigationSequence + 1,
        ...histUpdate,
      });
      return;
    }

    const next = [...openTabs, { ...buildSceneTab(id, Date.now()), ...(session ? { session } : {}) }];
    set({
      openTabs: orderPinnedTabsFirst(next),
      activeTabId: id,
      navigationMotion,
      navigationSequence: state.navigationSequence + 1,
      ...histUpdate,
    });
  };

  if (get().activeTabId === 'settings' && id !== 'settings') {
    requestAllSettingsDraftsExit(() => navigateToScene(id, performOpen, session));
    return;
  }
  navigateToScene(id, performOpen, session);
}

function navigateHistory(direction: -1 | 1): void {
  const perform = () => {
    const state = useSceneStore.getState();
    for (let i = state.navCursor + direction; i >= 0 && i < state.navHistory.length; i += direction) {
      const id = state.navHistory[i];
      if (!state.openTabs.some(tab => tab.id === id)) continue;
      navigateToScene(id, () => {
        const current = useSceneStore.getState();
        if (!current.openTabs.some(tab => tab.id === id)) return;
        useSceneStore.setState({
          activeTabId: id,
          navCursor: i,
          navigationMotion: getInteractionMotion(),
          navigationSequence: current.navigationSequence + 1,
          openTabs: current.openTabs.map(tab => tab.id === id ? { ...tab, lastUsed: Date.now() } : tab),
        });
      });
      return;
    }
  };
  if (useSceneStore.getState().activeTabId === 'settings') {
    requestAllSettingsDraftsExit(perform);
  } else {
    perform();
  }
}

/** The renderer/chrome use the scene kind; tabs/history use resource identity. */
export function selectActiveSceneId(state: SceneState): SceneTabId | null {
  return state.activeTabId ? getSceneViewId(state.activeTabId) : null;
}
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
      } else if (!(navStore.navSceneId === 'file-viewer'
        && (isSessionSceneId(state.activeTabId) || state.activeTabId === 'terminal'
          || state.activeTabId === 'shell' || state.activeTabId === 'git'
          || state.activeTabId === null))) {
        navStore.closeNavScene();
      }
    }
  });
}
