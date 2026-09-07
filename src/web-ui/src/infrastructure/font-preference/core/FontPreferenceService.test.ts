// @vitest-environment jsdom

import { beforeEach, describe, expect, it, vi } from 'vitest';

const configMocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
  setConfig: vi.fn(),
}));

vi.mock('@/infrastructure/api', () => ({
  configAPI: configMocks,
}));

import { FontPreferenceService } from './FontPreferenceService';

describe('FontPreferenceService', () => {
  beforeEach(() => {
    configMocks.getConfig.mockReset();
    configMocks.setConfig.mockReset();
    document.documentElement.removeAttribute('style');
  });

  it('applies only the canonical design-system font-size foundation', () => {
    const service = new FontPreferenceService();

    service.applyPreference({ uiSize: { level: 'default' } });

    expect(document.documentElement.style.getPropertyValue('--openbitfun-font-size-base')).toBe('14px');
    expect(document.documentElement.style.getPropertyValue('--openbitfun-font-size-meta')).toBe('11px');
    // typography-audit: negative-test-start -- verifies retired Appearance and FlowChat variables stay unwritten
    expect(document.documentElement.style.getPropertyValue('--openbitfun-appearance-token-font-size-base')).toBe('');
    expect(document.documentElement.style.getPropertyValue('--openbitfun-appearance-token-flowchat-font-size-base')).toBe('');
    // typography-audit: negative-test-end
  });

  it('drops retired FlowChat font settings while loading persisted preferences', async () => {
    configMocks.getConfig.mockResolvedValue({
      uiSize: { level: 'large' },
      flowChat: { mode: 'independent', basePx: 20 },
    });
    const service = new FontPreferenceService();

    await service.initialize();

    expect(service.getPreference()).toEqual({ uiSize: { level: 'large' } });
    expect(document.documentElement.style.getPropertyValue('--openbitfun-font-size-base')).toBe('16px');
    // typography-audit: negative-test-start -- verifies persisted legacy data cannot restore the retired variable
    expect(document.documentElement.style.getPropertyValue('--openbitfun-appearance-token-flowchat-font-size-base')).toBe('');
    // typography-audit: negative-test-end
  });

  it('adjusts a saved preset, applies typography, and persists the existing config shape', async () => {
    configMocks.getConfig.mockResolvedValue({ uiSize: { level: 'large' } });
    const service = new FontPreferenceService();
    await service.initialize();
    await service.adjustUiSize(1);
    expect(document.documentElement.style.getPropertyValue('--openbitfun-font-size-base')).toBe('17px');
    expect(configMocks.setConfig).toHaveBeenLastCalledWith('font', {
      uiSize: { level: 'custom', customPx: 17 },
    });
    await service.adjustUiSize(-1);
    expect(service.getPreference().uiSize.customPx).toBe(16);
  });

  it.each([[12, -1], [20, 1]] as const)('keeps the %ipx boundary without redundant writes', async (customPx, delta) => {
    const service = new FontPreferenceService();
    await service.setUiSize('custom', customPx);
    configMocks.setConfig.mockClear();
    await service.adjustUiSize(delta);
    expect(service.getPreference().uiSize.customPx).toBe(customPx);
    expect(configMocks.setConfig).not.toHaveBeenCalled();
  });

  it('uses the latest size for repeated key presses before persistence completes', async () => {
    configMocks.setConfig.mockImplementation(() => new Promise<void>(resolve => setTimeout(resolve, 0)));
    const service = new FontPreferenceService();
    await Promise.all([service.adjustUiSize(1), service.adjustUiSize(1), service.adjustUiSize(-1)]);
    expect(service.getPreference().uiSize.customPx).toBe(15);
  });
});
