/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelSelector } from './ModelSelector';
import { configManager } from '@/infrastructure/config/services/ConfigManager';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const aiApiMocks = vi.hoisted(() => ({
  getModelCatalog: vi.fn(),
  onModelCatalogUpdated: vi.fn(),
}));

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

vi.mock('@/component-library', () => ({
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

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: vi.fn(),
    on: vi.fn(),
    off: vi.fn(),
  },
}));

vi.mock('../store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => ({
      getState: () => ({ sessions: new Map() }),
      subscribe: () => () => undefined,
    }),
  },
}));

describe('ModelSelector external transport reuse', () => {
  let container: HTMLDivElement;
  let root: Root;
  let catalogUpdated: (() => void) | undefined;

  beforeEach(() => {
    catalogUpdated = undefined;
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
    expect(container.querySelector('[data-testid="chat-reasoning-preset-selector-btn"]')).toBeNull();

    await act(async () => {
      catalogUpdated?.();
      await Promise.resolve();
    });

    expect(aiApiMocks.getModelCatalog).toHaveBeenCalledTimes(2);
    expect(container.querySelector('[data-testid="chat-reasoning-preset-selector-btn"]')).not.toBeNull();
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

    const auto = document.body.querySelector<HTMLButtonElement>(
      '[data-testid="chat-model-selector-option"][data-model-id="auto"]',
    );
    await act(async () => {
      auto?.click();
      await Promise.resolve();
    });
    expect(configManager.setConfig).not.toHaveBeenCalled();
  });
});
