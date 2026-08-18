import { beforeEach, describe, expect, it } from 'vitest';
import { recordInteractionModality } from '@/shared/utils/motionPreference';
import { useSceneStore } from './sceneStore';

describe('sceneStore transition snapshots', () => {
  beforeEach(() => {
    recordInteractionModality('programmatic');
    useSceneStore.getState().resetForPeerSwitch();
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

  it('retains an auto-evicted MiniApp Runner while keeping the visible tab cap', () => {
    useSceneStore.getState().openScene('miniapps');
    useSceneStore.getState().openScene('miniapp:first');
    useSceneStore.getState().openScene('miniapp:second');

    expect(useSceneStore.getState().openTabs.map(tab => tab.id)).toEqual([
      'session',
      'miniapp:first',
      'miniapp:second',
    ]);
    expect(useSceneStore.getState().retainedScenes).toEqual([]);

    useSceneStore.getState().openScene('miniapps');

    expect(useSceneStore.getState().openTabs.map(tab => tab.id)).toEqual([
      'session',
      'miniapp:second',
      'miniapps',
    ]);
    expect(useSceneStore.getState().retainedScenes.map(scene => scene.id)).toEqual([
      'miniapp:first',
    ]);
  });

  it('restores a retained MiniApp without remounting it twice', () => {
    useSceneStore.getState().openScene('miniapps');
    useSceneStore.getState().openScene('miniapp:first');
    useSceneStore.getState().openScene('miniapp:second');
    useSceneStore.getState().openScene('miniapps');

    useSceneStore.getState().openScene('miniapp:first');

    const state = useSceneStore.getState();
    expect(state.openTabs.map(tab => tab.id)).toEqual([
      'session',
      'miniapps',
      'miniapp:first',
    ]);
    expect(state.retainedScenes.map(scene => scene.id)).toEqual(['miniapp:second']);
    expect([
      ...state.openTabs,
      ...state.retainedScenes,
    ].filter(scene => scene.id === 'miniapp:first')).toHaveLength(1);
  });

  it('explicitly closes a retained MiniApp Runner', () => {
    useSceneStore.getState().openScene('miniapps');
    useSceneStore.getState().openScene('miniapp:first');
    useSceneStore.getState().openScene('miniapp:second');
    useSceneStore.getState().openScene('miniapps');

    useSceneStore.getState().closeScene('miniapp:first');

    expect(useSceneStore.getState().retainedScenes).toEqual([]);
  });

  it('clears retained MiniApp Runners when switching the peer host', () => {
    useSceneStore.getState().openScene('miniapps');
    useSceneStore.getState().openScene('miniapp:first');
    useSceneStore.getState().openScene('miniapp:second');
    useSceneStore.getState().openScene('miniapps');
    expect(useSceneStore.getState().retainedScenes).toHaveLength(1);

    useSceneStore.getState().resetForPeerSwitch();

    expect(useSceneStore.getState().retainedScenes).toEqual([]);
  });
});
