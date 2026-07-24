// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { VirtualItem } from '../../store/modernFlowChatStore';
import {
  buildFlowChatSearchMatches,
  useFlowChatSearch,
  type UseFlowChatSearchReturn,
} from './useFlowChatSearch';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

function searchableItems(count: number): VirtualItem[] {
  return Array.from({ length: count }, (_, index) => ({
    type: 'user-message',
    turnId: `turn-${index}`,
    data: {
      id: `message-${index}`,
      content: `needle ${index}`,
    },
  })) as VirtualItem[];
}

function SearchHarness({
  items,
  onController,
}: {
  items: VirtualItem[];
  onController: (controller: UseFlowChatSearchReturn) => void;
}) {
  onController(useFlowChatSearch(items));
  return null;
}

describe('buildFlowChatSearchMatches', () => {
  it('keeps the concrete model text source used for exact navigation', () => {
    const virtualItems = [{
      type: 'model-round',
      turnId: 'turn-1',
      isLastRound: true,
      isTurnComplete: true,
      data: {
        id: 'round-1',
        items: [
          { id: 'text-1', type: 'text', content: 'unrelated' },
          { id: 'text-2', type: 'text', content: 'The precise keyword is here.' },
        ],
      },
    }] as VirtualItem[];

    expect(buildFlowChatSearchMatches(virtualItems, 'KEYWORD')).toEqual([{
      virtualItemIndex: 0,
      turnId: 'turn-1',
      type: 'model-round',
      flowItemId: 'text-2',
      expandableIds: undefined,
    }]);
  });

  it('records collapsed containers from outermost to innermost', () => {
    const virtualItems = [{
      type: 'explore-group',
      turnId: 'turn-1',
      data: {
        groupId: 'group-1',
        allItems: [{
          id: 'thinking-1',
          type: 'thinking',
          content: 'hidden needle',
        }],
      },
    }] as VirtualItem[];

    expect(buildFlowChatSearchMatches(virtualItems, 'needle')[0]).toMatchObject({
      flowItemId: 'thinking-1',
      expandableIds: ['group-1', 'thinking-1'],
    });
  });

  it('deduplicates by turn while searching steering messages', () => {
    const virtualItems = [
      {
        type: 'user-steering-message',
        turnId: 'turn-1',
        steeringId: 'steering-1',
        steeringStatus: 'completed',
        data: { id: 'user-1', content: 'needle in steering' },
      },
      {
        type: 'model-round',
        turnId: 'turn-1',
        isLastRound: true,
        isTurnComplete: true,
        data: {
          id: 'round-1',
          items: [{ id: 'text-1', type: 'text', content: 'needle again' }],
        },
      },
    ] as VirtualItem[];

    expect(buildFlowChatSearchMatches(virtualItems, 'needle')).toHaveLength(1);
    expect(buildFlowChatSearchMatches(virtualItems, 'needle')[0]).toMatchObject({
      virtualItemIndex: 0,
      type: 'user-steering-message',
    });
  });
});

describe('useFlowChatSearch navigation', () => {
  let container: HTMLDivElement;
  let root: ReturnType<typeof createRoot>;
  let controller: UseFlowChatSearchReturn | null;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    controller = null;
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  function render(items: VirtualItem[]): void {
    act(() => {
      root.render(React.createElement(SearchHarness, {
        items,
        onController: nextController => {
          controller = nextController;
        },
      }));
    });
  }

  function selectLastOfFive(): void {
    render(searchableItems(5));
    act(() => controller?.onSearchChange('needle'));
    act(() => controller?.goToPrev());
    expect(controller?.currentMatchIndex).toBe(4);
  }

  it('moves next from the resolved index after matches shrink', () => {
    selectLastOfFive();
    render(searchableItems(3));

    expect(controller?.currentMatchIndex).toBe(2);
    act(() => controller?.goToNext());

    expect(controller?.currentMatchIndex).toBe(0);
  });

  it('moves previous from the resolved index after matches shrink', () => {
    selectLastOfFive();
    render(searchableItems(3));

    expect(controller?.currentMatchIndex).toBe(2);
    act(() => controller?.goToPrev());

    expect(controller?.currentMatchIndex).toBe(1);
  });
});
