// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import type { IssueFixUserTodo } from '@/infrastructure/api';
import { IssueFixTodoMessage } from './IssueFixTodoMessage';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const todo = (overrides: Partial<IssueFixUserTodo> = {}): IssueFixUserTodo => ({
  todoId: 'todo-1',
  taskClass: 'user_action',
  text:
    '[P0] Authorize posting maintainer diagnosis comment for GCWing/BitFun#2032 (comment_only route: diagnosis drafted)',
  link: null,
  ...overrides,
});

describe('IssueFixTodoMessage', () => {
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

  it('projects a legacy todo into a short action plus the supporting context', () => {
    act(() => {
      root.render(<IssueFixTodoMessage todo={todo()} />);
    });

    const action = container.querySelector('.issue-fix__todo-message-action');
    const context = container.querySelector('.issue-fix__todo-message-context');

    expect(action?.textContent).toBe('Posting maintainer diagnosis comment for Issue #2032');
    expect(context?.textContent).toBe('Comment_only route: diagnosis drafted');
  });

  it('omits the context line when the todo carries no state/reason tail', () => {
    act(() => {
      root.render(
        <IssueFixTodoMessage
          todo={todo({ text: 'Authorize closing issue #2016', link: null })}
        />,
      );
    });

    expect(container.querySelector('.issue-fix__todo-message-action')?.textContent).toBe(
      'Closing issue #2016',
    );
    expect(container.querySelector('.issue-fix__todo-message-context')).toBeNull();
  });

  it('keeps both lines bounded so the toast stays scannable on a long drafted response', () => {
    act(() => {
      root.render(
        <IssueFixTodoMessage
          todo={todo({
            text: '[P0] Authorize posting response comment for GCWing/BitFun#1290 (discussion question: the user asks about a chat group; the drafted response includes every support channel and a long explanation)',
          })}
        />,
      );
    });

    const action = container.querySelector('.issue-fix__todo-message-action');
    const context = container.querySelector('.issue-fix__todo-message-context');

    expect(action!.textContent!.length).toBeLessThanOrEqual(72);
    expect(context!.textContent!.length).toBeLessThanOrEqual(96);
  });
});
