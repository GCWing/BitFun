import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { FileOperationToolCard } from './FileOperationToolCard';
import { FlowChatContext } from '../components/modern/FlowChatContext';
import type { FlowToolItem, Session, ToolCardConfig } from '../types/flow-chat';
import {
  clearHistorySessionOpenTransition,
  clearRecentHistorySessionOpenIntent,
  dispatchHistorySessionOpenIntent,
} from '../services/sessionOpenIntent';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  currentWorkspace: undefined as undefined | { rootPath: string; connectionId?: string },
  createDiffEditorTab: vi.fn(),
  openFile: vi.fn(),
  codePreviewProps: [] as Array<Record<string, unknown>>,
  inlineDiffPreviewProps: [] as Array<Record<string, unknown>>,
  getOperationDiff: vi.fn(async () => ({
    originalContent: '',
    modifiedContent: '',
    anchorLine: undefined,
  })),
  useGitState: vi.fn(() => ({
    isRepository: false,
  })),
  typewriterMode: 'passthrough' as 'passthrough' | 'partial',
  writePlanDisplayProps: [] as Array<Record<string, unknown>>,
}));

vi.mock('./WritePlanDisplay', () => ({
  WritePlanDisplay: (props: Record<string, unknown>) => {
    mocks.writePlanDisplayProps.push(props);
    return <div data-testid="write-plan-display" />;
  },
}));

vi.mock('../hooks/useTypewriter', () => ({
  useTypewriter: (targetText: string, animate: boolean) => {
    if (mocks.typewriterMode === 'partial' && animate) {
      return {
        displayText: targetText.slice(0, Math.max(0, Math.floor(targetText.length / 2))),
        isRevealing: true,
      };
    }
    return {
      displayText: targetText,
      isRevealing: false,
    };
  },
}));

vi.mock('../hooks/typewriterRevealGateContext', () => ({
  useReportTypewriterReveal: vi.fn(),
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
  CubeLoading: () => <span data-testid="cube-loading" />,
  IconButton: ({ children, onClick }: { children: React.ReactNode; onClick?: React.MouseEventHandler }) => (
    <button type="button" onClick={onClick}>{children}</button>
  ),
  ToolProcessingDots: () => <span data-testid="tool-processing-dots" />,
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
  CodePreview: (props: Record<string, unknown>) => {
    mocks.codePreviewProps.push(props);
    return <pre>{String(props.content ?? '')}</pre>;
  },
}));

