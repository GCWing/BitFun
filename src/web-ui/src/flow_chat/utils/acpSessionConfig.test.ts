import { describe, expect, it } from 'vitest';

import type { AcpSessionConfigOption } from '@/infrastructure/api/service-api/ACPClientAPI';
import {
  buildAcpFastModeValue,
  getAcpModelProviderName,
  resolveAcpReasoningState,
  resolveAcpFastModeState,
} from './acpSessionConfig';

describe('ACP Fast mode config', () => {
  it('resolves and toggles the select fallback exposed by Codex ACP', () => {
    const option: AcpSessionConfigOption = {
      id: 'fast-mode',
      name: 'Fast mode',
      type: 'select',
      currentValue: 'off',
      options: [
        { value: 'off', name: 'Off' },
        { value: 'on', name: 'On' },
      ],
    };

    expect(resolveAcpFastModeState([option])).toEqual({ option, enabled: false });
    expect(buildAcpFastModeValue(option, true)).toEqual({ type: 'select', value: 'on' });
  });

  it('supports ACP boolean config options', () => {
    const option: AcpSessionConfigOption = {
      id: 'fast-mode',
      name: 'Fast mode',
      type: 'boolean',
      currentValue: true,
    };

    expect(resolveAcpFastModeState([option])).toEqual({ option, enabled: true });
    expect(buildAcpFastModeValue(option, false)).toEqual({ type: 'boolean', value: false });
  });

  it('ignores unrelated or malformed options', () => {
    const malformed: AcpSessionConfigOption = {
      id: 'fast-mode',
      name: 'Fast mode',
      type: 'select',
      currentValue: 'standard',
      options: [{ value: 'standard', name: 'Standard' }],
    };

    expect(resolveAcpFastModeState([])).toBeNull();
    expect(resolveAcpFastModeState([malformed])).toBeNull();
    expect(buildAcpFastModeValue(malformed, true)).toBeNull();
  });
});

describe('ACP reasoning config', () => {
  it('projects the standard thought-level select without inferring provider behavior', () => {
    const option: AcpSessionConfigOption = {
      id: 'reasoning-effort',
      name: 'Reasoning effort',
      category: 'thought_level',
      type: 'select',
      currentValue: 'high',
      options: [
        { value: 'low', name: 'Low' },
        { value: 'high', name: 'High' },
      ],
    };

    const state = resolveAcpReasoningState([option]);
    expect(state?.option).toBe(option);
    expect(state?.selectedPreset).toBe('high');
    expect(state?.projection.presets?.map(preset => preset.id)).toEqual(['low', 'high']);
  });

  it('fails closed for uncategorized and malformed options', () => {
    expect(resolveAcpReasoningState([{
      id: 'reasoning-effort',
      name: 'Reasoning effort',
      type: 'select',
      currentValue: 'high',
      options: [{ value: 'high', name: 'High' }],
    }])).toBeNull();
  });
});

describe('ACP model provider display', () => {
  it('uses the explicit provider name when the ACP bridge includes one', () => {
    expect(getAcpModelProviderName({
      id: 'openai/gpt-5.4',
      name: 'gpt-5.4',
      providerName: 'OpenAI',
    })).toBe('OpenAI');
  });

  it('falls back to provider-qualified description or id', () => {
    expect(getAcpModelProviderName({
      id: 'openai/gpt-5.4',
      name: 'gpt-5.4',
    })).toBe('openai');

    expect(getAcpModelProviderName({
      id: 'gpt-5.4',
      name: 'gpt-5.4',
      description: 'azure-openai/gpt-5.4',
    })).toBe('azure-openai');
  });

  it('ignores unqualified descriptions', () => {
    expect(getAcpModelProviderName({
      id: 'gpt-5.4',
      name: 'gpt-5.4',
      description: 'Fast reasoning model',
    })).toBeUndefined();
  });
});
