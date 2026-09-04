// @vitest-environment jsdom
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it, vi } from 'vitest';
import SceneTopBar from './SceneTopBar';

const sceneState = vi.hoisted(() => ({ openTabs: [{}] as unknown[] }));
const stylesheet = readFileSync(
  resolve(process.cwd(), 'src/app/components/SceneTopBar/SceneTopBar.scss'),
  'utf8',
);

vi.mock('@/app/components/WindowControls', () => ({ WindowControls: () => <button>Window controls</button> }));
vi.mock('@/infrastructure/runtime', () => ({ supportsNativeWindowDragging: () => false }));
vi.mock('../../stores/sceneStore', () => ({ useSceneStore: (selector: (state: typeof sceneState) => unknown) => selector(sceneState) }));
vi.mock('../SceneBar/SceneBar', () => ({ default: () => <div role="tablist"><button role="tab">Settings</button></div> }));
vi.mock('./SceneChrome', () => ({ SceneChromeHost: (props: React.HTMLAttributes<HTMLDivElement>) => <div {...props}><button>Scene action</button></div> }));

describe('SceneTopBar', () => {
  it('extends the Toolbar divider through both side gaps on the same pixel row', () => {
    expect(stylesheet).not.toContain('border-block-end: 0;');
    expect(stylesheet).toContain(
      'inset-block-end: calc(0px - var(--openbitfun-border-width-default));',
    );
    expect(stylesheet).toContain('width: var(--openbitfun-space-4);');
    expect(stylesheet).toContain('inset-inline-end: 100%;');
    expect(stylesheet).toContain('inset-inline-start: 100%;');
  });

  it('composes the public Toolbar without changing scene actions or window interaction boundaries', () => {
    sceneState.openTabs = [{}];
    const host = document.createElement('div');
    document.body.append(host);
    const root = createRoot(host);
    const maximize = vi.fn();
    try {
      act(() => root.render(<SceneTopBar onMinimize={vi.fn()} onMaximize={maximize} onClose={vi.fn()} />));
      const toolbar = host.querySelector('[data-openbitfun-component="toolbar"]')!;
      expect(toolbar.getAttribute('data-openbitfun-part')).toBe('topBar');
      expect(toolbar.getAttribute('data-size')).toBe('md');
      expect(toolbar.getAttribute('data-bordered')).toBe('true');
      expect(toolbar.querySelector(':scope > [data-openbitfun-part="leading"] [role="tablist"]')).not.toBeNull();
      expect(toolbar.querySelector(':scope > [data-openbitfun-part="trailing"] [data-openbitfun-part="sceneActions"]')).not.toBeNull();
      expect(toolbar.querySelector('[data-openbitfun-part="controls"] button')).not.toBeNull();
      act(() => toolbar.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })));
      expect(maximize).toHaveBeenCalledOnce();
      act(() => toolbar.querySelector('[data-openbitfun-part="sceneActions"] button')!.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })));
      expect(maximize).toHaveBeenCalledOnce();
    } finally {
      act(() => root.unmount());
      host.remove();
    }
  });

  it('removes the divider when no scene tabs are open', () => {
    sceneState.openTabs = [];
    const host = document.createElement('div');
    document.body.append(host);
    const root = createRoot(host);
    try {
      act(() => root.render(<SceneTopBar />));
      expect(host.querySelector('[data-openbitfun-component="toolbar"]')?.getAttribute('data-bordered'))
        .toBe('false');
    } finally {
      act(() => root.unmount());
      host.remove();
      sceneState.openTabs = [{}];
    }
  });
});
