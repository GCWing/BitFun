// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { FlowTextBlock } from './FlowTextBlock';
import type { FlowTextItem } from '../types/flow-chat';

const mocks = vi.hoisted(() => ({
  markdownRenderer: vi.fn(),
}));

vi.mock('@/infrastructure/markdown', () => ({
  MarkdownRenderer: (props: { content: string; isStreaming?: boolean }) => {
    mocks.markdownRenderer(props);
    return <div data-testid="markdown-renderer">{props.content}</div>;
  },
}));

vi.mock('./modern/FlowChatContext', () => ({
  useFlowChatContext: () => ({}),
}));

vi.mock('../deep-research/DeepResearchProtocolGroup', () => ({
  DeepResearchProtocolGroup: ({ kind }: { kind: string }) => (
    <div data-testid={`deep-research-${kind}`}>{kind}</div>
  ),
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

  it('renders Deep Research protocol markers as structured UI instead of markdown text', async () => {
    const textItem: FlowTextItem = {
      id: 'text-protocol',
      type: 'text',
      timestamp: 1,
      status: 'completed',
      content: '[[PHASE:phase-0-orient]]\n\nStarting research.',
      isStreaming: false,
      isMarkdown: true,
    };

    await act(async () => {
      root.render(<FlowTextBlock textItem={textItem} />);
      await Promise.resolve();
    });

    expect(container.querySelector('[data-testid="deep-research-phase"]')).not.toBeNull();
    expect(mocks.markdownRenderer).toHaveBeenCalledWith(expect.objectContaining({
      content: 'Starting research.',
    }));
    expect(mocks.markdownRenderer).not.toHaveBeenCalledWith(expect.objectContaining({
      content: expect.stringContaining('[[PHASE:'),
    }));
  });

  it('does not flash an incomplete Deep Research marker while streaming', async () => {
    const textItem: FlowTextItem = {
      id: 'text-streaming-protocol',
      type: 'text',
      timestamp: 1,
      status: 'streaming',
      content: '[[SUBQ:q1|Market',
      isStreaming: true,
      isMarkdown: true,
    };

    await act(async () => {
      root.render(<FlowTextBlock textItem={textItem} />);
      await Promise.resolve();
    });

    expect(container.textContent).not.toContain('[[');
    expect(mocks.markdownRenderer).not.toHaveBeenCalledWith(expect.objectContaining({
      content: expect.stringContaining('[['),
    }));
  });

});
