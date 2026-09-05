/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelSelector } from './ModelSelector';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { setRecentReasoningPreset } from '../utils/reasoningPresets';
import {
  shouldIncludeInternalModelSession,
  shouldSyncSessionModelSelection,
} from '../utils/modelSelectionTarget';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const aiApiMocks = vi.hoisted(() => ({
  getModelCatalog: vi.fn(),
  onModelCatalogUpdated: vi.fn(),
}));

const flowChatStoreMocks = vi.hoisted(() => {
  type TestSession = {
    sessionKind?: string;
    isTransient?: boolean;
    agentBackedTransient?: boolean;
    workspacePath?: string;
    projectWorkspacePath?: string;
    remoteConnectionId?: string;
    remoteSshHost?: string;
    maxContextTokens?: number;
    config: {
      agentType?: string;
      modelName?: string;
      reasoningPreset?: string;
      workspacePath?: string;
      projectWorkspacePath?: string;
    };
  };
  const sessions = new Map<string, TestSession>();
  const subscribers = new Set<() => void>();
  const emit = () => subscribers.forEach(callback => callback());
  const store = {
    getState: () => ({ sessions }),
    subscribe: vi.fn((callback: () => void) => {
      subscribers.add(callback);
      return () => subscribers.delete(callback);
    }),
    updateSessionModelName: vi.fn((sessionId: string, modelName: string) => {
      const session = sessions.get(sessionId);
      if (session) session.config.modelName = modelName;
      emit();
    }),
    updateSessionReasoningPreset: vi.fn((sessionId: string, reasoningPreset?: string) => {
      const session = sessions.get(sessionId);
      if (session) session.config.reasoningPreset = reasoningPreset;
      emit();
    }),
    updateSessionMaxContextTokens: vi.fn((sessionId: string, maxContextTokens: number) => {
      const session = sessions.get(sessionId);
      if (session) session.maxContextTokens = maxContextTokens;
    }),
    updateAcpContextUsage: vi.fn(),
  };
  return { sessions, subscribers, store };
});

vi.mock('@/infrastructure/api/service-api/AIApi', () => ({
  aiApi: aiApiMocks,
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@openbitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@openbitfun/ui')>(),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  Switch: () => null,
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: {
    getConfigs: vi.fn(async () => ({
      'ai.models': [
        {
          id: 'model-a',
          name: 'Synced provider',
          model_name: 'friendly-model-a',
          provider: 'openai',
          base_url: 'https://example.test/v1',
          enabled: true,
          category: 'text',
          capabilities: ['text_chat'],
        },
      ],
      'ai.agent_model_defaults': { mode: 'model-a' },
    })),
    onConfigChange: vi.fn(() => () => undefined),
    setConfig: vi.fn(async () => undefined),
  },
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: { updateSessionModel: vi.fn(async () => undefined) },
}));

vi.mock('@/infrastructure/api/service-api/ACPClientAPI', () => ({
  ACPClientAPI: {
    getSessionOptions: vi.fn(),
    onSessionOptionsChanged: vi.fn(() => () => undefined),
  },
}));

vi.mock('../services/flow-chat-manager/SessionModule', () => ({
  getModelMaxTokens: vi.fn(async () => 128_000),
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: vi.fn(),
    on: vi.fn(),
    off: vi.fn(),
  },
}));

vi.mock('../store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => flowChatStoreMocks.store,
  },
}));

