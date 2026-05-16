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
