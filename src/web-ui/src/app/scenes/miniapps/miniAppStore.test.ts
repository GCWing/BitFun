import { beforeEach, describe, expect, it } from 'vitest';
import { useMiniAppStore } from './miniAppStore';

describe('miniAppStore customization state', () => {
  beforeEach(() => {
    useMiniAppStore.setState({
      apps: [],
      loading: false,
      openedAppIds: [],
      runningWorkerIds: [],
      customizingAppIds: [],
      composerClaims: {},
    });
  });

  it('tracks apps with an active customization panel', () => {
    useMiniAppStore.getState().markCustomizationActive('gomoku');
    useMiniAppStore.getState().markCustomizationActive('gomoku');

    expect(useMiniAppStore.getState().customizingAppIds).toEqual(['gomoku']);

    useMiniAppStore.getState().markCustomizationIdle('gomoku');

    expect(useMiniAppStore.getState().customizingAppIds).toEqual([]);
  });

  it('removes stale customization ids when the app catalog changes', () => {
    useMiniAppStore.setState({
      customizingAppIds: ['gomoku', 'removed-app'],
      openedAppIds: ['gomoku', 'removed-app'],
      runningWorkerIds: ['gomoku', 'removed-app'],
    });

    useMiniAppStore.getState().setApps([
      {
        id: 'gomoku',
        name: 'Gomoku',
        description: '',
        category: 'game',
        version: 1,
        icon: 'box',
        tags: [],
        created_at: 1,
        updated_at: 1,
        permissions: {},
      },
    ]);

    expect(useMiniAppStore.getState().customizingAppIds).toEqual(['gomoku']);
    expect(useMiniAppStore.getState().openedAppIds).toEqual(['gomoku']);
    expect(useMiniAppStore.getState().runningWorkerIds).toEqual(['gomoku']);
  });
});

describe('miniAppStore floating bubble composer claims', () => {
  beforeEach(() => {
    useMiniAppStore.setState({ apps: [], composerClaims: {} });
  });

  it('lets the newest runner take the claim', () => {
    useMiniAppStore.getState().claimComposer('ppt', { token: 'ppt#1', placeholder: 'a' });
    useMiniAppStore.getState().claimComposer('ppt', { token: 'ppt#2', placeholder: 'b' });

    expect(useMiniAppStore.getState().composerClaims.ppt).toEqual({
      token: 'ppt#2',
      placeholder: 'b',
    });
  });

  // The installed app and its draft preview run side by side during AI
  // customization. When one unmounts it must not release the other's claim,
  // or the bubble would silently fall back to the host session composer.
  it('ignores a release from a runner that no longer holds the claim', () => {
    useMiniAppStore.getState().claimComposer('ppt', { token: 'ppt#1' });
    useMiniAppStore.getState().claimComposer('ppt', { token: 'ppt#2' });

    useMiniAppStore.getState().releaseComposer('ppt', 'ppt#1');
    expect(useMiniAppStore.getState().composerClaims.ppt).toEqual({ token: 'ppt#2' });

    useMiniAppStore.getState().releaseComposer('ppt', 'ppt#2');
    expect(useMiniAppStore.getState().composerClaims.ppt).toBeUndefined();
  });

  it('drops claims for apps that leave the catalog', () => {
    useMiniAppStore.getState().claimComposer('removed-app', { token: 'removed-app#1' });

    useMiniAppStore.getState().setApps([]);

    expect(useMiniAppStore.getState().composerClaims).toEqual({});
  });
});
