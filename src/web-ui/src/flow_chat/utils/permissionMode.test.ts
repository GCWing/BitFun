import { describe, expect, it } from 'vitest';
import {
  chatInputPermissionMode,
  permissionModeFromConfig,
  permissionModeToConfig,
  sessionPermissionMode,
} from './permissionMode';
import type { ToolPermissionConfig } from '@/infrastructure/config/types';

function config(
  preset: ToolPermissionConfig['policy']['preset'],
  autoApproveAsk: boolean,
): ToolPermissionConfig {
  return {
    policy: { preset, rules: [] },
    interaction: { auto_approve_ask: autoApproveAsk },
  };
}

describe('permissionMode', () => {
  it('maps between the control naming and the backend naming', () => {
    expect(chatInputPermissionMode('ask')).toBe('ask');
    expect(chatInputPermissionMode('auto_approve')).toBe('auto');
    expect(chatInputPermissionMode('full_access')).toBe('full_access');

    expect(sessionPermissionMode('ask')).toBe('ask');
    expect(sessionPermissionMode('auto')).toBe('auto_approve');
    expect(sessionPermissionMode('full_access')).toBe('full_access');
  });

  it('round trips every mode through the control naming', () => {
    for (const mode of ['ask', 'auto_approve', 'full_access'] as const) {
      expect(sessionPermissionMode(chatInputPermissionMode(mode))).toBe(mode);
    }
  });

  it('derives the mode a stored configuration represents', () => {
    expect(permissionModeFromConfig(config('ask', false))).toBe('ask');
    expect(permissionModeFromConfig(config('ask', true))).toBe('auto_approve');
    // Full access already resolves every ask, so it outranks auto approval.
    expect(permissionModeFromConfig(config('full_access', true))).toBe('full_access');
  });

  it('projects a mode back onto both configuration knobs', () => {
    const base = config('ask', true);

    expect(permissionModeToConfig(base, 'ask')).toMatchObject({
      policy: { preset: 'ask' },
      interaction: { auto_approve_ask: false },
    });
    expect(permissionModeToConfig(base, 'auto_approve')).toMatchObject({
      policy: { preset: 'ask' },
      interaction: { auto_approve_ask: true },
    });
    expect(permissionModeToConfig(base, 'full_access')).toMatchObject({
      policy: { preset: 'full_access' },
      interaction: { auto_approve_ask: false },
    });
  });

  it('round trips a configuration through the mode it represents', () => {
    for (const stored of [config('ask', false), config('ask', true), config('full_access', false)]) {
      const mode = permissionModeFromConfig(stored);
      expect(permissionModeFromConfig(permissionModeToConfig(stored, mode))).toBe(mode);
    }
  });
});
