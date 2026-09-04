import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { SCENE_TAB_REGISTRY, getSceneDef } from './registry';

describe('scene tab icon registry', () => {
  it('does not register a default welcome tab', () => {
    expect(SCENE_TAB_REGISTRY.map(scene => scene.id)).not.toContain('welcome');
    expect(SCENE_TAB_REGISTRY.some(scene => scene.defaultOpen)).toBe(false);
  });

  it('uses the shared catalog glyph for the session tab', () => {
    const SceneIcon = getSceneDef('session')!.Icon!;
    const markup = renderToStaticMarkup(createElement(SceneIcon));
    expect(markup).toContain('data-openbitfun-component="icon"');
    expect(markup).toContain('data-openbitfun-name="session"');
  });

  it('uses a gear rather than the control-center glyph for Settings', () => {
    const SceneIcon = getSceneDef('settings')!.Icon!;
    const markup = renderToStaticMarkup(createElement(SceneIcon));
    expect(markup).toContain('data-openbitfun-name="gear"');
    expect(markup).not.toContain('data-openbitfun-name="settings"');
  });
});
