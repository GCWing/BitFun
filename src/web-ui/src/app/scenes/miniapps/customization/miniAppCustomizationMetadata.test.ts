import { describe, expect, it } from 'vitest';
import type { MiniAppCustomizationMetadata } from '@/infrastructure/api/service-api/MiniAppAPI';
import { getMiniAppBuiltinUpdateNotice } from './miniAppCustomizationMetadata';

const baseMetadata: MiniAppCustomizationMetadata = {
  origin: {
    kind: 'builtin',
    builtin_id: 'builtin-gomoku',
    builtin_version: 11,
  },
  local_override: true,
  last_applied_draft_id: 'draft-1',
  available_builtin_update: {
    builtin_version: 12,
    detected_at: 1710000000000,
  },
  updated_at: 1710000000000,
};

describe('getMiniAppBuiltinUpdateNotice', () => {
  it('returns the available bundled version for customized built-in apps', () => {
    expect(getMiniAppBuiltinUpdateNotice(baseMetadata)).toEqual({
      builtinVersion: 12,
    });
  });

  it('does not show an update notice for apps without local overrides', () => {
    expect(getMiniAppBuiltinUpdateNotice({
      ...baseMetadata,
      local_override: false,
    })).toBeNull();
  });

  it('does not show an update notice without an available bundled update', () => {
    expect(getMiniAppBuiltinUpdateNotice({
      ...baseMetadata,
      available_builtin_update: undefined,
    })).toBeNull();
  });
});