vi.mock('../components/InlineDiffPreview', () => ({
  InlineDiffPreview: (props: Record<string, unknown>) => {
    mocks.inlineDiffPreviewProps.push(props);
    return <pre>{String(props.modifiedContent ?? '')}</pre>;
  },
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
    getOperationSummary: vi.fn(async () => null),
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
  useGitState: mocks.useGitState,
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
    vi.stubGlobal('ResizeObserver', class {
      observe() {}
      unobserve() {}
      disconnect() {}
    });

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);

    mocks.currentWorkspace = undefined;
    mocks.createDiffEditorTab.mockReset();
    mocks.openFile.mockReset();
    mocks.codePreviewProps = [];
    mocks.inlineDiffPreviewProps = [];
    mocks.typewriterMode = 'passthrough';
    mocks.writePlanDisplayProps = [];
    mocks.useGitState.mockClear();
    mocks.useGitState.mockReturnValue({
      isRepository: false,
    });
    mocks.getOperationDiff.mockReset();
    mocks.getOperationDiff.mockResolvedValue({
      originalContent: '',
      modifiedContent: '',
      anchorLine: undefined,
    });
  });

  it('routes only successful Write calls for .plan.md files to the plan display', async () => {
    mocks.currentWorkspace = {
      rootPath: 'D:/workspace/project',
      connectionId: 'remote-1',
    };
    const content = '---\nname: Plan\noverview: Test the plan card.\ntodos: []\n---\n\n# Plan\n\nDetails.';
    const toolItem: FlowToolItem = {
      id: 'tool-plan',
      type: 'tool',
      toolName: 'Write',
      status: 'completed',
      toolCall: {
        id: 'call-plan',
        name: 'Write',
        input: {
          payload: `+++ .bitfun/plans/test.plan.md\n${content}`,
        },
      },
      toolResult: {
        success: true,
        result: { file_path: '.bitfun/plans/test.plan.md' },
      },
    } as FlowToolItem;

    await act(async () => {
      root.render(
        <FileOperationToolCard
          toolItem={toolItem}
          config={{} as ToolCardConfig}
          sessionId="session-1"
        />,
      );
    });

    expect(container.querySelector('[data-testid="write-plan-display"]')).not.toBeNull();
    expect(mocks.writePlanDisplayProps).toHaveLength(1);
    expect(mocks.writePlanDisplayProps[0]).toMatchObject({
      planFilePath: 'D:/workspace/project/.bitfun/plans/test.plan.md',
      initialContent: content,
      workspacePath: 'D:/workspace/project',
      remoteConnectionId: 'remote-1',
    });
  });

  it('keeps Edit calls for .plan.md files on the normal file operation card', async () => {
    const toolItem: FlowToolItem = {
      id: 'tool-plan-edit',
      type: 'tool',
      toolName: 'Edit',
      status: 'completed',
      toolCall: {
        id: 'call-plan-edit',
        name: 'Edit',
        input: {
          file_path: '.bitfun/plans/test.plan.md',
          old_string: 'Old',
          new_string: 'New',
        },
      },
      toolResult: {
        success: true,
        result: { file_path: '.bitfun/plans/test.plan.md' },
      },
    } as FlowToolItem;

    await act(async () => {
      root.render(
        <FileOperationToolCard
          toolItem={toolItem}
          config={{} as ToolCardConfig}
          sessionId="session-1"
        />,
      );
    });

    expect(container.querySelector('[data-testid="write-plan-display"]')).toBeNull();
    expect(container.querySelector('[data-testid="chat-file-change-card"]')).not.toBeNull();
  });

  it('keeps failed .plan.md writes on the normal error card', async () => {
    const toolItem: FlowToolItem = {
      id: 'tool-plan-error',
      type: 'tool',
      toolName: 'Write',
      status: 'error',
      toolCall: {
        id: 'call-plan-error',
        name: 'Write',
        input: {
          payload: '+++ .bitfun/plans/test.plan.md\npartial',
        },
      },
      toolResult: {
        success: false,
        error: 'Write failed',
      },
    } as FlowToolItem;

    await act(async () => {
      root.render(
        <FileOperationToolCard
          toolItem={toolItem}
          config={{} as ToolCardConfig}
          sessionId="session-1"
        />,
      );
    });

    expect(container.querySelector('[data-testid="write-plan-display"]')).toBeNull();
    expect(container.querySelector('[data-testid="chat-file-change-card"]')).not.toBeNull();
  });

  it('does not treat a successful fallback write as a completed plan', async () => {
    const toolItem: FlowToolItem = {
      id: 'tool-plan-fallback',
      type: 'tool',
      toolName: 'Write',
      status: 'completed',
      toolCall: {
        id: 'call-plan-fallback',
        name: 'Write',
        input: {
          payload: '+++ .bitfun/plans/test.plan.md\npartial',
        },
      },
      toolResult: {
        success: true,
        result: {
          file_path: '.bitfun/tmp/write-callplanfallback.txt',
          used_fallback_path: true,
        },
      },
    } as FlowToolItem;

    await act(async () => {
      root.render(
        <FileOperationToolCard
          toolItem={toolItem}
          config={{} as ToolCardConfig}
          sessionId="session-1"
        />,
      );
    });

    expect(container.querySelector('[data-testid="write-plan-display"]')).toBeNull();
    expect(container.querySelector('[data-testid="chat-file-change-card"]')).not.toBeNull();
  });

  it('does not trigger passive git refresh while historical restore is pending', async () => {
    mocks.currentWorkspace = { rootPath: 'D:/workspace/BitFun' };
    const toolItem: FlowToolItem = {
      id: 'tool-history',
      type: 'tool',
      toolName: 'Write',
      status: 'completed',
      toolCall: {
        id: 'call-history',
        name: 'Write',
        input: {
          file_path: 'src/newFile.ts',
          content: 'export const value = 1;',
        },
      },
      toolResult: {
        success: true,
        result: {
          file_path: 'src/newFile.ts',
        },
      },
    } as FlowToolItem;

    await act(async () => {
      root.render(
        <FlowChatContext.Provider
          value={{
            sessionId: 'history-session',
            activeSessionOverride: {
              sessionId: 'history-session',
              isHistorical: true,
              contextRestoreState: 'pending',
            } as Session,
          }}
        >
          <FileOperationToolCard
            toolItem={toolItem}
            config={{} as ToolCardConfig}
            sessionId="history-session"
          />
        </FlowChatContext.Provider>
      );
    });

    expect(mocks.useGitState).toHaveBeenCalledWith(expect.objectContaining({
      repositoryPath: 'D:/workspace/BitFun',
      isActive: false,
      refreshOnMount: false,
      refreshOnActive: false,
      participateInWindowFocusRefresh: false,
      layers: ['basic'],
    }));
  });

  it('keeps passive git refresh enabled for normal active sessions', async () => {
    mocks.currentWorkspace = { rootPath: 'D:/workspace/BitFun' };
    const toolItem: FlowToolItem = {
      id: 'tool-active',
      type: 'tool',
      toolName: 'Write',
      status: 'completed',
      toolCall: {
        id: 'call-active',
        name: 'Write',
        input: {
          file_path: 'src/newFile.ts',
          content: 'export const value = 1;',
        },
      },
      toolResult: {
        success: true,
        result: {
          file_path: 'src/newFile.ts',
        },
      },
    } as FlowToolItem;

    await act(async () => {
      root.render(
        <FlowChatContext.Provider
          value={{
            sessionId: 'active-session',
            activeSessionOverride: {
              sessionId: 'active-session',
              isHistorical: false,
            } as Session,
          }}
        >
          <FileOperationToolCard
            toolItem={toolItem}
            config={{} as ToolCardConfig}
            sessionId="active-session"
          />
        </FlowChatContext.Provider>
      );
    });

    expect(mocks.useGitState).toHaveBeenCalledWith(expect.objectContaining({
      repositoryPath: 'D:/workspace/BitFun',
      isActive: true,
      refreshOnMount: true,
      refreshOnActive: false,
    }));
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    clearRecentHistorySessionOpenIntent();
    clearHistorySessionOpenTransition();
    vi.unstubAllGlobals();
  });

  it('does not trigger passive git refresh during history open transition', async () => {
    mocks.currentWorkspace = { rootPath: 'D:/workspace/BitFun' };
    dispatchHistorySessionOpenIntent('history-session', 'History');
    const toolItem: FlowToolItem = {
      id: 'tool-transition',
      type: 'tool',
      toolName: 'Write',
      status: 'completed',
      toolCall: {
        id: 'call-transition',
        name: 'Write',
        input: {
          file_path: 'src/newFile.ts',
          content: 'export const value = 1;',
        },
      },
      toolResult: {
        success: true,
        result: {
          file_path: 'src/newFile.ts',
        },
      },
    } as FlowToolItem;

    await act(async () => {
      root.render(
        <FlowChatContext.Provider
          value={{
            sessionId: 'history-session',
            activeSessionOverride: {
              sessionId: 'history-session',
              isHistorical: true,
              contextRestoreState: 'ready',
            } as Session,
          }}
        >
          <FileOperationToolCard
            toolItem={toolItem}
            config={{} as ToolCardConfig}
            sessionId="history-session"
          />
        </FlowChatContext.Provider>
      );
    });

    expect(mocks.useGitState).toHaveBeenCalledWith(expect.objectContaining({
      repositoryPath: 'D:/workspace/BitFun',
      isActive: false,
      refreshOnMount: false,
      refreshOnActive: false,
    }));
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

  it('renders completed ACP file cards from result locations when input has no path', async () => {
    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Write',
      status: 'completed',
      toolCall: {
        id: 'call-1',
        name: 'Write',
        input: {
          title: 'Run Write',
        },
      },
      toolResult: {
        success: true,
        result: {
          content: [],
          locations: [
            {
              path: 'src/from-acp-location.ts',
            },
          ],
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

    expect(container.textContent).toContain('from-acp-location.ts');
    expect(container.textContent).not.toContain('toolCards.file.parsingPath');
  });

  it('renders write guardrail blocks as guidance instead of hard failure', async () => {
    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Write',
      status: 'error',
      toolCall: {
        id: 'call-1',
        name: 'Write',
        input: {
          file_path: 'docs/report.md',
        },
      },
      toolResult: {
        success: false,
        error:
          '[guidance] Use Read to load the current contents of docs/report.md before calling Write on it.',
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

    expect(container.textContent).toContain('toolCards.file.guidanceHint');
    expect(container.textContent).not.toContain('toolCards.file.failed');
    expect(container.textContent).toContain(
      'Use Read to load the current contents of docs/report.md before calling Write on it.',
    );
    expect(container.querySelector('.file-operation-card--guidance')).not.toBeNull();
  });

  it('renders edit guardrail blocks as guidance instead of hard failure', async () => {
    const toolItem: FlowToolItem = {
      id: 'tool-2',
      type: 'tool',
      toolName: 'Edit',
      status: 'error',
      toolCall: {
        id: 'call-2',
        name: 'Edit',
        input: {
          file_path: 'src/main.rs',
          old_string: 'foo',
          new_string: 'bar',
        },
      },
      toolResult: {
        success: false,
        error:
          '[guidance] Use Read to load the current contents of src/main.rs before calling Edit on it.',
      },
    } as FlowToolItem;

    const config: ToolCardConfig = {
      toolName: 'Edit',
      displayName: 'Edit',
      icon: 'EDIT',
      requiresConfirmation: false,
      resultDisplayType: 'detailed',
      description: 'Edit a file',
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

    expect(container.textContent).toContain('toolCards.file.guidanceHint');
    expect(container.textContent).not.toContain('toolCards.file.failed');
    expect(container.textContent).toContain(
      'Use Read to load the current contents of src/main.rs before calling Edit on it.',
    );
    expect(container.querySelector('.file-operation-card--guidance')).not.toBeNull();
  });

  it('shows receiving content label while write content streams before file_path', async () => {
    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Write',
      status: 'receiving',
      isParamsStreaming: true,
      toolCall: {
        id: 'call-1',
        name: 'Write',
        input: {
          content: 'const value = 1;',
        },
      },
      partialParams: {
        content: 'const value = 1;',
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

    expect(container.textContent).toContain('toolCards.file.receivingContent');
    expect(container.textContent).not.toContain('toolCards.file.parsingPath');
  });

  it('disables nested code-preview autoscroll while write content is streaming', async () => {
    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Write',
      status: 'streaming',
      isParamsStreaming: true,
      toolCall: {
        id: 'call-1',
        name: 'Write',
        input: {
          file_path: 'src/generated.ts',
          content: 'const value = 1;\nconst value2 = 2;',
        },
      },
      partialParams: {
        file_path: 'src/generated.ts',
        content: 'const value = 1;\nconst value2 = 2;',
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

    expect(mocks.codePreviewProps).toHaveLength(1);
    expect(mocks.codePreviewProps[0]).toMatchObject({
      isStreaming: true,
      autoScrollToBottom: false,
    });
  });

  it('applies typewriter reveal to write streaming content preview', async () => {
    mocks.typewriterMode = 'partial';
    const fullContent = 'const value = 1;\nconst value2 = 2;\nconst value3 = 3;';
    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Write',
      status: 'streaming',
      isParamsStreaming: true,
      toolCall: {
        id: 'call-1',
        name: 'Write',
        input: {
          file_path: 'src/generated.ts',
          content: fullContent,
        },
      },
      partialParams: {
        file_path: 'src/generated.ts',
        content: fullContent,
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

    expect(mocks.codePreviewProps).toHaveLength(1);
    const previewContent = String(mocks.codePreviewProps[0].content ?? '');
    expect(previewContent.length).toBeGreaterThan(0);
    expect(previewContent.length).toBeLessThan(fullContent.length);
    expect(mocks.codePreviewProps[0]).toMatchObject({
      isStreaming: true,
      autoScrollToBottom: false,
    });
    // Status still reflects received bytes, not only revealed characters.
    expect(container.textContent).toContain(`${fullContent.length} chars received`);
  });

  it('retains a compact completed write preview until newer content supersedes it', async () => {
    const config: ToolCardConfig = {
      toolName: 'Write',
      displayName: 'Write',
      icon: 'WRITE',
      requiresConfirmation: false,
      resultDisplayType: 'detailed',
      description: 'Write a file',
      displayMode: 'standard',
    };
    const streamingToolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Write',
      status: 'streaming',
      isParamsStreaming: true,
      toolCall: {
        id: 'call-1',
        name: 'Write',
        input: {
          file_path: 'src/generated.ts',
          content: 'line 1\nline 2\nline 3\nline 4\nline 5\nline 6',
        },
      },
      partialParams: {
        file_path: 'src/generated.ts',
        content: 'line 1\nline 2\nline 3\nline 4\nline 5\nline 6',
      },
    } as FlowToolItem;
    const completedToolItem: FlowToolItem = {
      ...streamingToolItem,
      status: 'completed',
      isParamsStreaming: false,
      toolResult: {
        success: true,
        result: {
          file_path: 'src/generated.ts',
        },
      },
    } as FlowToolItem;

    await act(async () => {
      root.render(
        <FileOperationToolCard
          toolItem={streamingToolItem}
          config={config}
          sessionId="session-1"
          isLastItem
        />
      );
    });

    mocks.codePreviewProps = [];
    mocks.inlineDiffPreviewProps = [];

    await act(async () => {
      root.render(
        <FileOperationToolCard
          toolItem={completedToolItem}
          config={config}
          sessionId="session-1"
          isLastItem
        />
      );
    });

    expect(container.querySelector('[data-testid="chat-file-change-preview"]')).not.toBeNull();
    expect(mocks.codePreviewProps).toHaveLength(2);
    expect(mocks.codePreviewProps.at(-1)).toMatchObject({
      isStreaming: false,
      maxHeight: 88,
    });
    expect(mocks.inlineDiffPreviewProps).toHaveLength(0);

    await act(async () => {
      root.render(
        <FileOperationToolCard
          toolItem={completedToolItem}
          config={config}
          sessionId="session-1"
          isLastItem={false}
        />
      );
    });

    // Auto-collapse animates closed; wait for SmoothHeightCollapse to unmount children.
    expect(container.querySelector('[data-testid="chat-file-change-card"]')?.getAttribute('data-expanded')).toBe('false');
    expect(mocks.inlineDiffPreviewProps).toHaveLength(0);
    await act(async () => {
      await new Promise((resolve) => {
        window.setTimeout(resolve, 350);
      });
    });
    expect(container.querySelector('[data-testid="chat-file-change-preview"]')).toBeNull();
  });

  it('uses the larger diff preview height after a completed write card is manually expanded', async () => {
    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Write',
      status: 'completed',
      isParamsStreaming: false,
      toolCall: {
        id: 'call-1',
        name: 'Write',
        input: {
          file_path: 'src/generated.ts',
          content: 'line 1\nline 2\nline 3\nline 4\nline 5\nline 6',
        },
      },
      toolResult: {
        success: true,
        result: {
          file_path: 'src/generated.ts',
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

    mocks.inlineDiffPreviewProps = [];

    const card = container.querySelector('.base-tool-card') as HTMLDivElement | null;
    await act(async () => {
      card?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(mocks.inlineDiffPreviewProps.map(props => props.maxHeight)).toContain(330);
  });
});
