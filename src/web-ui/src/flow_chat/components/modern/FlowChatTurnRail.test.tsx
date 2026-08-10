// @vitest-environment jsdom

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FlowChatTurnRail, type FlowChatTurnRailItem } from './FlowChatTurnRail';
import { FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX } from './flowChatTurnRailWindow';

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
  { itemKey: 'storage:0', turnId: 'turn-1', ordinal: 0, turnIndex: 1, content: 'First user message' },
  { itemKey: 'storage:1', turnId: 'turn-2', ordinal: 1, turnIndex: 2, content: 'Second user message' },
  { itemKey: 'storage:2', turnId: 'turn-3', ordinal: 2, turnIndex: 3, content: 'Third user message' },
  { itemKey: 'storage:3', turnId: 'turn-4', ordinal: 3, turnIndex: 4, content: 'Fourth user message' },
];

function createTurns(count: number): FlowChatTurnRailItem[] {
  return Array.from({ length: count }, (_, ordinal) => ({
    itemKey: `storage:${ordinal}`,
    turnId: `turn-${ordinal + 1}`,
    ordinal,
    turnIndex: ordinal + 1,
    content: `Message ${ordinal + 1}`,
  }));
}

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
    expect(onNavigate).toHaveBeenCalledWith(turns[2]);
  });

  it('keeps placeholder markers visible and navigable by ordinal', () => {
    const onNavigate = vi.fn();
    act(() => {
      root.render(
        <FlowChatTurnRail
          turns={[
            turns[0],
            { itemKey: 'storage:1', turnId: null, ordinal: 1, turnIndex: 2, content: null },
          ]}
          currentTurnId="turn-1"
          visibleTurnIds={['turn-1']}
          onNavigate={onNavigate}
        />,
      );
    });

    const placeholder = container.querySelector<HTMLButtonElement>('[data-turn-key="storage:1"]');
    expect(placeholder).not.toBeNull();
    expect(placeholder?.getAttribute('aria-disabled')).toBeNull();
    expect(placeholder?.getAttribute('data-turn-id')).toBeNull();

    act(() => placeholder?.click());

    expect(onNavigate).toHaveBeenCalledWith(expect.objectContaining({
      ordinal: 1,
      turnId: null,
    }));
    const tooltipMessages = container.querySelectorAll('.flowchat-turn-rail__tooltip-message');
    expect(tooltipMessages).toHaveLength(1);
  });

  it('keeps marker identity stable when a placeholder resolves', () => {
    act(() => {
      root.render(
        <FlowChatTurnRail
          turns={[{ itemKey: 'storage:7', turnId: null, ordinal: 7, turnIndex: 8, content: null }]}
          currentTurnId={null}
          visibleTurnIds={[]}
          onNavigate={vi.fn()}
        />,
      );
    });
    const placeholder = container.querySelector<HTMLButtonElement>('[data-turn-key="storage:7"]');
    expect(placeholder?.getAttribute('aria-disabled')).toBeNull();

    act(() => {
      root.render(
        <FlowChatTurnRail
          turns={[{
            itemKey: 'storage:7',
            turnId: 'turn-8',
            ordinal: 7,
            turnIndex: 8,
            content: 'Resolved message',
          }]}
          currentTurnId="turn-8"
          visibleTurnIds={['turn-8']}
          onNavigate={vi.fn()}
        />,
      );
    });

    const resolved = container.querySelector<HTMLButtonElement>('[data-turn-key="storage:7"]');
    expect(resolved).toBe(placeholder);
    expect(resolved?.getAttribute('data-turn-id')).toBe('turn-8');
    expect(resolved?.getAttribute('aria-disabled')).toBeNull();
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
    expect(list).not.toBeNull();
    if (!list) return;

    Object.defineProperty(list, 'clientHeight', { configurable: true, value: 40 });
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

    expect(list.scrollTop).toBe(11);
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

  it('bounds rendered markers to the viewport plus overscan', () => {
    const manyTurns = createTurns(100);
    act(() => {
      root.render(
        <FlowChatTurnRail
          turns={manyTurns}
          currentTurnId="turn-1"
          visibleTurnIds={['turn-1']}
          onNavigate={vi.fn()}
        />,
      );
    });

    const rail = container.querySelector<HTMLElement>('[data-testid="flowchat-turn-rail"]');
    const list = container.querySelector<HTMLDivElement>('.flowchat-turn-rail__list');
    expect(rail?.dataset.totalTurnCount).toBe('100');
    expect(list).not.toBeNull();
    if (!list) return;

    Object.defineProperty(list, 'clientHeight', { configurable: true, value: 56 });
    list.scrollTop = 50 * FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX;
    act(() => {
      list.dispatchEvent(new Event('scroll', { bubbles: true }));
    });

    const renderedItems = container.querySelectorAll<HTMLButtonElement>('.flowchat-turn-rail__item');
    expect(renderedItems.length).toBeLessThanOrEqual(18);
    expect(renderedItems.length).toBeGreaterThanOrEqual(10);
    expect(container.querySelector('[data-turn-ordinal="50"]')).not.toBeNull();
    expect(container.querySelector('[data-turn-ordinal="0"]')).toBeNull();
    expect(Array.from(renderedItems).filter(item => item.tabIndex === 0)).toHaveLength(1);
  });

  it('moves virtual keyboard focus across unmounted markers', () => {
    const manyTurns = createTurns(100);
    act(() => {
      root.render(
        <FlowChatTurnRail
          turns={manyTurns}
          currentTurnId="turn-1"
          visibleTurnIds={['turn-1']}
          onNavigate={vi.fn()}
        />,
      );
    });

    const list = container.querySelector<HTMLDivElement>('.flowchat-turn-rail__list');
    const first = container.querySelector<HTMLButtonElement>('[data-turn-ordinal="0"]');
    expect(list).not.toBeNull();
    expect(first).not.toBeNull();
    if (!list || !first) return;
    Object.defineProperty(list, 'clientHeight', { configurable: true, value: 42 });

    act(() => {
      first.focus();
      first.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'End',
        bubbles: true,
      }));
    });

    const last = container.querySelector<HTMLButtonElement>('[data-turn-ordinal="99"]');
    expect(last).not.toBeNull();
    expect(document.activeElement).toBe(last);
    expect(last?.getAttribute('aria-posinset')).toBe('100');
    expect(last?.getAttribute('aria-setsize')).toBe('100');
    expect(container.querySelector('[data-turn-ordinal="0"]')).toBeNull();
  });
});
