import { describe, expect, it } from 'vitest';

import type { AcpSessionConfigOption } from '@/infrastructure/api/service-api/ACPClientAPI';
import { buildAcpFastModeValue, resolveAcpFastModeState } from './acpSessionConfig';

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
