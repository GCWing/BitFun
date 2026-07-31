/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelSelector } from './ModelSelector';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

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
    }),
  },
}));

describe('ModelSelector external transport reuse', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
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
});
