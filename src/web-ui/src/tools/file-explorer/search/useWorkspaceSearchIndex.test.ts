// @vitest-environment jsdom
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it, vi } from 'vitest';
import { workspaceAPI } from '@/infrastructure/api';
import { useWorkspaceSearchIndex } from './useWorkspaceSearchIndex';

vi.mock('@/infrastructure/api', () => ({
  workspaceAPI: {
    getSearchRepoStatus: vi.fn(),
    buildSearchIndex: vi.fn(),
    rebuildSearchIndex: vi.fn(),
  },
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('temporarily suspended workspace search', () => {
  it.each(['/local/repo', '/remote/repo'])('ignores an existing enabled preference for %s', async (workspacePath) => {
    const host = document.createElement('div');
    const root = createRoot(host);
    let result: ReturnType<typeof useWorkspaceSearchIndex>;
    function Probe() {
      result = useWorkspaceSearchIndex({ workspacePath, enabled: true });
      return null;
    }
    await act(async () => root.render(createElement(Probe)));
    try {
      expect(result!.supported).toBe(false);
      await act(async () => {
        expect(await result!.refreshStatus()).toBeNull();
        expect(await result!.buildIndex()).toBeNull();
        expect(await result!.rebuildIndex()).toBeNull();
      });
      expect(workspaceAPI.getSearchRepoStatus).not.toHaveBeenCalled();
      expect(workspaceAPI.buildSearchIndex).not.toHaveBeenCalled();
      expect(workspaceAPI.rebuildSearchIndex).not.toHaveBeenCalled();
    } finally {
      await act(async () => root.unmount());
    }
  });
});
