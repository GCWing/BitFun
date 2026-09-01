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

const flowChatStoreMocks = vi.hoisted(() => {
  type TestSession = {
    config: { agentType?: string; modelName?: string; reasoningPreset?: string };
  };
  const sessions = new Map<string, TestSession>();
  const subscribers = new Set<() => void>();
  const store = {
    getState: () => ({ sessions }),
    subscribe: vi.fn((callback: () => void) => {
      subscribers.add(callback);
      return () => subscribers.delete(callback);
    }),
    updateSessionModelName: vi.fn(),
    updateSessionReasoningPreset: vi.fn(),
    updateSessionMaxContextTokens: vi.fn(),
    updateAcpContextUsage: vi.fn(),
  };
  return { sessions, subscribers, store };
});

vi.mock('@/infrastructure/api/service-api/AIApi', () => ({
  aiApi: aiApiMocks,
}));

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@bitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@bitfun/ui')>(),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  Switch: () => null,
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: {
    getConfigs: vi.fn(),
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
  globalEventBus: { emit: vi.fn(), on: vi.fn(), off: vi.fn() },
}));

vi.mock('../store/FlowChatStore', () => ({
  FlowChatStore: { getInstance: () => flowChatStoreMocks.store },
}));

const model = (
  id: string,
  providerName: string,
  providerInstanceId: string | undefined,
  baseUrl: string,
) => ({
  id,
  name: providerName,
  model_name: `${id}-native`,
  provider: 'openai',
  base_url: baseUrl,
  enabled: true,
  category: 'text',
  capabilities: ['text_chat'],
  ...(providerInstanceId
    ? { metadata: { provider_instance_id: providerInstanceId } }
    : {}),
});

const CATALOG_MODELS = [
  model('acme-fast', 'Acme', 'provider-acme', 'https://acme.test/v1'),
  model('acme-deep', 'Acme', 'provider-acme', 'https://acme.test/v1'),
  model('umbra-main', 'Umbra', 'provider-umbra', 'https://umbra.test/v1'),
];

const providerRows = () => Array.from(
  document.body.querySelectorAll<HTMLButtonElement>(
    '[data-testid="chat-model-selector-provider"]',
  ),
);

const modelOption = (modelId: string) => document.body.querySelector<HTMLButtonElement>(
  `[data-testid="chat-model-selector-option"][data-model-id="${modelId}"]`,
);

