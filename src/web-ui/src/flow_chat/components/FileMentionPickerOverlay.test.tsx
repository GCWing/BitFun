// @vitest-environment jsdom

import React, { act, useRef } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { workspaceAPI } from '@/infrastructure/api';
import { FileMentionPicker } from './FileMentionPicker';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/infrastructure/api', () => ({
  sessionAPI: {
    searchReferenceableSessions: vi.fn().mockResolvedValue([]),
  },
  workspaceAPI: {
    getDirectoryChildren: vi.fn().mockResolvedValue([]),
    searchFilenamesOnlyStreamDetailed: vi.fn().mockResolvedValue({
      searchId: 'search-1',
      searchKind: 'filenames',
      limit: 30,
      truncated: false,
      totalResults: 0,
    }),
  },
}));

vi.mock('@/infrastructure/api/service-api/ExternalSourcesAPI', () => ({
  externalSourcesAPI: {
    getWorkspaceReferences: vi.fn().mockResolvedValue({ references: [] }),
  },
}));

const Harness: React.FC<{
  searchQuery?: string;
  remoteConnectionId?: string;
}> = ({ searchQuery = '', remoteConnectionId = 'remote-connection-1' }) => {
  const anchorRef = useRef<HTMLButtonElement>(null);
  return (
    <div>
      <button ref={anchorRef} type="button">anchor</button>
      <FileMentionPicker
        isOpen
        searchQuery={searchQuery}
        workspacePath="/workspace"
        remoteConnectionId={remoteConnectionId}
        anchorRef={anchorRef}
        onSelect={vi.fn()}
        onClose={vi.fn()}
      />
    </div>
  );
};

describe('FileMentionPicker overlay', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    Element.prototype.scrollIntoView = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    vi.useRealTimers();
    act(() => root.unmount());
    document.querySelector('[data-bf-overlay-host="true"]')?.remove();
    container.remove();
    vi.clearAllMocks();
  });

  it('renders above clipped composers through the appearance overlay host', async () => {
    await act(async () => {
      root.render(<Harness />);
      await Promise.resolve();
    });

    const picker = document.querySelector<HTMLElement>('.file-mention-picker--overlay');
    expect(picker?.parentElement?.getAttribute('data-bf-overlay-host')).toBe('true');
    expect(picker?.style.visibility).toBe('visible');
    expect(workspaceAPI.getDirectoryChildren).toHaveBeenCalledWith(
      '/workspace',
      'remote-connection-1',
    );
  });

  it('shows the current directory name before its parent path', async () => {
    vi.mocked(workspaceAPI.getDirectoryChildren).mockResolvedValueOnce([
      {
        path: '/workspace/src',
        name: 'src',
        isDirectory: true,
      },
    ]);

    await act(async () => {
      root.render(<Harness />);
      await Promise.resolve();
    });

    const item = document.querySelector<HTMLElement>('[data-bf-part="option"]');
    expect(document.querySelector('[data-bf-part="currentDirectoryName"]')?.textContent).toBe('workspace');
    expect(document.querySelector('[data-bf-part="parentDirectoryPath"]')).toBeNull();
    expect(item?.querySelector('[data-bf-part="label"]')?.textContent).toBe('src');
    expect(item?.querySelector('[data-bf-part="metadata"]')).toBeNull();

    vi.mocked(workspaceAPI.getDirectoryChildren).mockResolvedValueOnce([
      {
        path: '/workspace/src/App.tsx',
        name: 'App.tsx',
        isDirectory: false,
      },
    ]);

    await act(async () => {
      item?.click();
      await Promise.resolve();
    });

    expect(document.querySelector('[data-bf-part="currentDirectoryName"]')?.textContent).toBe('src');
    expect(document.querySelector('[data-bf-part="parentDirectoryPath"]')?.textContent).toBe('workspace');
    const nestedItem = document.querySelector('[data-bf-part="option"]');
    expect(nestedItem?.querySelector('[data-bf-part="label"]')?.textContent).toBe('App.tsx');
    expect(nestedItem?.querySelector('[data-bf-part="metadata"]')).toBeNull();
  });

  it('does not present a remote browse failure as an empty directory', async () => {
    vi.mocked(workspaceAPI.getDirectoryChildren).mockRejectedValueOnce(
      new Error('remote connection unavailable'),
    );

    await act(async () => {
      root.render(<Harness />);
      await Promise.resolve();
    });

    expect(document.querySelector('[data-bf-part="empty"][data-bf-state~="error"]')?.textContent)
      .toBe('fileMention.browseUnavailable');
    expect(document.querySelector('[data-bf-part="empty"]:not([data-bf-state~="error"])')).toBeNull();
  });

  it('shows streamed remote matches before the recursive search completes', async () => {
    vi.useFakeTimers();
    let reportProgress: ((event: {
      searchId: string;
      searchKind: 'filenames';
      results: Array<{
        path: string;
        name: string;
        isDirectory: boolean;
        fileNameMatch: {
          path: string;
          name: string;
          isDirectory: boolean;
          matchType: 'fileName';
        };
        contentMatches: [];
      }>;
    }) => void) | undefined;
    let finishSearch: (() => void) | undefined;

    vi.mocked(workspaceAPI.searchFilenamesOnlyStreamDetailed).mockImplementationOnce((
      _rootPath,
      _pattern,
      _caseSensitive,
      _useRegex,
      _wholeWord,
      _searchIdOrSignal,
      _maxResults,
      _includeDirectories,
      callbacks,
    ) => {
      reportProgress = callbacks.onProgress;
      return new Promise(resolve => {
        finishSearch = () => resolve({
          searchId: 'search-remote-1',
          searchKind: 'filenames',
          limit: 30,
          truncated: false,
          totalResults: 1,
        });
      });
    });

    await act(async () => {
      root.render(<Harness searchQuery="手写" />);
      await Promise.resolve();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });

    expect(workspaceAPI.searchFilenamesOnlyStreamDetailed).toHaveBeenCalled();
    expect(
      vi.mocked(workspaceAPI.searchFilenamesOnlyStreamDetailed).mock.calls[0]?.[10],
    ).toBe('remote-connection-1');

    await act(async () => {
      reportProgress?.({
        searchId: 'search-remote-1',
        searchKind: 'filenames',
        results: [{
          path: '/workspace/手写笔画标注项目',
          name: '手写笔画标注项目',
          isDirectory: true,
          fileNameMatch: {
            path: '/workspace/手写笔画标注项目',
            name: '手写笔画标注项目',
            isDirectory: true,
            matchType: 'fileName',
          },
          contentMatches: [],
        }],
      });
      await Promise.resolve();
    });

    expect(document.querySelector('[data-bf-part="option"] [data-bf-part="label"]')?.textContent)
      .toBe('手写笔画标注项目');
    expect(document.querySelector('[data-bf-part="root"]')?.getAttribute('data-bf-state'))
      .toBe('loading');

    await act(async () => {
      finishSearch?.();
      await Promise.resolve();
    });
    vi.useRealTimers();
  });
});
