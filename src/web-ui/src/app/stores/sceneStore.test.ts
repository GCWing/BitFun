import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { recordInteractionModality } from '@/shared/utils/motionPreference';
import { useSceneStore } from './sceneStore';

describe('sceneStore transition snapshots', () => {
  beforeEach(() => {
    recordInteractionModality('programmatic');
    useSceneStore.getState().resetForPeerSwitch();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('publishes the first scene switch atomically without a blank active scene', () => {
    const snapshots: Array<{ activeTabId: string; openTabIds: string[] }> = [];
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
    expect(snapshots[0].openTabIds).toContain('settings');
    expect(snapshots[0].openTabIds).not.toContain('welcome');
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
      'session',
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
      'session',
      'settings',
      'terminal',
    ]);
    expect(state.openTabs.filter(tab => tab.id === 'settings')).toHaveLength(1);
    expect(state.activeTabId).toBe('settings');
  });

  it('keeps the pinned session tab when close is requested', () => {
    useSceneStore.getState().openScene('settings');
    useSceneStore.getState().closeScene('session');

    expect(useSceneStore.getState().openTabs.map(tab => tab.id)).toEqual([
      'session',
      'settings',
    ]);
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
    expect(state.openTabs.map(tab => tab.id)).toEqual(['session', 'terminal', 'git']);
    expect(state.activeTabId).toBe('git');
    expect(state.navHistory).not.toContain('settings');

    useSceneStore.getState().goBack();
    expect(useSceneStore.getState().activeTabId).toBe('terminal');
  });

  it('resets an expanded tab set when the peer host changes', () => {
    useSceneStore.getState().openScene('settings');
    useSceneStore.getState().openScene('terminal');
    useSceneStore.getState().openScene('git');
    expect(useSceneStore.getState().openTabs).toHaveLength(4);

    useSceneStore.getState().resetForPeerSwitch();

    const state = useSceneStore.getState();
    expect(state.openTabs.map(tab => tab.id)).toEqual(['welcome']);
    expect(state.activeTabId).toBe('welcome');
    expect(state.navHistory).toEqual(['welcome']);
    expect(state.navCursor).toBe(0);
  });
});
