// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import type { IssueFixUserTodo } from '@/infrastructure/api';
import { IssueFixTodoMessage } from './IssueFixTodoMessage';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

// The component resolves i18n keys through react-i18next; without a loaded
// backend the raw key comes back, which is exactly what these tests assert —
// the point is which key gets chosen and what interpolation it receives.
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

  it('recognizes a comment authorization and renders the localized action line', () => {
    act(() => {
      root.render(<IssueFixTodoMessage todo={todo()} />);
    });

    const action = container.querySelector('.issue-fix__todo-message-action');
    const context = container.querySelector('.issue-fix__todo-message-context');

    expect(action?.textContent).toBe('autonomous.actionLine.postComment');
    expect(context?.textContent).toBe('Comment_only route: diagnosis drafted');
  });

  it('recognizes an issue-close authorization and omits the missing context line', () => {
    act(() => {
      root.render(
        <IssueFixTodoMessage
          todo={todo({ text: 'Authorize closing issue #2016', link: null })}
        />,
      );
    });

    expect(container.querySelector('.issue-fix__todo-message-action')?.textContent).toBe(
      'autonomous.actionLine.closeIssue',
    );
    expect(container.querySelector('.issue-fix__todo-message-context')).toBeNull();
  });

  it('falls back to the compact free-form action when no shape matches', () => {
    act(() => {
      root.render(
        <IssueFixTodoMessage
          todo={todo({ text: 'Authorize rotating the deploy credentials (expired yesterday)' })}
        />,
      );
    });

    expect(container.querySelector('.issue-fix__todo-message-action')?.textContent).toBe(
      'Rotating the deploy credentials',
    );
    expect(container.querySelector('.issue-fix__todo-message-context')?.textContent).toBe(
      'Expired yesterday',
    );
  });

  it('preserves the full context line on a long drafted response', () => {
    act(() => {
      root.render(
        <IssueFixTodoMessage
          todo={todo({
            text: '[P0] Authorize posting response comment for GCWing/BitFun#1290 (discussion question: the user asks about a chat group; the drafted response includes every support channel and a long explanation)',
          })}
        />,
      );
    });

    const context = container.querySelector('.issue-fix__todo-message-context');
    // Full context is preserved; wrapping is the display layer's job.
    expect(context!.textContent).toContain('drafted response includes every support channel');
  });
});
