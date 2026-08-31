// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it, vi } from 'vitest';
import SceneTopBar from './SceneTopBar';

vi.mock('@/app/components/WindowControls', () => ({ WindowControls: () => <button>Window controls</button> }));
vi.mock('@/infrastructure/runtime', () => ({ supportsNativeWindowDragging: () => false }));
vi.mock('../../stores/sceneStore', () => ({ useSceneStore: (selector: (state: unknown) => unknown) => selector({ openTabs: [{}] }) }));
vi.mock('../SceneBar/SceneBar', () => ({ default: () => <div role="tablist"><button role="tab">Settings</button></div> }));
vi.mock('./SceneChrome', () => ({ SceneChromeHost: (props: React.HTMLAttributes<HTMLDivElement>) => <div {...props}><button>Scene action</button></div> }));

describe('SceneTopBar', () => {
  it('composes the public Toolbar without changing scene actions or window interaction boundaries', () => {
    const host = document.createElement('div');
    document.body.append(host);
    const root = createRoot(host);
    const maximize = vi.fn();
    try {
      act(() => root.render(<SceneTopBar onMinimize={vi.fn()} onMaximize={maximize} onClose={vi.fn()} />));
      const toolbar = host.querySelector('[data-bf-component="toolbar"]')!;
      expect(toolbar.getAttribute('data-bf-part')).toBe('topBar');
      expect(toolbar.querySelector(':scope > [data-bf-part="leading"] [role="tablist"]')).not.toBeNull();
      expect(toolbar.querySelector(':scope > [data-bf-part="trailing"] [data-bf-part="sceneActions"]')).not.toBeNull();
      expect(toolbar.querySelector('[data-bf-part="controls"] button')).not.toBeNull();
      act(() => toolbar.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })));
      expect(maximize).toHaveBeenCalledOnce();
      act(() => toolbar.querySelector('[data-bf-part="sceneActions"] button')!.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })));
      expect(maximize).toHaveBeenCalledOnce();
    } finally {
      act(() => root.unmount());
      host.remove();
    }
  });
});
