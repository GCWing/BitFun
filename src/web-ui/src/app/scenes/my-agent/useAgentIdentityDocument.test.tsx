// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  useAgentIdentityDocument,
  type UseAgentIdentityDocumentResult,
} from './useAgentIdentityDocument';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const apiMocks = vi.hoisted(() => ({
  readFileContent: vi.fn(),
  writeFileContent: vi.fn(),
}));
const watchFileChangesMock = vi.hoisted(() => vi.fn(() => vi.fn()));

vi.mock('@/infrastructure/api/service-api/WorkspaceAPI', () => ({
  workspaceAPI: {
    readFileContent: apiMocks.readFileContent,
    writeFileContent: apiMocks.writeFileContent,
  },
}));

vi.mock('@/tools/file-system/services/FileSystemService', () => ({
  fileSystemService: {
    watchFileChanges: watchFileChangesMock,
  },
}));

vi.mock('@/shared/services/ide-control', () => ({
  ideControl: {
    navigation: {
      goToFile: vi.fn(),
    },
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    error: vi.fn(),
  }),
}));

const INITIAL_IDENTITY = [
  '---',
  'name: Mira',
  'creature: Assistant',
  'vibe: Focused',
  'emoji: 💼',
  '---',
  '',
].join('\n');

describe('useAgentIdentityDocument autosave', () => {
  let container: HTMLDivElement;
  let root: Root;
  let latestResult: UseAgentIdentityDocumentResult | null;

  const Harness = () => {
    latestResult = useAgentIdentityDocument('/tmp/assistant');
    return null;
  };

  beforeEach(async () => {
    vi.useFakeTimers();
    apiMocks.readFileContent.mockReset();
    apiMocks.writeFileContent.mockReset();
    watchFileChangesMock.mockClear();
    apiMocks.readFileContent.mockResolvedValue(INITIAL_IDENTITY);
    apiMocks.writeFileContent
      .mockRejectedValueOnce(new Error('write failed'))
      .mockResolvedValue(undefined);

    latestResult = null;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    await act(async () => {
      root.render(<Harness />);
      await Promise.resolve();
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  it('stops retrying after a failed write until the user edits again', async () => {
    act(() => latestResult?.updateField('emoji', '🚀'));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });

    expect(apiMocks.writeFileContent).toHaveBeenCalledTimes(1);
    expect(latestResult?.saveStatus).toBe('error');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(4000);
    });
    expect(apiMocks.writeFileContent).toHaveBeenCalledTimes(1);

    act(() => latestResult?.updateField('emoji', '🧭'));
    expect(latestResult?.saveStatus).toBe('idle');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(800);
    });
    expect(apiMocks.writeFileContent).toHaveBeenCalledTimes(2);
    expect(latestResult?.saveStatus).toBe('saved');
  });
});
