// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { FlowThinkingItem } from '../types/flow-chat';
import { ModelThinkingDisplay } from './ModelThinkingDisplay';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: { count?: number }) => ({
      'toolCards.think.thinking': 'Thinking...',
      'toolCards.think.thinkingProcess': 'Thinking Process',
      'toolCards.think.thinkingSummary': 'Thinking Summary',
      'toolCards.think.thinkingComplete': 'Thinking complete',
      'toolCards.think.thinkingCharacters': `Thought ${values?.count ?? 0} characters`,
    })[key] ?? key,
  }),
}));

vi.mock('../hooks/useTypewriter', () => ({
  useTypewriter: (content: string) => ({ displayText: content, isRevealing: false }),
}));

vi.mock('../hooks/typewriterRevealGateContext', () => ({
  useReportTypewriterReveal: () => {},
}));

vi.mock('./useToolCardHeightContract', () => ({
  useToolCardHeightContract: () => ({
    cardRootRef: { current: null },
    applyExpandedState: (
      current: boolean,
      next: boolean,
      setExpanded: (value: boolean) => void,
    ) => {
      if (current !== next) setExpanded(next);
    },
  }),
}));

vi.mock('@/infrastructure/markdown', () => ({
  MarkdownRenderer: ({ content }: { content: string }) => (
    <div data-testid="thinking-markdown">{content}</div>
  ),
}));

function summaryItem(content: string): FlowThinkingItem {
  return {
    id: 'summary-1',
    type: 'thinking',
    reasoningKind: 'summary',
    content,
    isStreaming: true,
    isCollapsed: false,
    timestamp: 1,
    status: 'streaming',
  };
}

describe('ModelThinkingDisplay reasoning summary', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean })
      .IS_REACT_ACT_ENVIRONMENT = true;
    vi.stubGlobal('ResizeObserver', class {
      observe() {}
      disconnect() {}
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  });

  it('defaults to a collapsed single-line preview of the latest summary part', async () => {
    await act(async () => {
      root.render(<ModelThinkingDisplay thinkingItem={summaryItem(
        '**Inspecting the stream**\n\n**Preparing the repair**',
      )} />);
    });

    const panel = container.querySelector('[data-testid="chat-thinking-panel"]');
    const label = container.querySelector('[data-bf-part="label"]');
    expect(panel?.getAttribute('data-expanded')).toBe('false');
    expect(label?.textContent).toBe('Preparing the repair');
    expect(label?.textContent).not.toContain('characters');
  });

  it('uses design-system thinking and disclosure icons in the header', async () => {
    await act(async () => {
      root.render(<ModelThinkingDisplay thinkingItem={summaryItem('**Inspecting**')} />);
    });

    const leadingIcon = container.querySelector('[data-bf-part="leadingIcon"]');
    expect(leadingIcon?.querySelector('[data-bf-name="thinking"]')).not.toBeNull();
    expect(leadingIcon?.querySelector('[data-bf-name="chevron-right"]')).not.toBeNull();
    expect(leadingIcon?.querySelector('[data-bf-name="chevron-down"]')).not.toBeNull();
  });

  it('replaces the collapsed preview when a new summary part arrives', async () => {
    await act(async () => {
      root.render(<ModelThinkingDisplay thinkingItem={summaryItem('**First part**')} />);
    });
    expect(container.querySelector('[data-bf-part="label"]')?.textContent).toBe('First part');

    await act(async () => {
      root.render(<ModelThinkingDisplay thinkingItem={summaryItem(
        '**First part**\n\n**Second part**',
      )} />);
    });
    expect(container.querySelector('[data-bf-part="label"]')?.textContent).toBe('Second part');
  });

  it('keeps user expansion and renders the complete summary Markdown', async () => {
    const content = '**First part**\n\n**Second part**';
    await act(async () => {
      root.render(<ModelThinkingDisplay thinkingItem={summaryItem(content)} />);
    });

    await act(async () => {
      (container.querySelector('[data-testid="chat-thinking-toggle"]') as HTMLElement).click();
    });
    expect(container.querySelector('[data-testid="chat-thinking-panel"]')
      ?.getAttribute('data-expanded')).toBe('true');
    expect(container.querySelector('[data-bf-part="label"]')?.textContent)
      .toBe('Thinking Summary');
    expect(container.querySelector('[data-testid="thinking-markdown"]')?.textContent)
      .toBe(content);

    await act(async () => {
      root.render(<ModelThinkingDisplay thinkingItem={summaryItem(
        `${content}\n\n**Third part**`,
      )} />);
    });
    expect(container.querySelector('[data-testid="chat-thinking-panel"]')
      ?.getAttribute('data-expanded')).toBe('true');
  });
});
