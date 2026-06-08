// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { Markdown } from './Markdown';

const mocks = vi.hoisted(() => ({
  getCurrentWorkspacePath: vi.fn(),
  revealInExplorer: vi.fn(),
  openExternal: vi.fn(),
}));

vi.mock('../../../infrastructure/api', () => ({
  globalAPI: {
    getCurrentWorkspacePath: (...args: unknown[]) => mocks.getCurrentWorkspacePath(...args),
  },
  workspaceAPI: {
    revealInExplorer: (...args: unknown[]) => mocks.revealInExplorer(...args),
    readFileContent: vi.fn(),
  },
  systemAPI: {
    openExternal: (...args: unknown[]) => mocks.openExternal(...args),
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
  }),
}));

vi.mock('@/infrastructure/theme', () => ({
  useTheme: () => ({ isLight: false }),
}));

vi.mock('../Tooltip', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('./MermaidBlock', () => ({
  MermaidBlock: () => <div data-testid="mermaid-block" />,
}));

vi.mock('./ReproductionStepsBlock', () => ({
  ReproductionStepsBlock: () => <div data-testid="reproduction-steps" />,
}));

vi.mock('./AsyncPrismSyntaxHighlighter', () => ({
  AsyncPrismSyntaxHighlighter: ({ children }: { children: React.ReactNode }) => <pre>{children}</pre>,
}));

vi.mock('@/shared/context-menu-system/core/ContextMenuController', () => ({
  contextMenuController: {
    show: vi.fn(),
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('@/shared/utils/startupTrace', () => ({
  isStartupRenderTraceEnabled: () => false,
  recordReactRenderProfile: vi.fn(),
  startupTrace: {},
}));

const EXAMPLE_WORKSPACE = 'C:\\ExampleWorkspace';
const EXAMPLE_ABSOLUTE_README = 'D:\\SampleDocs\\Guides\\README.md';

describe('Markdown file links', () => {
  let container: HTMLDivElement;
  let root: Root;
  let onFileViewRequest: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    onFileViewRequest = vi.fn();
    mocks.getCurrentWorkspacePath.mockReset();
    mocks.revealInExplorer.mockReset();
    mocks.openExternal.mockReset();
    mocks.getCurrentWorkspacePath.mockResolvedValue(EXAMPLE_WORKSPACE);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.clearAllMocks();
  });

  it('routes same-label relative, absolute, and computer links independently', async () => {
    const content = [
      '1. [README.md](.\\README.md)',
      `2. [README.md](${EXAMPLE_ABSOLUTE_README})`,
      '3. [README.md](computer://README.md)',
      `4. [README.md](computer://${EXAMPLE_ABSOLUTE_README})`,
      '5. [deck.pptx](computer://deck.pptx)',
    ].join('\n');

    await act(async () => {
      root.render(
        <Markdown
          content={content}
          onFileViewRequest={onFileViewRequest}
        />,
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    const buttons = Array.from(container.querySelectorAll<HTMLButtonElement>('button.file-link'));
    expect(buttons).toHaveLength(5);

    await act(async () => {
      buttons[0].click();
      await Promise.resolve();
    });

    expect(onFileViewRequest).toHaveBeenNthCalledWith(1, '.\\README.md', 'README.md', undefined);
    expect(mocks.revealInExplorer).not.toHaveBeenCalled();

    await act(async () => {
      buttons[1].click();
      await Promise.resolve();
    });

    expect(onFileViewRequest).toHaveBeenNthCalledWith(2, EXAMPLE_ABSOLUTE_README, 'README.md', undefined);
    expect(mocks.revealInExplorer).not.toHaveBeenCalled();

    await act(async () => {
      buttons[2].click();
      await Promise.resolve();
    });

    expect(onFileViewRequest).toHaveBeenNthCalledWith(3, 'README.md', 'README.md', undefined);
    expect(mocks.revealInExplorer).not.toHaveBeenCalled();

    await act(async () => {
      buttons[3].click();
      await Promise.resolve();
    });

    expect(onFileViewRequest).toHaveBeenNthCalledWith(4, EXAMPLE_ABSOLUTE_README, 'README.md', undefined);
    expect(mocks.revealInExplorer).not.toHaveBeenCalled();

    await act(async () => {
      buttons[4].click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.revealInExplorer).toHaveBeenNthCalledWith(1, `${EXAMPLE_WORKSPACE}\\deck.pptx`);
    expect(onFileViewRequest).toHaveBeenCalledTimes(4);
  });
});
