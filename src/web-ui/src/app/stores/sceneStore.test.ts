import { beforeEach, describe, expect, it } from 'vitest';
import { useSceneStore } from './sceneStore';

describe('sceneStore transition snapshots', () => {
  beforeEach(() => {
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
});
