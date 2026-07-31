// @vitest-environment jsdom

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FlowChatTurnRail, type FlowChatTurnRailItem } from './FlowChatTurnRail';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { current?: number }) => {
      if (key === 'flowChatTurnRail.label') return 'Turns';
      if (key === 'flowChatTurnRail.untitledTurn') return 'Untitled turn';
      if (key === 'flowChatHeader.turnBadge') return `Turn ${options?.current ?? 0}`;
      return key;
    },
  }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({
    children,
    content,
  }: {
    children: React.ReactElement;
    content: React.ReactNode;
  }) => (
    <span data-testid="tooltip-wrapper">
      {children}
      <span data-testid="tooltip-content">{content}</span>
    </span>
  ),
}));

const turns: FlowChatTurnRailItem[] = [
  { turnId: 'turn-1', turnIndex: 1, content: 'First user message' },
  { turnId: 'turn-2', turnIndex: 2, content: 'Second user message' },
  { turnId: 'turn-3', turnIndex: 3, content: 'Third user message' },
  { turnId: 'turn-4', turnIndex: 4, content: 'Fourth user message' },
];

describe('FlowChatTurnRail', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('renders every turn and highlights all viewport turns consistently', () => {
    act(() => {
      root.render(
        <FlowChatTurnRail
          turns={turns}
          currentTurnId="turn-2"
          visibleTurnIds={['turn-2', 'turn-3']}
          onNavigate={vi.fn()}
        />,
      );
    });

    const items = container.querySelectorAll<HTMLButtonElement>('.flowchat-turn-rail__item');
    expect(items).toHaveLength(4);
    expect(items[1].getAttribute('aria-current')).toBe('step');
    expect(items[1].className).toContain('flowchat-turn-rail__item--visible');
    expect(items[2].className).toContain('flowchat-turn-rail__item--visible');
    expect(items[1].className).toBe(items[2].className);
    expect(items[0].getAttribute('aria-current')).toBeNull();
    expect(items[0].className).not.toContain('flowchat-turn-rail__item--visible');
  });

  it('shows the turn number and user message in the tooltip without a timestamp', () => {
    act(() => {
      root.render(
        <FlowChatTurnRail
          turns={turns.slice(0, 1)}
          currentTurnId="turn-1"
          visibleTurnIds={['turn-1']}
          onNavigate={vi.fn()}
        />,
      );
    });

    const tooltip = container.querySelector('[data-testid="tooltip-content"]');
    expect(tooltip?.textContent).toContain('Turn 1');
    expect(tooltip?.textContent).toContain('First user message');
    expect(tooltip?.querySelector('.flowchat-turn-rail__tooltip-time')).toBeNull();
  });

  it('delegates clicks to the shared turn navigation callback', () => {
    const onNavigate = vi.fn();
    act(() => {
      root.render(
        <FlowChatTurnRail
          turns={turns}
          currentTurnId="turn-1"
          visibleTurnIds={['turn-1']}
          onNavigate={onNavigate}
        />,
      );
    });

    act(() => {
      container.querySelector<HTMLButtonElement>('[data-turn-id="turn-3"]')?.click();
    });

    expect(onNavigate).toHaveBeenCalledOnce();
    expect(onNavigate).toHaveBeenCalledWith('turn-3');
  });

  it('keeps the active turn visible by scrolling only the rail list', () => {
    act(() => {
      root.render(
        <FlowChatTurnRail
          turns={turns}
          currentTurnId="turn-1"
          visibleTurnIds={['turn-1']}
          onNavigate={vi.fn()}
        />,
      );
    });

    const list = container.querySelector<HTMLElement>('.flowchat-turn-rail__list');
    const target = container.querySelector<HTMLElement>('[data-turn-id="turn-4"]');
    expect(list).not.toBeNull();
    expect(target).not.toBeNull();
    if (!list || !target) return;

    Object.defineProperty(list, 'clientHeight', { configurable: true, value: 40 });
    Object.defineProperty(target, 'offsetTop', { configurable: true, value: 60 });
    Object.defineProperty(target, 'offsetHeight', { configurable: true, value: 20 });
    list.scrollTop = 0;

    act(() => {
      root.render(
        <FlowChatTurnRail
          turns={turns}
          currentTurnId="turn-4"
          visibleTurnIds={['turn-4']}
          onNavigate={vi.fn()}
        />,
      );
    });

    expect(list.scrollTop).toBe(40);
  });

  it('moves keyboard focus through the vertical turn list', () => {
    act(() => {
      root.render(
        <FlowChatTurnRail
          turns={turns}
          currentTurnId="turn-2"
          visibleTurnIds={['turn-2']}
          onNavigate={vi.fn()}
        />,
      );
    });

    const current = container.querySelector<HTMLButtonElement>('[data-turn-id="turn-2"]');
    const next = container.querySelector<HTMLButtonElement>('[data-turn-id="turn-3"]');
    expect(current).not.toBeNull();
    expect(next).not.toBeNull();
    if (!current || !next) return;

    act(() => {
      current.focus();
      current.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'ArrowDown',
        bubbles: true,
      }));
    });

    expect(document.activeElement).toBe(next);
    expect(next.tabIndex).toBe(0);
    expect(current.tabIndex).toBe(-1);
  });
});
