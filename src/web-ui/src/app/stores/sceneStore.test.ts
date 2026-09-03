import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { recordInteractionModality } from '@/shared/utils/motionPreference';
import { useSceneStore } from './sceneStore';
import {
  discardAndContinueSettingsNavigation,
  getSettingsDraftSnapshot,
  registerSettingsDraft,
  resetSettingsDraftRegistryForTests,
} from '@/infrastructure/config/settingsDraftRegistry';

describe('sceneStore transition snapshots', () => {
  beforeEach(() => {
    recordInteractionModality('programmatic');
    resetSettingsDraftRegistryForTests();
    useSceneStore.getState().resetForPeerSwitch();
  });

  afterEach(() => {
    resetSettingsDraftRegistryForTests();
    vi.restoreAllMocks();
  });

  it('starts on the welcome surface without creating a tab', () => {
    const state = useSceneStore.getState();

    expect(state.openTabs).toEqual([]);
    expect(state.activeTabId).toBeNull();
    expect(state.navHistory).toEqual([]);
    expect(state.navCursor).toBe(-1);
  });

  it('publishes the first scene switch atomically from the tabless welcome surface', () => {
    const snapshots: Array<{ activeTabId: string | null; openTabIds: string[] }> = [];
    const unsubscribe = useSceneStore.subscribe(state => {
      snapshots.push({
        activeTabId: state.activeTabId,
        openTabIds: state.openTabs.map(tab => tab.id),
      });
    });

    useSceneStore.getState().openScene('settings');
    unsubscribe();

    expect(snapshots).toHaveLength(1);
    expect(snapshots[0].activeTabId).toBe('settings');
    expect(snapshots[0].openTabIds).toEqual(['settings']);
  });

  it('records pointer scene navigation without animating keyboard activation', () => {
    recordInteractionModality('pointer');
    useSceneStore.getState().openScene('settings');
    expect(useSceneStore.getState().navigationMotion).toBe('pointer');

    recordInteractionModality('keyboard');
    useSceneStore.getState().openScene('session');
    expect(useSceneStore.getState().navigationMotion).toBe('instant');
  });

  it('keeps every explicitly opened scene instead of evicting older tabs', () => {
    useSceneStore.getState().openScene('settings');
    useSceneStore.getState().openScene('terminal');
    useSceneStore.getState().openScene('git');
    useSceneStore.getState().openScene('miniapps');
    useSceneStore.getState().openScene('miniapp:first');
    useSceneStore.getState().openScene('miniapp:second');

    expect(useSceneStore.getState().openTabs.map(tab => tab.id)).toEqual([
      'settings',
      'terminal',
      'git',
      'miniapps',
      'miniapp:first',
      'miniapp:second',
    ]);
    expect(useSceneStore.getState().activeTabId).toBe('miniapp:second');
  });

  it('activates an existing tab without changing its order or duplicating it', () => {
    useSceneStore.getState().openScene('settings');
    useSceneStore.getState().openScene('terminal');
    useSceneStore.getState().openScene('settings');

    const state = useSceneStore.getState();
    expect(state.openTabs.map(tab => tab.id)).toEqual([
      'settings',
      'terminal',
    ]);
    expect(state.openTabs.filter(tab => tab.id === 'settings')).toHaveLength(1);
    expect(state.activeTabId).toBe('settings');
  });

  it('closes the last session tab into the tabless welcome state', () => {
    useSceneStore.getState().openScene('session');
    useSceneStore.getState().closeScene('session');

    const state = useSceneStore.getState();
    expect(state.openTabs).toEqual([]);
    expect(state.activeTabId).toBeNull();
    expect(state.navHistory).toEqual([]);
    expect(state.navCursor).toBe(-1);
  });

  it('keeps the session tab first while allowing it to close back to another scene', () => {
    useSceneStore.getState().openScene('settings');
    useSceneStore.getState().openScene('session');

    expect(useSceneStore.getState().openTabs.map(tab => tab.id)).toEqual([
      'session',
      'settings',
    ]);

    useSceneStore.getState().closeScene('session');

    const state = useSceneStore.getState();
    expect(state.openTabs.map(tab => tab.id)).toEqual(['settings']);
    expect(state.activeTabId).toBe('settings');
    expect(state.navHistory).not.toContain('session');
  });

  it('preserves close fallback and history navigation across many open tabs', () => {
    let now = 1;
    vi.spyOn(Date, 'now').mockImplementation(() => now++);

    useSceneStore.getState().openScene('settings');
    useSceneStore.getState().openScene('terminal');
    useSceneStore.getState().openScene('git');
    useSceneStore.getState().openScene('settings');
    useSceneStore.getState().closeScene('settings');

    const state = useSceneStore.getState();
    expect(state.openTabs.map(tab => tab.id)).toEqual(['terminal', 'git']);
    expect(state.activeTabId).toBe('git');
    expect(state.navHistory).not.toContain('settings');

    useSceneStore.getState().goBack();
    expect(useSceneStore.getState().activeTabId).toBe('terminal');
  });

  it('resets an expanded tab set to the tabless welcome surface when the peer host changes', () => {
    useSceneStore.getState().openScene('settings');
    useSceneStore.getState().openScene('terminal');
    useSceneStore.getState().openScene('git');
    expect(useSceneStore.getState().openTabs).toHaveLength(3);

    useSceneStore.getState().resetForPeerSwitch();

    const state = useSceneStore.getState();
    expect(state.openTabs).toEqual([]);
    expect(state.activeTabId).toBeNull();
    expect(state.navHistory).toEqual([]);
    expect(state.navCursor).toBe(-1);
  });

  it('keeps Settings active until its draft is resolved before changing scenes', async () => {
    useSceneStore.getState().openScene('session');
    useSceneStore.getState().openScene('settings');
    registerSettingsDraft({
      id: 'settings-form',
      pageId: 'application.voice',
      label: 'Voice',
      dirty: true,
      save: vi.fn(),
      discard: vi.fn(),
    });

    useSceneStore.getState().openScene('session');
    expect(useSceneStore.getState().activeTabId).toBe('settings');
    expect(getSettingsDraftSnapshot().pendingNavigation).not.toBeNull();

    await discardAndContinueSettingsNavigation();
    expect(useSceneStore.getState().activeTabId).toBe('session');
  });

  it('abandons device-owned drafts during the non-interactive peer reset', () => {
    const save = vi.fn();
    const discard = vi.fn();
    registerSettingsDraft({
      id: 'old-device-form',
      pageId: 'application.voice',
      label: 'Voice',
      dirty: true,
      save,
      discard,
    });

    useSceneStore.getState().resetForPeerSwitch();

    expect(getSettingsDraftSnapshot().resources).toEqual([]);
    expect(save).not.toHaveBeenCalled();
    expect(discard).not.toHaveBeenCalled();
  });

  it('reveals a background Settings tab before asking whether to close its draft', async () => {
    useSceneStore.getState().openScene('settings');
    useSceneStore.getState().openScene('session');
    registerSettingsDraft({
      id: 'background-form',
      pageId: 'application.voice',
      label: 'Voice',
      dirty: true,
      save: vi.fn(),
      discard: vi.fn(),
    });

    useSceneStore.getState().closeScene('settings');
    expect(useSceneStore.getState().activeTabId).toBe('settings');
    expect(getSettingsDraftSnapshot().pendingNavigation).not.toBeNull();

    await discardAndContinueSettingsNavigation();
    expect(useSceneStore.getState().openTabs.some(tab => tab.id === 'settings')).toBe(false);
    expect(useSceneStore.getState().activeTabId).toBe('session');
  });
});
