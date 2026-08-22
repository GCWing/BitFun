import { configAPI } from '@/infrastructure/api';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { INTERACTIVE_CAPABILITY_CATALOG } from './interactiveCapabilityCatalog';
import {
  discoverBitFunCapabilities,
  executeBitFunControlRequest,
} from './bitfunControlBridge';

describe('BitFunControl discovery', () => {
  afterEach(() => vi.restoreAllMocks());

  it('lists the shared catalog with bounded pagination', () => {
    const result = discoverBitFunCapabilities({
      requestId: 'test',
      action: 'list',
      limit: 500,
    }) as { items: unknown[]; totalCount: number; nextCursor: number | null };
    expect(result.items).toHaveLength(38);
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

  it('returns matching documented sub-capabilities without loading the full catalog into the tool prompt', () => {
    const result = discoverBitFunCapabilities({
      requestId: 'browser-picker',
      action: 'search',
      query: '元素选择器',
    }) as { items: Array<{ id: string; matchedItems: Array<{ id: string }> }> };
    const browser = result.items.find(({ id }) => id === 'feature.browser');
    expect(browser?.matchedItems.some(({ id }) => id === 'element-picker')).toBe(true);
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

  it('returns one public setting contract without leaking internal handlers', async () => {
    vi.spyOn(configAPI, 'getConfig').mockImplementation(async (path) => (
      path === 'appearance.selection' ? 'system' : 'zh-CN'
    ));
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
    const setConfig = vi.spyOn(configAPI, 'setConfig').mockResolvedValue(undefined);
    await executeBitFunControlRequest({
      requestId: 'configure-theme',
      action: 'configure',
      capabilityId: 'setting.application.appearance',
      optionId: 'theme',
      value: 'bitfun-dark',
    });
    expect(setConfig).toHaveBeenCalledWith('appearance.selection', 'bitfun-dark');
    await expect(executeBitFunControlRequest({
      requestId: 'raw-command',
      action: 'configure',
      capabilityId: 'set_config',
      optionId: 'anything',
      value: true,
    })).rejects.toThrow('Unknown BitFun capability');
  });

  it('merges grouped settings without overwriting sibling values', async () => {
    vi.spyOn(configAPI, 'getConfig').mockResolvedValue({
      enable_agent_companion: true,
      agent_companion_display_mode: 'desktop',
    });
    const setConfig = vi.spyOn(configAPI, 'setConfig').mockResolvedValue(undefined);
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
});
