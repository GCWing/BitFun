import { describe, expect, it } from 'vitest';
import { groupLogViewAppearanceDescriptor } from './GroupLogView.appearance';

describe('group log view appearance contract', () => {
  it('paints the read-only log as a continuous session workspace surface', () => {
    expect(groupLogViewAppearanceDescriptor.parts).toContainEqual({
      id: 'root',
      propertyProfile: 'layout',
      visualRole: 'continuous-surface',
      continuityGroup: 'session-workspace',
    });
  });

  it('declares no toolbar/input part (read-only view has no composer surface)', () => {
    expect(groupLogViewAppearanceDescriptor.parts.some(p => p.id === 'input')).toBe(false);
  });
});
