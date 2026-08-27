// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  SceneChromeContribution,
  SceneChromeHost,
  SceneChromeProvider,
} from './SceneChrome';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('SceneChrome', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('mounts only the active scene contribution in the registered host', () => {
    const renderScene = (activeSceneId: 'session' | 'settings') => {
      root.render(
        <SceneChromeProvider activeSceneId={activeSceneId}>
          <SceneChromeHost data-testid="scene-chrome-host" />
          <SceneChromeContribution sceneId="session">
            <button type="button" data-testid="session-action">Session action</button>
          </SceneChromeContribution>
          <SceneChromeContribution sceneId="settings">
            <button type="button" data-testid="settings-action">Settings action</button>
          </SceneChromeContribution>
        </SceneChromeProvider>,
      );
    };

    act(() => renderScene('session'));
    const host = container.querySelector('[data-testid="scene-chrome-host"]');
    expect(host?.querySelector('[data-testid="session-action"]')).not.toBeNull();
    expect(host?.querySelector('[data-testid="settings-action"]')).toBeNull();

    act(() => renderScene('settings'));
    expect(host?.querySelector('[data-testid="session-action"]')).toBeNull();
    expect(host?.querySelector('[data-testid="settings-action"]')).not.toBeNull();
  });
});
