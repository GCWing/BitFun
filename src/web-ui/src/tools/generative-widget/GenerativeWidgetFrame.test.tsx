/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import GenerativeWidgetFrame, { GENERATIVE_WIDGET_SHELL_HTML } from './GenerativeWidgetFrame';
import { widgetAppearanceAdapter } from '@/infrastructure/appearance/adapters/WidgetAppearanceAdapter';

describe('GenerativeWidgetFrame shell', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    widgetAppearanceAdapter.apply(
      { id: 'test.widget', mode: 'dark', vars: {} },
      undefined,
      { revision: 1, appearanceId: 'test', mode: 'dark', globals: {}, assets: {} },
    );
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.clearAllMocks();
  });

  it('keeps iframe-local small text aligned with the host default token', () => {
    const values = [...GENERATIVE_WIDGET_SHELL_HTML.matchAll(/--bf-appearance-token-font-size-sm:\s*([^;]+);/g)].map(
      (match) => match[1]?.trim()
    );

    expect(values).toEqual(['13px']);
  });

  // document.write leaves the frame without a normal document load, and WebKit can
  // drop such a frame's scroll node when the host window is hidden and shown again.
  // srcdoc keeps the same-origin semantics the widget message boundary relies on
  // while still going through the document loading path.
  it('renders the widget shell through srcdoc rather than writing into about:blank', async () => {
    await act(async () => {
      root.render(
        <GenerativeWidgetFrame
          widgetId="widget_1"
          title="Widget"
          widgetCode="<svg viewBox='0 0 10 10'><circle cx='5' cy='5' r='4' /></svg>"
        />,
      );
    });

    await act(async () => {
      await new Promise(resolve => window.setTimeout(resolve, 0));
    });

    const iframe = container.querySelector('iframe') as HTMLIFrameElement;
    expect(iframe).toBeTruthy();
    expect(iframe.getAttribute('src')).toBeNull();
    expect(iframe.getAttribute('srcdoc')).toContain('bitfun-widget');
    expect(iframe.getAttribute('sandbox')).toContain('allow-same-origin');
  });
});
