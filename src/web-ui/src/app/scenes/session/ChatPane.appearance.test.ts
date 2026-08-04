import { describe, expect, it } from 'vitest';
import { chatPaneAppearanceDescriptor } from './ChatPane.appearance';

describe('chat pane appearance contract', () => {
  it('exposes the real painted chat surface as a continuous workspace owner', () => {
    expect(chatPaneAppearanceDescriptor.parts).toContainEqual({
      id: 'root',
      propertyProfile: 'layout',
      visualRole: 'continuous-surface',
      continuityGroup: 'session-workspace',
    });
  });
});
