/**
 * @vitest-environment jsdom
 *
 * The composer's mode picker for ACP sessions. An agent publishes its modes as
 * a `mode`-category config option, and it gets a trigger of its own beside the
 * model picker — the two are unrelated choices and used to share one dropdown.
 * This covers the agent that publishes only a mode, the agent that publishes
 * both, and the agent that has fixed the mode and left exactly one choice.
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelSelector } from './ModelSelector';
import { ACPClientAPI } from '@/infrastructure/api/service-api/ACPClientAPI';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const aiApiMocks = vi.hoisted(() => ({
  getModelCatalog: vi.fn(),
  onModelCatalogUpdated: vi.fn(),
}));

const flowChatStoreMocks = vi.hoisted(() => {
  type TestSession = {
    workspacePath?: string;
    remoteConnectionId?: string;
    remoteSshHost?: string;
    config: { agentType?: string; modelName?: string; workspacePath?: string };
  };
  const sessions = new Map<string, TestSession>();
  const store = {
    getState: () => ({ sessions }),
    subscribe: vi.fn(() => () => undefined),
    updateSessionModelName: vi.fn(),
    updateSessionReasoningPreset: vi.fn(),
    updateSessionMaxContextTokens: vi.fn(),
    updateAcpContextUsage: vi.fn(),
  };
  return { sessions, store };
});

vi.mock('@/infrastructure/api/service-api/AIApi', () => ({ aiApi: aiApiMocks }));

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@openbitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@openbitfun/ui')>(),
  // Keep the hover copy inspectable: descriptions live only in the tooltip now.
  Tooltip: ({ children, content }: { children: React.ReactNode; content?: React.ReactNode }) => (
    <span data-tooltip={typeof content === 'string' ? content : undefined}>{children}</span>
  ),
  Switch: () => null,
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: {
    getConfigs: vi.fn(async () => ({ 'ai.models': [] })),
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
    setSessionConfigOption: vi.fn(),
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

/** The mode option as an unlocked agent publishes it. */
const MODE_OPTION = {
  id: 'agent-preset',
  name: 'Mode',
  category: 'mode',
  type: 'select' as const,
  currentValue: 'standard',
  options: [
    { value: 'standard', name: 'Standard', description: 'the default toolset' },
    { value: 'minimal', name: 'Minimal', description: 'bash and an editor' },
  ],
};

