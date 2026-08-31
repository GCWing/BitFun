// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_RETAINED_MOUNT_MS } from '@/shared/presence';
import { BranchSelectModal } from './BranchSelectModal';

const mocks = vi.hoisted(() => ({
  getBranches: vi.fn(),
}));

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
  i18nService: { t: (key: string) => key },
}));

vi.mock('../../../infrastructure/api/service-api/GitAPI', () => ({
  gitAPI: { getBranches: mocks.getBranches },
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('BranchSelectModal presence', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    mocks.getBranches.mockResolvedValue([]);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.querySelectorAll('.branch-select-overlay').forEach((node) => node.remove());
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  const renderModal = (isOpen: boolean, title: string) => {
    act(() => {
      root.render(
        <BranchSelectModal
          isOpen={isOpen}
          onClose={vi.fn()}
          onSelect={vi.fn()}
          repositoryPath=""
          title={title}
        />,
      );
    });
  };

  it('retains the closed surface and last title until the exit window completes', () => {
    renderModal(true, 'Choose release branch');
    expect(document.querySelector('.branch-select-overlay')?.getAttribute('data-state')).toBe('open');

    renderModal(false, 'Changed after close');
    expect(document.querySelector('.branch-select-overlay')?.getAttribute('data-state')).toBe('closed');
    expect(document.querySelector('.branch-select-dialog__title')?.textContent).toBe('Choose release branch');

    act(() => vi.advanceTimersByTime(DEFAULT_RETAINED_MOUNT_MS - 1));
    expect(document.querySelector('.branch-select-overlay')).not.toBeNull();

    act(() => vi.advanceTimersByTime(1));
    expect(document.querySelector('.branch-select-overlay')).toBeNull();
  });
});
