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
});
