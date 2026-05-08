import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { FileOperationToolCard } from './FileOperationToolCard';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

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
  createDiffEditorTab: vi.fn(),
}));

vi.mock('../../shared/services/FileTabManager', () => ({
  fileTabManager: {
    openFile: vi.fn(),
  },
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
});
