// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CopyableTextPreview } from './CopyableTextPreview';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', async () => {
  const actual = await vi.importActual<typeof import('react-i18next')>('react-i18next');
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) => ({
        'toolCards.common.copy': 'Copy',
        'toolCards.common.copied': 'Copied',
        'toolCards.common.copyFailed': 'Failed to copy',
      })[key] ?? key,
    }),
  };
});

vi.mock('@openbitfun/ui', () => ({
  Icon: ({ name }: { name: string }) => <span data-openbitfun-component="icon" data-openbitfun-name={name} />,
  Tooltip: ({ content, children }: { content: React.ReactNode; children: React.ReactElement }) => (
    <>
      {children}
      {content}
    </>
  ),
  IconButton: ({
    children,
    icon,
    tooltip: _tooltip,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & {
    icon?: React.ReactNode;
    tooltip?: string;
  }) => <button type="button" {...props}>{icon ?? children}</button>,
}));

describe('CopyableTextPreview', () => {
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
    vi.unstubAllGlobals();
  });

  it('copies the complete interactive tooltip text', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });
    const tooltipText = 'pnpm run type-check:web\npnpm run lint:web';

    await act(async () => {
      root.render(
        <CopyableTextPreview
          text="pnpm run type-check:web"
          emptyText="No command"
          tooltipContent={tooltipText}
        />,
      );
    });

    const copyButton = container.querySelector<HTMLButtonElement>('[aria-label="Copy"]');
    expect(copyButton).not.toBeNull();
    expect(container.querySelector('.copyable-text-preview-tooltip-content__text')?.textContent)
      .toBe(tooltipText);

    await act(async () => {
      copyButton?.click();
      await Promise.resolve();
    });

    expect(writeText).toHaveBeenCalledWith(tooltipText);
  });
});
