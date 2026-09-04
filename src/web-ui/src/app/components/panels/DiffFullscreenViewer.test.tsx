// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_RETAINED_MOUNT_MS } from '@/shared/presence';
import { DiffFullscreenViewer } from './DiffFullscreenViewer';

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
  i18nService: { t: (key: string) => key },
}));

vi.mock('../../../tools/editor', () => ({
  DiffEditor: ({ filePath }: { filePath: string }) => (
    <div data-testid="retained-diff-path">{filePath}</div>
  ),
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('DiffFullscreenViewer presence', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.querySelectorAll('.diff-fullscreen-overlay').forEach((node) => node.remove());
    document.body.style.overflow = '';
    vi.useRealTimers();
  });

  const renderViewer = (isOpen: boolean, filePath: string) => {
    act(() => {
      root.render(
        <DiffFullscreenViewer
          isOpen={isOpen}
          onClose={vi.fn()}
          filePath={filePath}
          originalContent="before"
          modifiedContent="after"
          onAcceptFile={vi.fn()}
          onRejectFile={vi.fn()}
          onAcceptBlock={vi.fn()}
          onRejectBlock={vi.fn()}
        />,
      );
    });
  };

  it('keeps the last diff stable during exit before unmounting it', () => {
    renderViewer(true, '/repo/old.ts');
    expect(document.querySelector('.diff-fullscreen-overlay')?.getAttribute('data-state')).toBe('open');
    expect(document.querySelector('button[aria-label="tooltip.close"]')?.getAttribute('data-openbitfun-component')).toBe('icon-button');

    renderViewer(false, '/repo/new.ts');
    expect(document.querySelector('.diff-fullscreen-overlay')?.getAttribute('data-state')).toBe('closed');
    expect(document.querySelector('[data-testid="retained-diff-path"]')?.textContent).toBe('/repo/old.ts');

    act(() => vi.advanceTimersByTime(DEFAULT_RETAINED_MOUNT_MS));
    expect(document.querySelector('.diff-fullscreen-overlay')).toBeNull();
  });
});
