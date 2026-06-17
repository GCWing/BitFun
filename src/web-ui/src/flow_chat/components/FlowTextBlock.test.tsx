// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { FlowTextBlock } from './FlowTextBlock';
import type { FlowTextItem } from '../types/flow-chat';

const mocks = vi.hoisted(() => ({
  markdownRenderer: vi.fn(),
}));

vi.mock('@/component-library', () => ({
  MarkdownRenderer: (props: { content: string; isStreaming?: boolean }) => {
    mocks.markdownRenderer(props);
    return <div data-testid="markdown-renderer">{props.content}</div>;
  },
  DotMatrixLoader: () => <div data-testid="dot-matrix-loader" />,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: () => [],
  }),
}));

vi.mock('./modern/FlowChatContext', () => ({
  useFlowChatContext: () => ({}),
}));

describe('FlowTextBlock', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    vi.useFakeTimers();

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.markdownRenderer.mockReset();
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('does not re-render completed markdown just to settle streaming growth state', async () => {
    const textItem: FlowTextItem = {
      id: 'text-1',
      type: 'text',
      timestamp: 1,
      status: 'completed',
      content: 'Completed historical markdown',
      isStreaming: false,
      isMarkdown: true,
    };

    await act(async () => {
      root.render(<FlowTextBlock textItem={textItem} />);
      await Promise.resolve();
    });

    expect(mocks.markdownRenderer).toHaveBeenCalledTimes(1);
  });
});
