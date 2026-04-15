import { beforeEach, describe, expect, it, vi } from 'vitest';

const {
  openSceneMock,
  setActiveTabMock,
  getConfigMock,
  setConfigMock,
  resetConfigState,
} = vi.hoisted(() => {
  const openSceneMock = vi.fn();
  const setActiveTabMock = vi.fn();
  let configState: Record<string, unknown> = {};

  const getConfigMock = vi.fn(async (key: string) => configState[key]);
  const setConfigMock = vi.fn(async (key: string, value: unknown) => {
    configState[key] = value;
  });

  return {
    openSceneMock,
    setActiveTabMock,
    getConfigMock,
    setConfigMock,
    resetConfigState(nextState: Record<string, unknown>) {
      configState = structuredClone(nextState);
    },
  };
});

vi.mock('@/app/stores/sceneStore', () => ({
  useSceneStore: {
    getState: () => ({
      activeTabId: 'session',
      openScene: openSceneMock,
    }),
  },
}));

vi.mock('@/app/scenes/settings/settingsStore', () => ({
  useSettingsStore: {
    getState: () => ({
      activeTab: 'models',
      setActiveTab: setActiveTabMock,
    }),
  },
}));

vi.mock('@/infrastructure/config', () => ({
  configManager: {
    getConfig: getConfigMock,
    setConfig: setConfigMock,
  },
}));

vi.mock('@/infrastructure/config/services/modelConfigs', () => ({
  getModelDisplayName: ({ name, model_name }: { name?: string; model_name?: string }) =>
    model_name || name || 'Unknown',
}));

vi.mock('@/infrastructure/config/services/providerCatalog', () => ({
  matchProviderCatalogItemByBaseUrl: () => null,
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

import { SelfControlService } from './SelfControlService';

describe('SelfControlService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetConfigState({});
  });

  it('accepts raw Rust payloads that use action as the discriminator', async () => {
    const service = new SelfControlService();

    await expect(service.executeAction({ action: 'open_scene', sceneId: 'settings' })).resolves.toBe(
      'Opened scene: settings',
    );

    expect(openSceneMock).toHaveBeenCalledWith('settings');
  });

  it('repairs default model references after deleting the current default model', async () => {
    resetConfigState({
      'ai.models': [
        {
          id: 'model-primary',
          name: 'Target',
          model_name: 'target-v1',
          provider: 'provider-a',
          enabled: true,
        },
        {
          id: 'model-fallback',
          name: 'Fallback',
          model_name: 'fallback-v1',
          provider: 'provider-b',
          enabled: true,
        },
      ],
      'ai.default_models': {
        primary: 'model-primary',
        fast: 'model-primary',
      },
    });

    const service = new SelfControlService();

    await expect(service.executeAction({ action: 'delete_model', modelQuery: 'Target' })).resolves.toContain(
      'Default model updates: primary fallback -> model-fallback; fast fallback -> model-fallback.',
    );

    expect(setConfigMock).toHaveBeenCalledWith('ai.models', [
      {
        id: 'model-fallback',
        name: 'Fallback',
        model_name: 'fallback-v1',
        provider: 'provider-b',
        enabled: true,
      },
    ]);
    expect(setConfigMock).toHaveBeenCalledWith('ai.default_models', {
      primary: 'model-fallback',
      fast: 'model-fallback',
    });
  });
});
