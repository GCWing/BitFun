import { SessionIcon } from '@bitfun/ui';
import { describe, expect, it } from 'vitest';
import { SCENE_TAB_REGISTRY, getSceneDef } from './registry';

describe('scene tab icon registry', () => {
  it('does not register a default welcome tab', () => {
    expect(SCENE_TAB_REGISTRY.map(scene => scene.id)).not.toContain('welcome');
    expect(SCENE_TAB_REGISTRY.some(scene => scene.defaultOpen)).toBe(false);
  });

  it('uses the design-system SessionIcon only for the session tab', () => {
    expect(getSceneDef('session')?.Icon).toBe(SessionIcon);
    expect(
      SCENE_TAB_REGISTRY
        .filter(scene => scene.Icon === SessionIcon)
        .map(scene => scene.id),
    ).toEqual(['session']);
  });
});
