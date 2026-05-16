import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { FileOperationToolCard } from './FileOperationToolCard';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  currentWorkspace: undefined as undefined | { rootPath: string },
  createDiffEditorTab: vi.fn(),
  openFile: vi.fn(),
  getOperationDiff: vi.fn(async () => ({
    originalContent: '',
    modifiedContent: '',
    anchorLine: undefined,
  })),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('../../component-library', () => ({
  CubeLoading: () => <span data-testid="cube-loading" />,
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('../../tools/snapshot_system/hooks/useSnapshotState', () => ({
  useSnapshotState: () => ({
    files: [],
    error: null,
    clearError: vi.fn(),
  }),
}));

vi.mock('../../tools/snapshot_system/core/SnapshotEventBus', () => ({
  SNAPSHOT_EVENTS: {
    FILE_OPERATION_COMPLETED: 'file-operation-completed',
  },
  SnapshotEventBus: {
    getInstance: () => ({
      emit: vi.fn(),
    }),
  },
}));

vi.mock('../components/CodePreview', () => ({
  CodePreview: ({ content }: { content: string }) => <pre>{content}</pre>,
}));

vi.mock('../components/InlineDiffPreview', () => ({
  InlineDiffPreview: ({ modifiedContent }: { modifiedContent: string }) => <pre>{modifiedContent}</pre>,
}));

vi.mock('../../shared/utils/tabUtils', () => ({
  createDiffEditorTab: mocks.createDiffEditorTab,
}));

vi.mock('../../shared/services/FileTabManager', () => ({
  fileTabManager: {
    openFile: mocks.openFile,
  },
}));

vi.mock('../../infrastructure/api', () => ({
  snapshotAPI: {
    getOperationDiff: mocks.getOperationDiff,
  },
}));

vi.mock('../../infrastructure/contexts/WorkspaceContext', () => ({
  useOptionalCurrentWorkspace: () => ({
    workspace: mocks.currentWorkspace,
  }),
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    info: vi.fn(),
  },
}));

vi.mock('@/tools/git/hooks/useGitState', () => ({
  useGitState: () => ({
    isRepository: false,
  }),
}));

describe('FileOperationToolCard', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);
    vi.stubGlobal('CustomEvent', dom.window.CustomEvent);

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);

    mocks.currentWorkspace = undefined;
    mocks.createDiffEditorTab.mockReset();
    mocks.openFile.mockReset();
    mocks.getOperationDiff.mockReset();
    mocks.getOperationDiff.mockResolvedValue({
      originalContent: '',
      modifiedContent: '',
      anchorLine: undefined,
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
  });

  it('renders failed write cards outside WorkspaceProvider', () => {
    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Write',
      status: 'error',
      toolCall: {
        id: 'call-1',
        name: 'Write',
        input: {
          file_path: 'src/newFile.ts',
          content: 'export const value = 1;',
        },
      },
      toolResult: {
        success: false,
        error: 'Arguments are invalid JSON.',
      },
    } as FlowToolItem;

    const config: ToolCardConfig = {
      toolName: 'Write',
      displayName: 'Write',
      icon: 'WRITE',
      requiresConfirmation: false,
      resultDisplayType: 'detailed',
      description: 'Write a file',
      displayMode: 'standard',
    };

    expect(() => {
      act(() => {
        root.render(
          <FileOperationToolCard
            toolItem={toolItem}
            config={config}
            sessionId="session-1"
          />
        );
      });
    }).not.toThrow();

    expect(container.textContent).toContain('toolCards.file.write');
    expect(container.textContent).toContain('toolCards.file.failedArguments are invalid JSON.');
  });

  it('opens completed write cards with the resolved result path', async () => {
    mocks.currentWorkspace = { rootPath: 'D:/workspace/project' };

    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Write',
      status: 'completed',
      toolCall: {
        id: 'call-1',
        name: 'Write',
        input: {
          file_path: 'newFile.ts',
          content: 'export const value = 1;',
        },
      },
      toolResult: {
        success: true,
        result: {
          file_path: 'D:/workspace/project/src/newFile.ts',
          bytes_written: 23,
          success: true,
        },
      },
    } as FlowToolItem;

    const config: ToolCardConfig = {
      toolName: 'Write',
      displayName: 'Write',
      icon: 'WRITE',
      requiresConfirmation: false,
      resultDisplayType: 'detailed',
      description: 'Write a file',
      displayMode: 'standard',
    };

    await act(async () => {
      root.render(
        <FileOperationToolCard
          toolItem={toolItem}
          config={config}
          sessionId="session-1"
        />
      );
    });

    const openButton = container.querySelector('.file-op-open-full-button') as HTMLButtonElement | null;
    expect(openButton).not.toBeNull();

    await act(async () => {
      openButton?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(mocks.getOperationDiff).toHaveBeenCalledWith(
      'session-1',
      'D:/workspace/project/src/newFile.ts',
      'call-1',
    );
    expect(mocks.openFile).toHaveBeenCalledWith(expect.objectContaining({
      filePath: 'D:/workspace/project/src/newFile.ts',
      fileName: 'newFile.ts',
      mode: 'agent',
    }));
  });

  it('opens a local diff for completed write cards without snapshot context', async () => {
    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Write',
      status: 'completed',
      toolCall: {
        id: 'call-1',
        name: 'Write',
        input: {
          filepath: 'src/newFile.ts',
          content: 'export const value = 1;\n',
        },
      },
      toolResult: {
        success: true,
        result: {
          success: true,
        },
      },
    } as FlowToolItem;

    const config: ToolCardConfig = {
      toolName: 'Write',
      displayName: 'Write',
      icon: 'WRITE',
      requiresConfirmation: false,
      resultDisplayType: 'detailed',
      description: 'Write a file',
      displayMode: 'standard',
    };

    await act(async () => {
      root.render(
        <FileOperationToolCard
          toolItem={toolItem}
          config={config}
        />
      );
    });

    const diffButton = container.querySelector('.file-op-diff-pill') as HTMLButtonElement | null;
    expect(diffButton).not.toBeNull();

    await act(async () => {
      diffButton?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    await act(async () => {
      dom.window.dispatchEvent(new dom.window.Event('tick'));
      await new Promise(resolve => dom.window.setTimeout(resolve, 260));
    });

    expect(mocks.getOperationDiff).not.toHaveBeenCalled();
    expect(mocks.createDiffEditorTab).toHaveBeenCalledWith(
      'src/newFile.ts',
      'newFile.ts',
      '',
      'export const value = 1;\n',
      false,
      'agent',
      undefined,
      undefined,
      false,
      {
        titleKind: 'diff',
        duplicateKeyPrefix: 'diff',
      },
    );
  });
});