describe('ModelSelector ACP mode picker', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    flowChatStoreMocks.sessions.clear();
    flowChatStoreMocks.sessions.set('acp-session', {
      workspacePath: '/tmp/project',
      config: { agentType: 'acp:dsh' },
    });
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, String(value)),
      removeItem: (key: string) => storage.delete(key),
      clear: () => storage.clear(),
      key: (index: number) => [...storage.keys()][index] ?? null,
      get length() { return storage.size; },
    });
    aiApiMocks.getModelCatalog.mockResolvedValue({ version: 1, default_models: {}, models: [] });
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

  /** Render the selector against one set of ACP session options. */
  const renderWithOptions = async (
    configOptions: unknown[],
    session: { availableModels?: unknown[]; currentModelId?: string } = {},
  ) => {
    vi.mocked(ACPClientAPI.getSessionOptions).mockResolvedValue({
      availableModels: session.availableModels ?? [],
      ...(session.currentModelId ? { currentModelId: session.currentModelId } : {}),
      configOptions,
    } as never);
    await act(async () => {
      root.render(<ModelSelector currentMode="acp:dsh" sessionId="acp-session" reasoningTriggerPresentation="label" />);
      await Promise.resolve();
    });
    await act(async () => { await Promise.resolve(); });
  };

  it('renders the mode as the whole picker when the agent offers no models', async () => {
    await renderWithOptions([MODE_OPTION]);

    const trigger = container.querySelector<HTMLButtonElement>('[data-testid="chat-acp-mode-selector-btn"]');
    // Without this the ACP branch used to return null on an empty model list,
    // leaving a dsh session with no picker at all.
    expect(trigger, 'the mode picker must render without models').not.toBeNull();
    expect(trigger?.textContent).toContain('Standard');
    // Nothing left for the model picker to show, so it stays away entirely.
    expect(container.querySelector('[data-testid="chat-model-selector-btn"]')).toBeNull();

    await act(async () => { trigger?.click(); });
    const options = document.body.querySelectorAll('[data-testid="chat-acp-mode-option"]');
    expect([...options].map(option => option.getAttribute('data-mode-value')))
      .toEqual(['standard', 'minimal']);
  });

  it('keeps the mode out of the model dropdown when the agent offers both', async () => {
    await renderWithOptions([MODE_OPTION], {
      availableModels: [
        { id: 'deepseek-official/deepseek-v4-flash', name: 'DeepSeek V4 Flash' },
        { id: 'deepseek-official/deepseek-v4-pro', name: 'DeepSeek V4 Pro' },
      ],
      currentModelId: 'deepseek-official/deepseek-v4-flash',
    });

    const modelTrigger = container.querySelector<HTMLButtonElement>('[data-testid="chat-model-selector-btn"]');
    const modeTrigger = container.querySelector<HTMLButtonElement>('[data-testid="chat-acp-mode-selector-btn"]');
    expect(modelTrigger, 'the model picker keeps its own trigger').not.toBeNull();
    expect(modeTrigger?.textContent).toContain('Standard');

    // The whole point of the split: one button, one kind of choice.
    await act(async () => { modelTrigger?.click(); });
    const modelMenu = document.body.querySelector('[data-testid="chat-model-selector-menu"]');
    expect(modelMenu?.querySelectorAll('[data-testid="chat-model-selector-option"]')).toHaveLength(2);
    expect(modelMenu?.querySelectorAll('[data-testid="chat-acp-mode-option"]')).toHaveLength(0);

    await act(async () => { modeTrigger?.click(); });
    expect(
      document.body.querySelectorAll('[data-testid="chat-acp-mode-selector-menu"] [data-testid="chat-acp-mode-option"]'),
    ).toHaveLength(2);
  });

  it('sends the chosen mode to the agent', async () => {
    await renderWithOptions([MODE_OPTION]);
    vi.mocked(ACPClientAPI.setSessionConfigOption).mockResolvedValue({
      availableModels: [],
      configOptions: [{ ...MODE_OPTION, currentValue: 'minimal' }],
    } as never);

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="chat-acp-mode-selector-btn"]')?.click();
    });
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>(
        '[data-testid="chat-acp-mode-option"][data-mode-value="minimal"]',
      )?.click();
      await Promise.resolve();
    });

    expect(ACPClientAPI.setSessionConfigOption).toHaveBeenCalledWith(expect.objectContaining({
      sessionId: 'acp-session',
      clientId: 'dsh',
      configId: 'agent-preset',
      value: { type: 'select', value: 'minimal' },
    }));
    expect(
      container.querySelector('[data-testid="chat-acp-mode-selector-btn"]')?.textContent,
    ).toContain('Minimal');
  });

  it('disables the picker once the agent leaves one mode', async () => {
    // How an agent says "fixed" without a flag ACP does not have: it publishes
    // only the mode in force, and explains itself in the option description.
    await renderWithOptions([{
      ...MODE_OPTION,
      description: 'This conversation has already started, so its mode is fixed.',
      options: [MODE_OPTION.options[0]],
    }]);

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="chat-acp-mode-selector-btn"]')?.click();
    });
    const options = document.body.querySelectorAll<HTMLButtonElement>(
      '[data-testid="chat-acp-mode-option"]',
    );
    expect(options).toHaveLength(1);
    expect(options[0]?.disabled).toBe(true);
    // The row itself stays a bare mode name; the reason is hover-only.
    expect(options[0]?.textContent).toBe('Standard');
    expect(options[0]?.getAttribute('title')).toContain('already started');

    await act(async () => {
      options[0]?.click();
      await Promise.resolve();
    });
    expect(ACPClientAPI.setSessionConfigOption).not.toHaveBeenCalled();
  });

  const reasoningOption = {
    id: 'reasoning-effort', name: 'Reasoning', category: 'thought_level',
    type: 'select' as const, currentValue: 'medium',
    options: [{ value: 'medium', name: 'Medium' }, { value: 'high', name: 'High' }],
  };
  const modelOptions = {
    availableModels: [{ id: 'remote-model', name: 'Remote Model' }],
    currentModelId: 'remote-model',
  };
  const click = async (testId: string) => {
    const button = document.body.querySelector<HTMLButtonElement>(`[data-testid="${testId}"]`);
    expect(button).not.toBeNull();
    await act(async () => { button?.click(); });
  };

  it('shares model and reasoning settings and writes the selected value through the remote ACP adapter', async () => {
    Object.assign(flowChatStoreMocks.sessions.get('acp-session')!, {
      remoteConnectionId: 'ssh-connection', remoteSshHost: 'a100',
    });
    await renderWithOptions([MODE_OPTION, reasoningOption], modelOptions);
    expect(container.querySelector('[data-testid="chat-reasoning-preset-selector-btn"]')).toBeNull();
    expect(container.querySelector('[data-testid="chat-model-selector-trigger-reasoning"]')?.textContent)
      .toContain('reasoningSelector.levels.medium');
    await click('chat-model-selector-btn');
    expect(document.body.querySelector('[data-testid="chat-model-selector-settings-model"]')).not.toBeNull();
    await click('chat-model-selector-settings-model');
    expect(document.body.querySelectorAll('[data-testid="chat-model-selector-option"]')).toHaveLength(1);
    await click('chat-model-selector-settings-reasoning');
    expect(document.body.querySelector('[data-preset-id="auto"]')).toBeNull();
    vi.mocked(ACPClientAPI.setSessionConfigOption).mockResolvedValue({
      ...modelOptions, configOptions: [MODE_OPTION, { ...reasoningOption, currentValue: 'high' }],
    } as never);
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>('[data-preset-id="high"]')?.click();
    });
    expect(ACPClientAPI.setSessionConfigOption).toHaveBeenCalledWith(expect.objectContaining({
      sessionId: 'acp-session', clientId: 'dsh', configId: 'reasoning-effort',
      remoteConnectionId: 'ssh-connection', remoteSshHost: 'a100',
      value: { type: 'select', value: 'high' },
    }));
    expect(container.querySelector('[data-testid="chat-model-selector-trigger-reasoning"]')?.textContent)
      .toContain('reasoningSelector.levels.high');
    expect(container.querySelector('[data-testid="chat-model-selector-btn"]')?.getAttribute('aria-expanded')).toBe('false');
  });

  it('keeps reasoning accessible when ACP advertises no model list', async () => {
    await renderWithOptions([MODE_OPTION, reasoningOption]);
    await click('chat-model-selector-btn');
    expect(document.body.querySelector('[data-testid="chat-model-selector-settings-model"]')).toBeNull();
    await click('chat-model-selector-settings-reasoning');
    expect(document.body.querySelectorAll('[data-testid="chat-model-selector-reasoning-option"]')).toHaveLength(2);
    expect(container.querySelector('[data-testid="chat-acp-mode-selector-btn"]')).not.toBeNull();
  });

  it('navigates the shared ACP reasoning submenu with the keyboard and restores focus', async () => {
    await renderWithOptions([reasoningOption], modelOptions);
    await click('chat-model-selector-btn');
    const row = document.body.querySelector<HTMLButtonElement>('[data-testid="chat-model-selector-settings-reasoning"]')!;
    await act(async () => {
      row.focus();
      row.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    });
    expect(document.activeElement?.getAttribute('data-preset-id')).toBe('medium');
    await act(async () => {
      document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    });
    expect(document.activeElement).toBe(row);
    await act(async () => {
      row.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    });
    expect(document.activeElement).toBe(container.querySelector('[data-testid="chat-model-selector-btn"]'));
  });

  it('retains the current reasoning value after an ACP update fails', async () => {
    await renderWithOptions([reasoningOption], modelOptions);
    vi.mocked(ACPClientAPI.setSessionConfigOption).mockRejectedValueOnce(new Error('Host offline'));
    await click('chat-model-selector-btn');
    await click('chat-model-selector-settings-reasoning');
    await act(async () => {
      document.body.querySelector<HTMLButtonElement>('[data-preset-id="high"]')?.click();
    });
    expect(container.querySelector('[data-testid="chat-model-selector-trigger-reasoning"]')?.textContent)
      .toContain('reasoningSelector.levels.medium');
    expect(container.querySelector<HTMLButtonElement>('[data-testid="chat-model-selector-btn"]')?.disabled).toBe(false);
  });

  it('keeps the advertised fast mode available in the shared settings menu', async () => {
    const fastMode = { id: 'fast-mode', name: 'Fast', type: 'boolean', currentValue: false };
    await renderWithOptions([reasoningOption, fastMode], modelOptions);
    await click('chat-model-selector-btn');
    const fastRow = document.body.querySelector<HTMLButtonElement>('[role="menuitemcheckbox"]');
    expect(fastRow).not.toBeNull();
    vi.mocked(ACPClientAPI.setSessionConfigOption).mockResolvedValue({
      ...modelOptions, configOptions: [reasoningOption, { ...fastMode, currentValue: true }],
    } as never);
    await act(async () => { fastRow?.click(); });
    expect(ACPClientAPI.setSessionConfigOption).toHaveBeenCalledWith(expect.objectContaining({
      configId: 'fast-mode', value: { type: 'boolean', value: true },
    }));
  });


  it('uses an advertised ACP auto value instead of synthesizing a local reset', async () => {
    const withAuto = { ...reasoningOption, options: [...reasoningOption.options, { value: 'auto', name: 'Auto' }] };
    await renderWithOptions([withAuto], modelOptions);
    await click('chat-model-selector-btn');
    await click('chat-model-selector-settings-reasoning');
    expect(document.body.querySelectorAll('[data-preset-id="auto"]')).toHaveLength(1);
    vi.mocked(ACPClientAPI.setSessionConfigOption).mockResolvedValue({
      ...modelOptions, configOptions: [{ ...withAuto, currentValue: 'auto' }],
    } as never);
    await act(async () => { document.body.querySelector<HTMLButtonElement>('[data-preset-id="auto"]')?.click(); });
    expect(ACPClientAPI.setSessionConfigOption).toHaveBeenCalledWith(expect.objectContaining({
      configId: 'reasoning-effort', value: { type: 'select', value: 'auto' },
    }));
  });

  it('shows an advertised model when an older ACP payload omits the current model id', async () => {
    await renderWithOptions([reasoningOption], { availableModels: modelOptions.availableModels });
    expect(container.querySelector('[data-testid="chat-model-selector-btn"]')?.textContent).toContain('Remote Model');
  });

});
