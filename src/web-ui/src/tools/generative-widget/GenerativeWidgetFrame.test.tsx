/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import GenerativeWidgetFrame, { GENERATIVE_WIDGET_SHELL_HTML } from './GenerativeWidgetFrame';

vi.mock('@/infrastructure/theme', () => ({
  themeService: {
    on: vi.fn(() => vi.fn()),
  },
}));

describe('GenerativeWidgetFrame shell', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
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
    const values = [...GENERATIVE_WIDGET_SHELL_HTML.matchAll(/--font-size-sm:\s*([^;]+);/g)].map(
      (match) => match[1]?.trim()
    );

    expect(values).toEqual(['13px']);
  });

  it('writes the widget shell into about:blank instead of relying on srcdoc', async () => {
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
    expect(iframe.getAttribute('src')).toBe('about:blank');
    expect(iframe.getAttribute('srcdoc')).toBeNull();
    expect(iframe.getAttribute('sandbox')).toContain('allow-same-origin');
    expect(iframe.contentDocument?.documentElement.outerHTML).toContain('bitfun-widget');
  });
});
