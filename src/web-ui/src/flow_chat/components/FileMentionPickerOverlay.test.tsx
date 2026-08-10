// @vitest-environment jsdom

import React, { act, useRef } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FileMentionPicker } from './FileMentionPicker';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/api', () => ({
  sessionAPI: {
    searchReferenceableSessions: vi.fn().mockResolvedValue([]),
  },
  workspaceAPI: {
    getDirectoryChildren: vi.fn().mockResolvedValue([]),
    searchFilenamesOnly: vi.fn().mockResolvedValue([]),
  },
}));

vi.mock('@/infrastructure/api/service-api/ExternalSourcesAPI', () => ({
  externalSourcesAPI: {
    getWorkspaceReferences: vi.fn().mockResolvedValue({ references: [] }),
  },
}));

const Harness: React.FC = () => {
  const anchorRef = useRef<HTMLButtonElement>(null);
  return (
    <div>
      <button ref={anchorRef} type="button">anchor</button>
      <FileMentionPicker
        isOpen
        searchQuery=""
        workspacePath="/workspace"
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
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
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
  });
});
