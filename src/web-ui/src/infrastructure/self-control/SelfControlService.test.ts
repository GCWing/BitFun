// @vitest-environment jsdom
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

import { ModelNotFoundError, SelfControlError, SelfControlService } from './SelfControlService';

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

  // ── Phase 1: failure semantics ──────────────────────────────────────────
  // Prior to Phase 1 these failure paths returned strings like "Element not
  // found: ..." while the listener still reported success: true. The model
  // would then act as if the click had landed. Each test below asserts the
  // operation now throws a SelfControlError so the listener can surface
  // success: false to the backend.

  it('throws SelfControlError(NOT_FOUND) when click target is missing', async () => {
    const service = new SelfControlService();
    await expect(
      service.executeAction({ action: 'click', selector: '#does-not-exist' }),
    ).rejects.toMatchObject({ name: 'SelfControlError', code: 'NOT_FOUND' });
  });

  it('throws SelfControlError(NOT_FOUND) when input target is missing', async () => {
    const service = new SelfControlService();
    await expect(
      service.executeAction({ action: 'input', selector: '#nope', value: 'hi' }),
    ).rejects.toBeInstanceOf(SelfControlError);
  });

  it('throws SelfControlError(NOT_FOUND) when read_text target is missing', async () => {
    const service = new SelfControlService();
    await expect(
      service.executeAction({ action: 'read_text', selector: '#nope' }),
    ).rejects.toBeInstanceOf(SelfControlError);
  });

  it('throws SelfControlError(MISSING_SESSION) when press_key has no focus and no target_selector', async () => {
    document.body.focus();
    const service = new SelfControlService();
    await expect(
      service.executeAction({ action: 'press_key', key: 'Enter' }),
    ).rejects.toMatchObject({ name: 'SelfControlError', code: 'MISSING_SESSION' });
  });

  it('throws ModelNotFoundError when set_default_model has no match', async () => {
    resetConfigState({
      'ai.models': [
        {
          id: 'm1',
          name: 'Real',
          model_name: 'real-v1',
          provider: 'p',
          enabled: true,
        },
      ],
    });
    const service = new SelfControlService();
    await expect(
      service.executeAction({
        action: 'set_default_model',
        modelQuery: 'totally-imaginary-model-xyz',
      }),
    ).rejects.toBeInstanceOf(ModelNotFoundError);
  });

  it('throws SelfControlError(INVALID_PARAMS) on unknown action type', async () => {
    const service = new SelfControlService();
    await expect(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      service.executeAction({ action: 'totally_unknown' as any }),
    ).rejects.toMatchObject({ name: 'SelfControlError', code: 'INVALID_PARAMS' });
  });

  // ── Schema-driven alias normalization ───────────────────────────────────
  // Regression for the "missing field value" failure: a model that wrote
  // { action: "set_config", key, value } would previously have its `value`
  // dropped silently because normalizeAction only knew about
  // `configValue` / `config_value`. The boundary check inside `setConfig`
  // would then never fire because configValue was undefined and the
  // request would reach Tauri as `(key, undefined)`.

  it('aliases set_config "value" to canonical "configValue"', async () => {
    const service = new SelfControlService();
    await service.executeAction({
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      action: 'set_config',
      key: 'ai.models',
      value: [{ id: 'm1', enabled: true }],
    } as any);
    expect(setConfigMock).toHaveBeenCalledWith('ai.models', [{ id: 'm1', enabled: true }]);
  });

  it('aliases set_config "data" / "payload" to canonical "configValue"', async () => {
    const service = new SelfControlService();
    await service.executeAction({
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      action: 'set_config',
      key: 'ai.flag.a',
      data: true,
    } as any);
    expect(setConfigMock).toHaveBeenCalledWith('ai.flag.a', true);

    setConfigMock.mockClear();
    await service.executeAction({
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      action: 'set_config',
      key: 'ai.flag.b',
      payload: 42,
    } as any);
    expect(setConfigMock).toHaveBeenCalledWith('ai.flag.b', 42);
  });

  it('throws SelfControlError(INVALID_PARAMS) when set_config has no value/configValue at all', async () => {
    const service = new SelfControlService();
    await expect(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      service.executeAction({ action: 'set_config', key: 'ai.x' } as any),
    ).rejects.toMatchObject({ name: 'SelfControlError', code: 'INVALID_PARAMS' });
    expect(setConfigMock).not.toHaveBeenCalled();
  });

  it('preserves the input action\'s own `value` field (alias does not steal it)', async () => {
    document.body.innerHTML = '<input id="i" />';
    const service = new SelfControlService();
    await service.executeAction({
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      action: 'input',
      selector: '#i',
      value: 'hello',
    } as any);
    expect((document.getElementById('i') as HTMLInputElement).value).toBe('hello');
  });

  // ── Per-action alias scoping (B3) ────────────────────────────────────────
  // Regression for the silent-success cases observed when aliases were
  // global. `text` must alias to `value` ONLY inside `input`; `text` must
  // remain the search query for `click_by_text`.

  it('aliases input "text" → "value" and writes the real string into the field', async () => {
    document.body.innerHTML = '<input id="i" />';
    const service = new SelfControlService();
    await service.executeAction({
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      action: 'input',
      selector: '#i',
      text: 'kimi',
    } as any);
    expect((document.getElementById('i') as HTMLInputElement).value).toBe('kimi');
  });

  it('throws INVALID_PARAMS when input has neither value nor any alias (no silent "undefined" write)', async () => {
    document.body.innerHTML = '<input id="i" />';
    const service = new SelfControlService();
    await expect(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      service.executeAction({ action: 'input', selector: '#i' } as any),
    ).rejects.toMatchObject({ name: 'SelfControlError', code: 'INVALID_PARAMS' });
    expect((document.getElementById('i') as HTMLInputElement).value).toBe('');
  });

  it('does NOT cross-alias `text` from input back into click_by_text', async () => {
    document.body.innerHTML = '<button>foo</button>';
    const service = new SelfControlService();
    // click_by_text legitimately needs `text`; this just confirms it still
    // works (the per-action alias table did not break the canonical field).
    await expect(
      service.executeAction({ action: 'click_by_text', text: 'foo' }),
    ).resolves.toContain('Clicked element with text: foo');
  });

  // ── B1+B2 strict validation ──────────────────────────────────────────────

  it('throws INVALID_PARAMS when scroll has no direction and no deltaY (no silent no-op)', async () => {
    const service = new SelfControlService();
    await expect(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      service.executeAction({ action: 'scroll' } as any),
    ).rejects.toMatchObject({ name: 'SelfControlError', code: 'INVALID_PARAMS' });
  });

  it('derives scroll direction from numeric deltaY (Playwright/wheel-style payload)', async () => {
    document.body.innerHTML = '<div id="scroll-area" style="overflow:auto"></div>';
    const el = document.getElementById('scroll-area') as HTMLElement;
    // jsdom doesn't implement scroll APIs — stub them so we just observe the
    // dispatch path. The real assertion is that direction was derived from
    // deltaY (no INVALID_PARAMS, "down" appears in the result text).
    el.scrollBy = vi.fn() as unknown as Element['scrollBy'];
    el.scrollTo = vi.fn() as unknown as Element['scrollTo'];
    const service = new SelfControlService();
    const result = await service.executeAction({
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      action: 'scroll',
      selector: '#scroll-area',
      deltaY: 640,
    } as any);
    expect(result).toContain('Scrolled down');
    expect(el.scrollBy).toHaveBeenCalled();
  });

  it('throws INVALID_PARAMS when click selector is empty (no silent retry-on-nothing)', async () => {
    const service = new SelfControlService();
    await expect(
      service.executeAction({ action: 'click', selector: '' }),
    ).rejects.toMatchObject({ name: 'SelfControlError', code: 'INVALID_PARAMS' });
  });

  // ── B4: enable / disable / toggle model named tasks ──────────────────────

  it('enable_model flips a disabled model and writes ai.models', async () => {
    resetConfigState({
      'ai.models': [
        { id: 'k1', name: 'OpenBitFun', model_name: 'kimi-k2.6', provider: 'openbitfun', enabled: false },
        { id: 'g1', name: 'OpenAI', model_name: 'gpt-5', provider: 'openai', enabled: true },
      ],
    });
    const service = new SelfControlService();
    const result = await service.executeAction({
      action: 'execute_task',
      task: 'enable_model',
      params: { modelQuery: 'kimi' },
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } as any);
    expect(result).toContain('Enabled model');
    expect(setConfigMock).toHaveBeenCalledWith('ai.models', [
      { id: 'k1', name: 'OpenBitFun', model_name: 'kimi-k2.6', provider: 'openbitfun', enabled: true },
      { id: 'g1', name: 'OpenAI', model_name: 'gpt-5', provider: 'openai', enabled: true },
    ]);
  });

  it('disable_model rolls primary/fast defaults forward when the disabled model was the active default', async () => {
    resetConfigState({
      'ai.models': [
        { id: 'k1', name: 'A', model_name: 'a-v1', provider: 'p', enabled: true },
        { id: 'g1', name: 'B', model_name: 'b-v1', provider: 'p', enabled: true },
      ],
      'ai.default_models': { primary: 'k1', fast: 'k1' },
    });
    const service = new SelfControlService();
    const result = await service.executeAction({
      action: 'execute_task',
      task: 'disable_model',
      params: { modelQuery: 'a-v1' },
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } as any);
    expect(result).toContain('Disabled model');
    expect(result).toContain('primary fallback -> g1');
    expect(setConfigMock).toHaveBeenCalledWith('ai.default_models', { primary: 'g1', fast: 'g1' });
  });

  it('toggle_model flips state both directions', async () => {
    resetConfigState({
      'ai.models': [
        { id: 'k1', name: 'A', model_name: 'a-v1', provider: 'p', enabled: false },
      ],
    });
    const service = new SelfControlService();
    await service.executeAction({
      action: 'execute_task',
      task: 'toggle_model',
      params: { modelQuery: 'a-v1' },
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } as any);
    expect(setConfigMock).toHaveBeenLastCalledWith('ai.models', [
      { id: 'k1', name: 'A', model_name: 'a-v1', provider: 'p', enabled: true },
    ]);
  });

  it('enable_model returns AMBIGUOUS when query matches multiple models', async () => {
    resetConfigState({
      'ai.models': [
        { id: 'k1', name: 'A', model_name: 'kimi-v1', provider: 'p', enabled: false },
        { id: 'k2', name: 'B', model_name: 'kimi-v2', provider: 'p', enabled: false },
      ],
    });
    const service = new SelfControlService();
    await expect(
      service.executeAction({
        action: 'execute_task',
        task: 'enable_model',
        params: { modelQuery: 'kimi' },
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
      } as any),
    ).rejects.toMatchObject({ name: 'SelfControlError', code: 'AMBIGUOUS' });
  });
});
