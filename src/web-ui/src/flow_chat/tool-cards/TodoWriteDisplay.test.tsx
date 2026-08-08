// @vitest-environment jsdom

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';
import { createTodoRenderItems } from './todoRenderItems';
import { TodoWriteDisplay } from './TodoWriteDisplay';
import { FLOWCHAT_COLLAPSE_DURATION_MS } from '../components/modern/flowChatCollapseMotion';

vi.mock('react-i18next', async (importOriginal) => ({
  ...await importOriginal<typeof import('react-i18next')>(),
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('../hooks/useDialogTurnTodos', () => ({
  useDialogTurnTodos: () => [],
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const config: ToolCardConfig = {
  toolName: 'TodoWrite',
  displayName: 'TodoWrite',
  icon: 'list-todo',
  requiresConfirmation: false,
  resultDisplayType: 'detailed',
  displayMode: 'standard',
};

function createTodoWriteItem(status: 'pending' | 'in_progress'): FlowToolItem {
  return {
    id: 'todo-tool-a',
    type: 'tool',
    toolName: 'TodoWrite',
    timestamp: 1,
    status: 'streaming',
    isParamsStreaming: true,
    partialParams: {
      todos: [{ id: 'todo-a', content: 'Implement change', status }],
    },
    toolCall: {
      id: 'todo-tool-a',
      input: {},
    },
  };
}

describe('createTodoRenderItems', () => {
  it('keeps React render keys unique when restored todos reuse ids', () => {
    const items = createTodoRenderItems([
      { id: '[truncated for session view]', content: 'Phase 1', status: 'completed' },
      { id: '[truncated for session view]', content: 'Phase 2', status: 'completed' },
      { id: 'p3-2', content: 'Phase 3', status: 'pending' },
    ]);

    expect(new Set(items.map(item => item.key)).size).toBe(items.length);
    expect(items.map(item => item.key)).toEqual([
      '[truncated for session view]-0',
      '[truncated for session view]-1',
      'p3-2',
    ]);
  });
});

describe('TodoWriteDisplay automatic collapse', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.append(container);
    root = createRoot(container);
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function () {
      const height = (this as HTMLElement).querySelector?.('.todo-expanded-body') ? 320 : 64;
      return {
        bottom: height,
        height,
        left: 0,
        right: 300,
        top: 0,
        width: 300,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      };
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('lets completed todo content collapse through normal layout state', () => {
    vi.useFakeTimers();
    act(() => {
      root.render(<TodoWriteDisplay toolItem={createTodoWriteItem('pending')} config={config} />);
    });
    expect(container.querySelector('.todo-expanded-body')).not.toBeNull();

    act(() => {
      root.render(<TodoWriteDisplay toolItem={createTodoWriteItem('in_progress')} config={config} />);
    });

    expect(container.querySelector('.todo-expanded-body')).not.toBeNull();

    act(() => {
      vi.advanceTimersByTime(FLOWCHAT_COLLAPSE_DURATION_MS);
    });
    expect(container.querySelector('.todo-expanded-body')).toBeNull();
  });
});
