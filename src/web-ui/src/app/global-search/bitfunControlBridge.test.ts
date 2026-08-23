import { appearanceService } from '@/infrastructure/appearance';
import { configManager } from '@/infrastructure/config';
import { i18nService } from '@/infrastructure/i18n';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { INTERACTIVE_CAPABILITY_CATALOG } from './interactiveCapabilityCatalog';
import {
  discoverBitFunCapabilities,
  executeBitFunControlRequest,
} from './bitfunControlBridge';

const mocks = vi.hoisted(() => ({
  activateInteractiveCapability: vi.fn(),
}));

vi.mock('./interactiveCapabilityActivator', () => ({
  activateInteractiveCapability: mocks.activateInteractiveCapability,
}));

describe('BitFunControl discovery', () => {
  afterEach(() => {
    vi.clearAllMocks();
    vi.restoreAllMocks();
  });

  it('lists the shared catalog with bounded pagination', () => {
    const result = discoverBitFunCapabilities({
      requestId: 'test',
      action: 'list',
    }) as { items: unknown[]; totalCount: number; nextCursor: number | null };
    expect(result.items).toHaveLength(INTERACTIVE_CAPABILITY_CATALOG.counts.userFacing);
    expect(result.totalCount).toBe(INTERACTIVE_CAPABILITY_CATALOG.capabilities.length);
    expect(result.nextCursor).toBeNull();
  });

  it('searches Chinese and English against the same source', () => {
    const english = discoverBitFunCapabilities({
      requestId: 'en',
      action: 'search',
      query: 'terminal',
    }) as { items: Array<{ id: string }> };
    const chinese = discoverBitFunCapabilities({
      requestId: 'zh',
      action: 'search',
      query: '终端',
    }) as { items: Array<{ id: string }> };
    expect(english.items.some(({ id }) => id === 'feature.terminal')).toBe(true);
    expect(chinese.items.some(({ id }) => id === 'feature.terminal')).toBe(true);
  });

  it('recovers the companion setting from bilingual synonym bundles used in conversation', () => {
    for (const query of ['宠物 pet mascot 桌面宠物', 'pet mascot appearance']) {
      const result = discoverBitFunCapabilities({
        requestId: `pet-${query}`,
        action: 'search',
        query,
      }) as { items: Array<{ id: string }> };
      expect(result.items.some(({ id }) => id === 'setting.application.pet')).toBe(true);
    }
  });

  it('returns matching documented sub-capabilities without loading the full catalog into the tool prompt', () => {
    const result = discoverBitFunCapabilities({
      requestId: 'browser-picker',
      action: 'search',
      query: '元素选择器',
    }) as { items: Array<{ id: string; matchedItems: Array<{ id: string }> }> };
    const browser = result.items.find(({ id }) => id === 'feature.browser');
    expect(browser?.matchedItems.some(({ id }) => id === 'element-picker')).toBe(true);
  });

  it('returns the on-demand shared browser control workflow to the agent', async () => {
    const result = await executeBitFunControlRequest({
      requestId: 'get-browser-control',
      action: 'get',
      capabilityId: 'feature.browser',
    }) as {
      capability: {
        agentControl?: { tool: string; workflowZh: string[]; workflowEn: string[] };
      };
    };
    expect(result.capability.agentControl?.tool).toBe('ControlHub');
    expect(result.capability.agentControl?.workflowZh.join(' ')).toContain('browser.open_builtin');
    expect(result.capability.agentControl?.workflowEn.join(' ')).toContain('external CDP contract');
  });

  it('discovers personal-assistant configuration as its own user feature', () => {
    const result = discoverBitFunCapabilities({
      requestId: 'assistant-persona',
      action: 'search',
      query: 'IDENTITY.md',
    }) as { items: Array<{ id: string; matchedItems: Array<{ id: string }> }> };
    const assistant = result.items.find(({ id }) => id === 'feature.personal-assistants');
    expect(assistant).toBeDefined();
    expect(assistant?.matchedItems.some(({ id }) => id === 'persona-documents')).toBe(true);
  });

  it('never lists raw Tauri commands as user capabilities', () => {
    const result = discoverBitFunCapabilities({
      requestId: 'semantic',
      action: 'list',
    }) as { items: Array<{ id: string; kind: string }> };
    expect(result.items.every(({ id, kind }) =>
      /^(feature|setting)\./.test(id) && ['feature', 'setting'].includes(kind))).toBe(true);
    expect(result.items.some(({ id }) => id === 'get_configs')).toBe(false);
  });

  it('opens the exact documented item returned by discovery', async () => {
    await executeBitFunControlRequest({
      requestId: 'open-shortcuts',
      action: 'open',
      capabilityId: 'setting.application.input',
      itemId: 'shortcut-browser',
    });
    expect(mocks.activateInteractiveCapability).toHaveBeenCalledWith(
      'setting.application.input',
      { itemId: 'shortcut-browser' },
    );
  });

  it('returns one public setting contract without leaking internal handlers', async () => {
    vi.spyOn(appearanceService, 'initialize').mockResolvedValue(undefined);
    vi.spyOn(appearanceService, 'getSnapshot').mockReturnValue({
      ...appearanceService.getSnapshot(),
      selectedAppearanceId: 'system',
    });
    vi.spyOn(i18nService, 'getCurrentLocale').mockReturnValue('zh-CN');
    const result = await executeBitFunControlRequest({
      requestId: 'get-setting',
      action: 'get',
      capabilityId: 'setting.application.appearance',
    }) as { capability: { items: Array<{ id: string }> }; currentOptionValues: Record<string, unknown> };
    expect(JSON.stringify(result.capability)).not.toContain('handler');
    expect(JSON.stringify(result.capability)).not.toContain('appearance.selection');
    expect(result.currentOptionValues).toEqual({ theme: 'system', language: 'zh-CN' });
    expect(result.capability.items.length).toBeGreaterThanOrEqual(4);
  });

  it('configures only an option resolved from the semantic catalog', async () => {
    const select = vi.spyOn(appearanceService, 'select').mockResolvedValue(undefined);
    vi.spyOn(appearanceService, 'initialize').mockResolvedValue(undefined);
    vi.spyOn(appearanceService, 'getSnapshot').mockReturnValue({
      ...appearanceService.getSnapshot(),
      selectedAppearanceId: 'bitfun-dark',
    });
    const result = await executeBitFunControlRequest({
      requestId: 'configure-theme',
      action: 'configure',
      capabilityId: 'setting.application.appearance',
      optionId: 'theme',
      value: 'bitfun-dark',
    }) as { effectiveValue: unknown };
    expect(select).toHaveBeenCalledWith('bitfun-dark');
    expect(result.effectiveValue).toBe('bitfun-dark');
    await expect(executeBitFunControlRequest({
      requestId: 'raw-command',
      action: 'configure',
      capabilityId: 'set_config',
      optionId: 'anything',
      value: true,
    })).rejects.toThrow('Unknown BitFun capability');
  });

  it('merges grouped settings without overwriting sibling values', async () => {
    vi.spyOn(configManager, 'getOptionalConfig').mockResolvedValue({
      enable_agent_companion: true,
      agent_companion_display_mode: 'desktop',
    });
    const setConfig = vi.spyOn(configManager, 'setConfig').mockResolvedValue(undefined);
    await executeBitFunControlRequest({
      requestId: 'configure-companion',
      action: 'configure',
      capabilityId: 'setting.application.pet',
      optionId: 'display-mode',
      value: 'input',
    });
    expect(setConfig).toHaveBeenCalledWith('app.ai_experience', {
      enable_agent_companion: true,
      agent_companion_display_mode: 'input',
    });
  });

  it('changes language through the live i18n service', async () => {
    const changeLanguage = vi.spyOn(i18nService, 'changeLanguage').mockResolvedValue(undefined);
    vi.spyOn(i18nService, 'getCurrentLocale').mockReturnValue('en-US');
    const result = await executeBitFunControlRequest({
      requestId: 'configure-language',
      action: 'configure',
      capabilityId: 'setting.application.appearance',
      optionId: 'language',
      value: 'en-US',
    }) as { effectiveValue: unknown };
    expect(changeLanguage).toHaveBeenCalledWith('en-US');
    expect(result.effectiveValue).toBe('en-US');
  });
});
