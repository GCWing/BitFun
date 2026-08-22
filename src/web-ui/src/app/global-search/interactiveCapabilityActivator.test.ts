import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  openDestination: vi.fn(),
  openScene: vi.fn(),
  activateProductAction: vi.fn(),
}));

vi.mock('@/app/scenes/settings/settingsStore', () => ({
  useSettingsStore: {
    getState: () => ({ openDestination: mocks.openDestination }),
  },
}));

vi.mock('@/app/stores/sceneStore', () => ({
  useSceneStore: {
    getState: () => ({ openScene: mocks.openScene }),
  },
}));

vi.mock('./productActionActivator', () => ({
  activateProductAction: mocks.activateProductAction,
}));

import { activateInteractiveCapability } from './interactiveCapabilityActivator';

describe('activateInteractiveCapability', () => {
  beforeEach(() => vi.clearAllMocks());

  it('opens the exact settings subview declared by a documented item', async () => {
    await activateInteractiveCapability('setting.application.input', {
      itemId: 'shortcut-browser',
    });

    expect(mocks.openDestination).toHaveBeenCalledWith({
      kind: 'settings',
      pageId: 'application.input',
      viewId: 'shortcuts',
    });
    expect(mocks.openScene).toHaveBeenCalledWith('settings');
  });

  it('rejects stale item IDs instead of silently opening the wrong place', async () => {
    await expect(activateInteractiveCapability('setting.application.input', {
      itemId: 'missing-item',
    })).rejects.toThrow('Unknown documented item');
    expect(mocks.openDestination).not.toHaveBeenCalled();
  });
});