describe('ModelSelector provider levels', () => {
  let container: HTMLDivElement;
  let root: Root;

  const openSettingsMenu = async () => {
    await act(async () => {
      container.querySelector<HTMLButtonElement>(
        '[data-testid="chat-model-selector-btn"]',
      )?.click();
    });
  };

  const openMenu = async () => {
    await openSettingsMenu();
  };

  const openProvider = async (providerKey: string) => {
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>(
        `[data-testid="chat-model-selector-provider"][data-provider-key="${providerKey}"]`,
      )?.click();
    });
  };

  const nativeSubmenu = () => document.body.querySelector<HTMLElement>(
    '[data-testid="chat-model-selector-submenu"]',
  );

  const sharedSubmenuItems = () => nativeSubmenu()?.querySelector<HTMLElement>(
    '[data-bf-part="section-items"]',
  ) ?? null;

  const settingsSection = () => document.body.querySelector<HTMLElement>(
    '[data-testid="chat-model-selector-settings"]',
  );

  const sharedSettingsItems = () => settingsSection()?.querySelector<HTMLElement>(
    '[data-bf-part="section-items"]',
  ) ?? null;

  const renderSelector = async (
    models: unknown[] = CATALOG_MODELS,
    modeModel = 'primary',
    sessionId?: string,
  ) => {
    vi.mocked(configManager.getConfigs).mockResolvedValue({
      'ai.models': models,
      'ai.default_models': { primary: 'acme-fast', fast: 'umbra-main' },
      'ai.agent_model_defaults': { mode: modeModel },
    });

    await act(async () => {
      root.render(
        <ModelSelector
          currentMode="agentic"
          sessionId={sessionId}
          reasoningTriggerPresentation="label"
        />,
      );
      await Promise.resolve();
    });
  };

  beforeEach(() => {
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
    aiApiMocks.getModelCatalog.mockResolvedValue({
      version: 1,
      default_models: { primary: 'acme-fast' },
      models: [],
    });
    aiApiMocks.onModelCatalogUpdated.mockImplementation(() => () => undefined);
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

  it('opens with providers, reasoning settings, and a way to restore defaults', async () => {
    flowChatStoreMocks.sessions.set('session-a', {
      config: {
        agentType: 'agentic',
        modelName: 'umbra-main',
        reasoningPreset: 'high',
      },
    });
    aiApiMocks.getModelCatalog.mockResolvedValue({
      version: 1,
      default_models: { primary: 'acme-fast' },
      models: [{
        id: 'umbra-main',
        reasoning: {
          status: 'known',
          default_preset: 'medium',
          presets: [
            { id: 'medium', label: 'Medium', order: 10, source: 'models_dev', actions: [{ type: 'effort', value: 'medium' }] },
            { id: 'high', label: 'High', order: 20, source: 'models_dev', actions: [{ type: 'effort', value: 'high' }] },
          ],
        },
      }],
    });

    await renderSelector(CATALOG_MODELS, 'primary', 'session-a');
    await openSettingsMenu();

    const settings = settingsSection();
    expect(settings).not.toBeNull();
    expect(settings?.querySelector(
      '[data-testid="chat-model-selector-settings-model"]',
    )).toBeNull();
    expect(providerRows().map(row => row.dataset.providerKey))
      .toEqual(['provider-acme', 'provider-umbra']);
    expect(providerRows().find(row => row.dataset.providerKey === 'provider-umbra')?.textContent)
      .toContain('umbra-main-native');
    expect(settings?.querySelector(
      '[data-testid="chat-model-selector-settings-reasoning"]',
    )?.textContent).toContain('reasoningSelector.levels.high');
    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-model-selector-btn"]',
    );
    const reasoningSummary = trigger?.querySelector(
      '[data-testid="chat-model-selector-trigger-reasoning"]',
    );
    const dropdownIndicator = trigger?.querySelector(
      '[data-testid="chat-model-selector-dropdown-indicator"]',
    );
    expect(reasoningSummary?.textContent).toContain('reasoningSelector.levels.high');
    expect(reasoningSummary?.nextElementSibling).toBe(dropdownIndicator);
    expect(
      container.querySelector('[data-testid="chat-reasoning-preset-selector-btn"]'),
    ).toBeNull();

    await act(async () => {
      settings?.querySelector<HTMLButtonElement>(
        '[data-testid="chat-model-selector-settings-reset"]',
      )?.click();
      await Promise.resolve();
    });

    expect(configManager.setConfig).toHaveBeenCalledWith(
      'ai.agent_model_defaults.mode',
      'primary',
    );
    expect(flowChatStoreMocks.store.updateSessionReasoningPreset)
      .toHaveBeenCalledWith('session-a', undefined);
  });

  it('opens the reasoning presets from the settings summary', async () => {
    flowChatStoreMocks.sessions.set('session-a', {
      config: { agentType: 'agentic', modelName: 'umbra-main', reasoningPreset: 'high' },
    });
    aiApiMocks.getModelCatalog.mockResolvedValue({
      version: 1,
      default_models: { primary: 'acme-fast' },
      models: [{
        id: 'umbra-main',
        reasoning: {
          status: 'known',
          default_preset: 'medium',
          presets: [
            { id: 'medium', label: 'Medium', order: 10, source: 'models_dev', actions: [{ type: 'effort', value: 'medium' }] },
            { id: 'high', label: 'High', order: 20, source: 'models_dev', actions: [{ type: 'effort', value: 'high' }] },
          ],
        },
      }],
    });

    await renderSelector(CATALOG_MODELS, 'primary', 'session-a');
    await openSettingsMenu();
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>(
        '[data-testid="chat-model-selector-settings-reasoning"]',
      )?.click();
    });

    expect(document.body.querySelector(
      '[data-testid="chat-model-selector-settings"]',
    )).not.toBeNull();
    expect(nativeSubmenu()?.dataset.submenuKind).toBe('reasoning');
    const options = Array.from(document.body.querySelectorAll<HTMLButtonElement>(
      '[data-testid="chat-model-selector-reasoning-option"]',
    ));
    expect(sharedSubmenuItems()).not.toBeNull();
    expect(options.every(option => sharedSubmenuItems()?.contains(option))).toBe(true);
    expect(options.map(option => option.dataset.presetId))
      .toEqual(['auto', 'medium', 'high']);
    expect(options.every(option => (
      option.querySelector('.bitfun-model-selector__option-desc') === null
    ))).toBe(true);
    expect(options.every(option => option.querySelector('svg') === null)).toBe(true);
    expect(options.find(option => option.dataset.presetId === 'high')?.getAttribute('aria-checked'))
      .toBe('true');
  });

  it('offers providers first and keeps the symbolic selectors on that level', async () => {
    await renderSelector();
    await openMenu();

    expect(settingsSection()).not.toBeNull();
    expect(nativeSubmenu()).toBeNull();
    expect(sharedSettingsItems()).not.toBeNull();
    expect(providerRows().every(row => sharedSettingsItems()?.contains(row))).toBe(true);
    expect(sharedSettingsItems()?.contains(modelOption('primary'))).toBe(true);
    expect(sharedSettingsItems()?.contains(modelOption('fast'))).toBe(true);
    expect(providerRows().map(row => row.dataset.providerKey))
      .toEqual(['provider-acme', 'provider-umbra']);
    expect(modelOption('primary')).not.toBeNull();
    expect(modelOption('fast')).not.toBeNull();
    expect(settingsSection()?.querySelector(
      '[data-testid="chat-model-selector-settings-model"]',
    )).toBeNull();
    expect(document.body.querySelector(
      '[data-testid="chat-model-selector-provider-selected-model"]',
    )).toBeNull();
    // A concrete model is only reachable through its provider now.
    expect(modelOption('acme-deep')).toBeNull();
    expect(modelOption('umbra-main')).toBeNull();
  });

  it('shows only the chosen provider\'s models and applies a selection', async () => {
    await renderSelector();
    await openMenu();
    await openProvider('provider-acme');

    expect(nativeSubmenu()?.dataset.submenuKind).toBe('models');
    expect(providerRows()).toHaveLength(2);
    expect(modelOption('acme-fast')).not.toBeNull();
    expect(modelOption('acme-deep')).not.toBeNull();
    expect(modelOption('umbra-main')).toBeNull();
    expect(sharedSubmenuItems()).not.toBeNull();
    expect(sharedSubmenuItems()?.contains(modelOption('acme-fast'))).toBe(true);
    expect(sharedSubmenuItems()?.contains(modelOption('acme-deep'))).toBe(true);
    // The symbolic selectors stay on the first level and are not repeated.
    expect(sharedSettingsItems()?.contains(modelOption('primary'))).toBe(true);
    expect(sharedSubmenuItems()?.contains(modelOption('primary'))).toBe(false);

    await act(async () => {
      modelOption('acme-deep')?.click();
      await Promise.resolve();
    });

    expect(configManager.setConfig).toHaveBeenCalledWith(
      'ai.agent_model_defaults.mode',
      'acme-deep',
    );
  });

  it('marks the provider that owns the pinned model and shows that model beneath it', async () => {
    await renderSelector(CATALOG_MODELS, 'umbra-main');
    await openMenu();

    const selectedKeys = providerRows()
      .filter(row => row.dataset.selected === 'true')
      .map(row => row.dataset.providerKey);
    expect(selectedKeys).toEqual(['provider-umbra']);

    const selectedProvider = providerRows().find(
      row => row.dataset.providerKey === 'provider-umbra',
    );
    const selectedModel = selectedProvider?.querySelector<HTMLElement>(
      '[data-testid="chat-model-selector-provider-selected-model"]',
    );
    expect(selectedModel?.dataset.modelId).toBe('umbra-main');
    expect(selectedModel?.textContent).toBe('umbra-main-native');
    expect(selectedModel?.lastElementChild?.getAttribute('data-testid'))
      .toBe('chat-model-selector-provider-selected-check');
    expect(
      providerRows()
        .find(row => row.dataset.providerKey === 'provider-acme')
        ?.querySelector('[data-testid="chat-model-selector-provider-selected-model"]'),
    ).toBeNull();
  });

  it('keeps the provider level stable while switching providers and on reopen', async () => {
    await renderSelector();
    await openMenu();
    await openProvider('provider-acme');

    expect(providerRows()).toHaveLength(2);
    expect(sharedSubmenuItems()?.contains(modelOption('acme-deep'))).toBe(true);

    await openProvider('provider-umbra');
    expect(sharedSubmenuItems()?.contains(modelOption('umbra-main'))).toBe(true);
    expect(modelOption('acme-deep')).toBeNull();

    // Closing and reopening starts over at the provider list.
    await act(async () => {
      container.querySelector<HTMLButtonElement>(
        '[data-testid="chat-model-selector-btn"]',
      )?.click();
    });
    await openSettingsMenu();
    expect(document.body.querySelector(
      '[data-testid="chat-model-selector-settings"]',
    )).not.toBeNull();
    expect(providerRows()).toHaveLength(2);
    expect(nativeSubmenu()).toBeNull();
    expect(modelOption('acme-deep')).toBeNull();
  });

  it('lets Escape close the model flyout before closing the provider menu', async () => {
    await renderSelector();
    await openMenu();
    await openProvider('provider-acme');

    const pressEscape = async () => {
      await act(async () => {
        (nativeSubmenu() ?? document.body
          .querySelector('[data-testid="chat-model-selector-menu"]'))
          ?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      });
    };

    await pressEscape();
    const menu = document.body.querySelector('[data-testid="chat-model-selector-menu"]');
    expect(menu?.getAttribute('data-open')).toBe('true');
    expect(nativeSubmenu()).toBeNull();
    expect(providerRows()).toHaveLength(2);

    await pressEscape();
    expect(
      document.body
        .querySelector('[data-testid="chat-model-selector-menu"]')
        ?.getAttribute('data-open'),
    ).toBe('false');
  });

  it('opens native submenus only by click, keeps the parent stable, and toggles them explicitly', async () => {
    flowChatStoreMocks.sessions.set('session-a', {
      config: { agentType: 'agentic', modelName: 'umbra-main', reasoningPreset: 'high' },
    });
    aiApiMocks.getModelCatalog.mockResolvedValue({
      version: 1,
      default_models: { primary: 'acme-fast' },
      models: [{
        id: 'umbra-main',
        reasoning: {
          status: 'known',
          default_preset: 'medium',
          presets: [
            { id: 'medium', label: 'Medium', order: 10, source: 'models_dev', actions: [{ type: 'effort', value: 'medium' }] },
            { id: 'high', label: 'High', order: 20, source: 'models_dev', actions: [{ type: 'effort', value: 'high' }] },
          ],
        },
      }],
    });

    await renderSelector(CATALOG_MODELS, 'primary', 'session-a');
    await openSettingsMenu();
    const providerRow = document.body.querySelector<HTMLButtonElement>(
      '[data-testid="chat-model-selector-provider"][data-provider-key="provider-acme"]',
    );
    const reasoningRow = document.body.querySelector<HTMLButtonElement>(
      '[data-testid="chat-model-selector-settings-reasoning"]',
    );

    await act(async () => {
      providerRow?.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      providerRow?.focus();
    });
    expect(nativeSubmenu()).toBeNull();

    await act(async () => providerRow?.click());
    expect(providerRow?.getAttribute('aria-expanded')).toBe('true');
    expect(nativeSubmenu()?.dataset.submenuKind).toBe('models');
    expect(document.body.querySelector(
      '[data-testid="chat-model-selector-settings"]',
    )).not.toBeNull();

    await act(async () => {
      providerRow?.dispatchEvent(new MouseEvent('mouseleave', { bubbles: true }));
      reasoningRow?.focus();
    });
    expect(nativeSubmenu()?.dataset.submenuKind).toBe('models');

    await act(async () => reasoningRow?.click());
    expect(providerRow?.getAttribute('aria-expanded')).toBe('false');
    expect(reasoningRow?.getAttribute('aria-expanded')).toBe('true');
    expect(nativeSubmenu()?.dataset.submenuKind).toBe('reasoning');

    await act(async () => reasoningRow?.click());
    expect(nativeSubmenu()).toBeNull();
    expect(document.body.querySelector(
      '[data-testid="chat-model-selector-settings"]',
    )).not.toBeNull();
  });

  it('supports Right and Left Arrow navigation and closes both menus on outside click', async () => {
    await renderSelector();
    await openSettingsMenu();
    const providerRow = document.body.querySelector<HTMLButtonElement>(
      '[data-testid="chat-model-selector-provider"][data-provider-key="provider-acme"]',
    );
    providerRow?.focus();

    await act(async () => {
      providerRow?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
      await new Promise(resolve => window.setTimeout(resolve, 25));
    });
    expect(nativeSubmenu()?.dataset.submenuKind).toBe('models');
    expect(nativeSubmenu()?.contains(document.activeElement)).toBe(true);

    await act(async () => {
      nativeSubmenu()?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    });
    expect(nativeSubmenu()).toBeNull();
    expect(document.activeElement).toBe(providerRow);

    await act(async () => providerRow?.click());
    expect(nativeSubmenu()).not.toBeNull();
    await act(async () => {
      document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    });
    expect(nativeSubmenu()).toBeNull();
    expect(document.body.querySelector(
      '[data-testid="chat-model-selector-menu"]',
    )?.getAttribute('data-open')).toBe('false');
  });

  it('keeps a config written before the provider-instance migration visible', async () => {
    // Upgraded installs can still hold models without the grouping metadata;
    // they must stay selectable as their own provider rather than disappear.
    await renderSelector([
      ...CATALOG_MODELS,
      model('legacy-model', 'Legacy endpoint', undefined, 'https://legacy.test/v1'),
    ]);
    await openMenu();

    expect(providerRows().map(row => row.dataset.providerKey))
      .toEqual(['provider-acme', 'provider-umbra', 'legacy-model']);

    await openProvider('legacy-model');
    expect(modelOption('legacy-model')).not.toBeNull();
  });
});