describe('ModelSelector external transport reuse', () => {
  let container: HTMLDivElement;
  let root: Root;
  let catalogUpdated: (() => void) | undefined;

  beforeEach(() => {
    catalogUpdated = undefined;
    flowChatStoreMocks.sessions.clear();
    flowChatStoreMocks.subscribers.clear();
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, String(value)),
      removeItem: (key: string) => storage.delete(key),
      clear: () => storage.clear(),
      key: (index: number) => [...storage.keys()][index] ?? null,
      get length() { return storage.size; },
    });
    window.localStorage.clear();
    aiApiMocks.getModelCatalog.mockResolvedValue({
      version: 1,
      default_models: { primary: 'model-a' },
      models: [],
    });
    aiApiMocks.onModelCatalogUpdated.mockImplementation((callback: () => void) => {
      catalogUpdated = callback;
      return () => {
        if (catalogUpdated === callback) catalogUpdated = undefined;
      };
    });
    class TestResizeObserver {
      observe() {}
      disconnect() {}
    }
    vi.stubGlobal('ResizeObserver', TestResizeObserver);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('syncs agent-backed transient selections to their hidden runtime session', () => {
    const miniAppSession = {
      sessionKind: 'miniapp',
      isTransient: true,
      agentBackedTransient: true,
    };

    expect(shouldSyncSessionModelSelection(miniAppSession)).toBe(true);
    expect(shouldIncludeInternalModelSession(miniAppSession)).toBe(true);
    expect(shouldSyncSessionModelSelection({ isTransient: true })).toBe(false);
  });

  it('updates an agent-backed transient session when its model and reasoning change', async () => {
    flowChatStoreMocks.sessions.set('miniapp-session', {
      sessionKind: 'miniapp',
      isTransient: true,
      agentBackedTransient: true,
      workspacePath: '/tmp/miniapp-runtime',
      projectWorkspacePath: '/tmp/project',
      config: {
        agentType: 'agentic',
        modelName: 'model-a',
        reasoningPreset: 'medium',
      },
    });
    vi.mocked(configManager.getConfigs).mockResolvedValueOnce({
      'ai.models': [
        {
          id: 'model-a',
          name: 'Shared provider',
          model_name: 'model-a-native',
          provider: 'openai',
          base_url: 'https://example.test/v1',
          enabled: true,
          category: 'text',
          capabilities: ['text_chat'],
          metadata: { provider_instance_id: 'provider-shared' },
        },
        {
          id: 'model-b',
          name: 'Shared provider',
          model_name: 'model-b-native',
          provider: 'openai',
          base_url: 'https://example.test/v1',
          enabled: true,
          category: 'text',
          capabilities: ['text_chat'],
          metadata: { provider_instance_id: 'provider-shared' },
        },
      ],
      'ai.default_models': { primary: 'model-a' },
      'ai.agent_model_defaults': { mode: 'model-a' },
    });
    aiApiMocks.getModelCatalog.mockResolvedValueOnce({
      version: 1,
      default_models: { primary: 'model-a' },
      models: ['model-a', 'model-b'].map(id => ({
        id,
        name: id,
        provider: 'openai',
        base_url: 'https://example.test/v1',
        model_name: `${id}-native`,
        enabled: true,
        capabilities: ['text_chat'],
        reasoning: {
          status: 'known',
          default_preset: 'medium',
          presets: [
            {
              id: 'medium',
              label: 'Medium',
              order: 10,
              source: 'models_dev',
              actions: [{ type: 'effort', value: 'medium' }],
            },
            {
              id: 'high',
              label: 'High',
              order: 20,
              source: 'models_dev',
              actions: [{ type: 'effort', value: 'high' }],
            },
          ],
        },
      })),
    });
    setRecentReasoningPreset('model-b', 'medium');

    await act(async () => {
      root.render(
        <ModelSelector
          currentMode="agentic"
          sessionId="miniapp-session"
          persistSharedModeDefault={false}
          reasoningTriggerPresentation="label"
        />,
      );
      await Promise.resolve();
    });
    expect(
      container.querySelector('[data-testid="chat-model-selector-trigger-reasoning"]'),
    ).not.toBeNull();
    expect(
      container.querySelector('[data-testid="chat-reasoning-preset-selector-btn"]'),
    ).toBeNull();
    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="chat-model-selector-btn"]')?.click();
    });
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>(
        '[data-testid="chat-model-selector-settings-model"]',
      )?.click();
    });
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>(
        '[data-testid="chat-model-selector-provider"][data-provider-key="provider-shared"]',
      )?.click();
    });
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>(
        '[data-testid="chat-model-selector-option"][data-model-id="model-b"]',
      )?.click();
      await Promise.resolve();
    });

    expect(agentAPI.updateSessionModel).toHaveBeenLastCalledWith(expect.objectContaining({
      sessionId: 'miniapp-session',
      modelName: 'model-b',
      reasoningPreset: 'medium',
      workspacePath: '/tmp/project',
      includeInternal: true,
    }));

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="chat-model-selector-btn"]')?.click();
    });
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>(
        '[data-testid="chat-model-selector-settings-reasoning"]',
      )?.click();
    });
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>('[data-preset-id="high"]')?.click();
      await Promise.resolve();
    });

    expect(agentAPI.updateSessionModel).toHaveBeenLastCalledWith(expect.objectContaining({
      sessionId: 'miniapp-session',
      modelName: 'model-b',
      reasoningPreset: 'high',
      workspacePath: '/tmp/project',
      includeInternal: true,
    }));
  });

  it('reloads the local catalog when the backend reports a snapshot update', async () => {
    const updatedCatalog = {
      version: 2,
      default_models: { primary: 'model-a' },
      models: [{
        id: 'model-a',
        name: 'Synced provider',
        provider: 'openai',
        base_url: 'https://example.test/v1',
        model_name: 'friendly-model-a',
        enabled: true,
        capabilities: ['text_chat'],
        reasoning: {
          status: 'known',
          default_preset: 'high',
          presets: [{
            id: 'high',
            label: 'High',
            order: 10,
            source: 'models_dev',
            actions: [{ type: 'effort', value: 'high' }],
          }],
        },
      }],
    };
    aiApiMocks.getModelCatalog
      .mockResolvedValueOnce({ version: 1, default_models: { primary: 'model-a' }, models: [] })
      .mockResolvedValueOnce(updatedCatalog);

    await act(async () => {
      root.render(<ModelSelector currentMode="agentic" sessionId="session-a" />);
      await Promise.resolve();
    });
    expect(catalogUpdated).toBeTypeOf('function');
    expect(container.querySelector(
      '[data-testid="chat-model-selector-trigger-reasoning"]',
    )).toBeNull();
    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="chat-model-selector-btn"]')?.click();
    });
    expect(document.body.querySelector(
      '[data-testid="chat-model-selector-settings-reasoning"]',
    )).toBeNull();

    await act(async () => {
      catalogUpdated?.();
      await Promise.resolve();
    });

    expect(aiApiMocks.getModelCatalog).toHaveBeenCalledTimes(2);
    expect(container.querySelector(
      '[data-testid="chat-model-selector-trigger-reasoning"]',
    )).not.toBeNull();
    expect(document.body.querySelector(
      '[data-testid="chat-model-selector-settings-reasoning"]',
    )).not.toBeNull();
  });

  it('renders the target catalog through the shared selector and applies a choice', async () => {
    const onSelect = vi.fn(async () => undefined);
    await act(async () => {
      root.render(
        <ModelSelector
          currentMode="agentic"
          externalSelection={{
            models: ['model-a', 'model-b'],
            defaultModelId: 'model-a',
            providerLabel: 'parallels-ubuntu',
            onSelect,
          }}
        />,
      );
      await Promise.resolve();
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-model-selector-btn"]',
    );
    expect(trigger?.textContent).toContain('friendly-model-a');
    await act(async () => {
      trigger?.click();
    });

    const modelB = document.body.querySelector<HTMLButtonElement>(
      '[data-testid="chat-model-selector-option"][data-model-id="model-b"]',
    );
    await act(async () => {
      modelB?.click();
      await Promise.resolve();
    });
    expect(onSelect).toHaveBeenCalledWith('model-b');
  });

  it('offers this device\'s models when the transport only relays the session', async () => {
    // A projection restored without a probe snapshot used to render nothing at
    // all, leaving the user with no way to switch models in that session.
    const onSelect = vi.fn(async () => undefined);
    await act(async () => {
      root.render(
        <ModelSelector
          currentMode="agentic"
          externalSelection={{
            models: [],
            includeLocalCatalog: true,
            providerLabel: 'parallels-ubuntu',
            onSelect,
          }}
        />,
      );
      await Promise.resolve();
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-model-selector-btn"]',
    );
    expect(trigger, 'the picker must not disappear').not.toBeNull();
    // Falls back to this device's own default rather than list order.
    expect(trigger?.textContent).toContain('friendly-model-a');

    await act(async () => {
      trigger?.click();
    });
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>(
        '[data-testid="chat-model-selector-option"][data-model-id="model-a"]',
      )?.click();
      await Promise.resolve();
    });
    expect(onSelect).toHaveBeenCalledWith('model-a');
  });

  it('keeps the local list out of a transport that owns a foreign catalog', async () => {
    await act(async () => {
      root.render(
        <ModelSelector
          currentMode="agentic"
          externalSelection={{
            models: ['remote-only'],
            selectedModelId: 'remote-only',
            providerLabel: 'parallels-ubuntu',
            onSelect: vi.fn(),
          }}
        />,
      );
      await Promise.resolve();
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>(
        '[data-testid="chat-model-selector-btn"]',
      )?.click();
    });
    expect(document.body.querySelector(
      '[data-testid="chat-model-selector-option"][data-model-id="model-a"]',
    )).toBeNull();
  });

  it('reads reasoning presets from this device when the target reported none', async () => {
    aiApiMocks.getModelCatalog.mockResolvedValueOnce({
      version: 1,
      default_models: { primary: 'model-a' },
      models: [{
        id: 'model-a',
        name: 'Synced provider',
        provider: 'openai',
        base_url: 'https://example.test/v1',
        model_name: 'friendly-model-a',
        enabled: true,
        capabilities: ['text_chat'],
        reasoning: {
          status: 'known',
          default_preset: 'high',
          presets: [{
            id: 'high',
            label: 'High',
            order: 10,
            source: 'models_dev',
            actions: [{ type: 'effort', value: 'high' }],
          }],
        },
      }],
    });
    const onSelectReasoningPreset = vi.fn(async () => undefined);

    await act(async () => {
      root.render(
        <ModelSelector
          currentMode="agentic"
          externalSelection={{
            models: [],
            includeLocalCatalog: true,
            selectedModelId: 'model-a',
            providerLabel: 'parallels-ubuntu',
            onSelect: vi.fn(),
            onSelectReasoningPreset,
          }}
        />,
      );
      await Promise.resolve();
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-reasoning-preset-selector-btn"]',
    );
    expect(trigger).not.toBeNull();
    await act(async () => {
      trigger?.click();
    });
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>('[data-preset-id="high"]')?.click();
      await Promise.resolve();
    });
    expect(onSelectReasoningPreset).toHaveBeenCalledWith('high');
  });

  it('hides reasoning when the target did not report a catalog', async () => {
    await act(async () => {
      root.render(
        <ModelSelector
          currentMode="agentic"
          externalSelection={{
            models: ['model-a'],
            selectedModelId: 'model-a',
            providerLabel: 'parallels-ubuntu',
            onSelect: vi.fn(),
            onSelectReasoningPreset: vi.fn(),
          }}
        />,
      );
      await Promise.resolve();
    });

    expect(
      container.querySelector('[data-testid="chat-reasoning-preset-selector-btn"]'),
    ).toBeNull();
  });

  it('renders target-owned reasoning presets and applies a choice', async () => {
    const onSelectReasoningPreset = vi.fn(async () => undefined);
    await act(async () => {
      root.render(
        <ModelSelector
          currentMode="agentic"
          externalSelection={{
            models: ['model-a'],
            selectedModelId: 'model-a',
            providerLabel: 'parallels-ubuntu',
            reasoningCatalog: {
              version: 1,
              default_models: {},
              models: [{
                id: 'model-a',
                name: 'Target model',
                provider: 'openai',
                base_url: 'https://target.example.test/v1',
                model_name: 'model-a',
                enabled: true,
                capabilities: ['text_chat'],
                reasoning: {
                  status: 'known',
                  default_preset: 'medium',
                  presets: [
                    {
                      id: 'medium',
                      label: 'Medium',
                      order: 10,
                      source: 'models_dev',
                      actions: [{ type: 'effort', value: 'medium' }],
                    },
                    {
                      id: 'high',
                      label: 'High',
                      order: 20,
                      source: 'models_dev',
                      actions: [{ type: 'effort', value: 'high' }],
                    },
                  ],
                },
              }],
            },
            onSelect: vi.fn(),
            onSelectReasoningPreset,
          }}
        />,
      );
      await Promise.resolve();
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-reasoning-preset-selector-btn"]',
    );
    expect(trigger).not.toBeNull();
    await act(async () => {
      trigger?.click();
    });
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>('[data-preset-id="high"]')?.click();
      await Promise.resolve();
    });

    expect(onSelectReasoningPreset).toHaveBeenCalledWith('high');
  });

  it('uses an external agent profile model without changing the shared mode default', async () => {
    await act(async () => {
      root.render(
        <ModelSelector
          currentMode="reviewer"
          modeDefaultModelId="model-a"
          persistSharedModeDefault={false}
        />,
      );
      await Promise.resolve();
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-model-selector-btn"]',
    );
    expect(trigger?.textContent).toContain('friendly-model-a');
    await act(async () => {
      trigger?.click();
    });

    const primary = document.body.querySelector<HTMLButtonElement>(
      '[data-testid="chat-model-selector-option"][data-model-id="primary"]',
    );
    await act(async () => {
      primary?.click();
      await Promise.resolve();
    });
    expect(configManager.setConfig).not.toHaveBeenCalled();
  });
});
