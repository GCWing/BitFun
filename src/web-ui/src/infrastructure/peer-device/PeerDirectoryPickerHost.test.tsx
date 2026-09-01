// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_RETAINED_MOUNT_MS } from '@/shared/presence';
import { PeerDirectoryPickerHost } from './PeerDirectoryPickerHost';
import { usePeerDirectoryPickerStore } from './peerDirectoryPickerStore';

vi.mock('./PeerDirectoryBrowser', async () => {
  const ReactModule = await import('react');
  return {
    PeerDirectoryBrowser: ({
      visible,
      title,
      initialPath,
    }: {
      visible: boolean;
      title: string;
      initialPath?: string;
    }) => {
      const [mountedPath] = ReactModule.useState(initialPath);
      return ReactModule.createElement('div', {
        'data-testid': 'peer-directory-browser-stub',
        'data-state': visible ? 'open' : 'closed',
        'data-title': title,
        'data-initial-path': initialPath,
        'data-mounted-path': mountedPath,
      });
    },
  };
});

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('PeerDirectoryPickerHost presence', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    usePeerDirectoryPickerStore.setState({
      isOpen: false,
      title: '',
      defaultPath: undefined,
      resolve: null,
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    act(() => root.render(<PeerDirectoryPickerHost />));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    usePeerDirectoryPickerStore.setState({
      isOpen: false,
      title: '',
      defaultPath: undefined,
      resolve: null,
    });
    vi.useRealTimers();
  });

  it('retains the last picker inputs while closing', () => {
    act(() => {
      void usePeerDirectoryPickerStore.getState().show({
        title: 'Choose peer folder',
        defaultPath: '/peer/project',
      });
    });

    const openBrowser = container.querySelector('[data-testid="peer-directory-browser-stub"]');
    expect(openBrowser?.getAttribute('data-state')).toBe('open');

    act(() => usePeerDirectoryPickerStore.getState().cancel());
    const closingBrowser = container.querySelector('[data-testid="peer-directory-browser-stub"]');
    expect(closingBrowser?.getAttribute('data-state')).toBe('closed');
    expect(closingBrowser?.getAttribute('data-title')).toBe('Choose peer folder');
    expect(closingBrowser?.getAttribute('data-initial-path')).toBe('/peer/project');

    act(() => vi.advanceTimersByTime(DEFAULT_RETAINED_MOUNT_MS));
    expect(container.querySelector('[data-testid="peer-directory-browser-stub"]')).toBeNull();
  });

  it('remounts a rapid replacement request with its own initial path', () => {
    act(() => {
      void usePeerDirectoryPickerStore.getState().show({
        title: 'First request',
        defaultPath: '/peer/first',
      });
    });
    act(() => usePeerDirectoryPickerStore.getState().cancel());

    act(() => {
      void usePeerDirectoryPickerStore.getState().show({
        title: 'Second request',
        defaultPath: '/peer/second',
      });
    });

    const replacement = container.querySelector('[data-testid="peer-directory-browser-stub"]');
    expect(replacement?.getAttribute('data-state')).toBe('open');
    expect(replacement?.getAttribute('data-title')).toBe('Second request');
    expect(replacement?.getAttribute('data-mounted-path')).toBe('/peer/second');
  });
});
