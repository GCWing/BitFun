import { describe, expect, it } from 'vitest';
import { requiresPermissionConfirmation } from './miniAppCustomizationRisk';
import type { MiniAppPermissionDiff } from '@/infrastructure/api/service-api/MiniAppAPI';

describe('requiresPermissionConfirmation', () => {
  it('requires a second confirmation for high-risk permission changes', () => {
    const diff: MiniAppPermissionDiff = {
      high_risk: true,
      added: ['fs.write:{workspace}'],
      expanded: [],
      removed: [],
    };

    expect(requiresPermissionConfirmation(diff)).toBe(true);
  });

  it('does not require a second confirmation for removed-only changes', () => {
    const diff: MiniAppPermissionDiff = {
      high_risk: false,
      added: [],
      expanded: [],
      removed: ['net.allow:example.com'],
    };

    expect(requiresPermissionConfirmation(diff)).toBe(false);
  });
});
