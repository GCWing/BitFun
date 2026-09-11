import { beforeEach, describe, expect, it, vi } from 'vitest';

const configMocks = vi.hoisted(() => ({
  current: {} as Record<string, unknown>,
  getConfig: vi.fn(),
  updateConfig: vi.fn(),
  watch: vi.fn(),
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: {
    getConfig: configMocks.getConfig,
    updateConfig: configMocks.updateConfig,
    watch: configMocks.watch,
  },
}));

import {
  chatInputModePreferenceService,
  normalizeChatInputModePreference,
  resolveConfiguredChatInputDefaultModeId,
} from './ChatInputModePreferenceService';

describe('ChatInputModePreferenceService', () => {
  beforeEach(() => {
    configMocks.current = {};
    configMocks.getConfig.mockReset().mockImplementation(async () => configMocks.current);
    configMocks.updateConfig.mockReset().mockImplementation(async (_path, update) => {
      configMocks.current = update(configMocks.current);
      return configMocks.current;
    });
    configMocks.watch.mockReset().mockReturnValue(() => undefined);
  });

  it('adopts follow-last for untouched config and falls back to Standard without history', () => {
    const preference = normalizeChatInputModePreference(undefined);

    expect(preference).toEqual({
      strategy: 'follow_last',
      fixedModeId: null,
      lastModeId: null,
    });
    expect(resolveConfiguredChatInputDefaultModeId(preference)).toBeNull();
  });

  it('preserves the historical fixed meaning of default_mode_id when strategy is absent', () => {
    const preference = normalizeChatInputModePreference({
      default_mode_id: ' PlannerPlus ',
    });

    expect(preference.strategy).toBe('fixed');
    expect(resolveConfiguredChatInputDefaultModeId(preference)).toBe('PlannerPlus');
  });

  it('uses only the remembered selection in follow-last mode', () => {
    const preference = normalizeChatInputModePreference({
      default_mode_strategy: 'follow_last',
      default_mode_id: 'Ultimate',
      last_mode_id: ' creative ',
    });

    expect(preference).toEqual({
      strategy: 'follow_last',
      fixedModeId: 'Ultimate',
      lastModeId: 'Creative',
    });
    expect(resolveConfiguredChatInputDefaultModeId(preference)).toBe('Creative');
  });

  it('keeps fixed-policy compatibility while dropping ineligible stored mode ids', () => {
    const preference = normalizeChatInputModePreference({
      default_mode_id: 'Claw',
      last_mode_id: 'Plan',
    });

    expect(preference).toEqual({
      strategy: 'fixed',
      fixedModeId: null,
      lastModeId: null,
    });
    expect(resolveConfiguredChatInputDefaultModeId(preference)).toBeNull();
  });

  it('seeds fixed mode from the remembered selection and keeps unrelated fields', async () => {
    configMocks.current = {
      last_mode_id: 'Ultimate',
      show_permission_mode_control: false,
    };

    await expect(chatInputModePreferenceService.setStrategy('fixed')).resolves.toEqual({
      strategy: 'fixed',
      fixedModeId: 'Ultimate',
      lastModeId: 'Ultimate',
    });
    expect(configMocks.current).toEqual({
      default_mode_strategy: 'fixed',
      default_mode_id: 'Ultimate',
      last_mode_id: 'Ultimate',
      show_permission_mode_control: false,
    });
  });

  it('remembers a successful selection without changing a fixed default', async () => {
    configMocks.current = {
      default_mode_strategy: 'fixed',
      default_mode_id: 'Standard',
    };

    const preference = await chatInputModePreferenceService.rememberMode('Creative');

    expect(preference).toEqual({
      strategy: 'fixed',
      fixedModeId: 'Standard',
      lastModeId: 'Creative',
    });
    expect(resolveConfiguredChatInputDefaultModeId(preference)).toBe('Standard');
  });

  it.each(['Claw', 'Plan', 'Multitask'])('does not remember fixed or retired mode %s', async modeId => {
    configMocks.current = { last_mode_id: 'minimal' };

    await chatInputModePreferenceService.rememberMode(modeId);

    expect(configMocks.updateConfig).not.toHaveBeenCalled();
    expect(configMocks.getConfig).toHaveBeenCalledOnce();
  });
});
