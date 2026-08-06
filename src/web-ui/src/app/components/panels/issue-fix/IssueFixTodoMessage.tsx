import React from 'react';
import type { IssueFixUserTodo } from '@/infrastructure/api';
import { userTodoPresentation } from './issueFixRunState';

interface IssueFixTodoMessageProps {
  todo: IssueFixUserTodo;
}

/**
 * Compact two-line rendering of a pending user-lane todo. The first line is
 * the action itself; the second, when the backend supplied a state/reason
 * tail, carries the supporting context (current status / why you are asked).
 * Shared by the in-panel "pending your action" block and the app-wide toast
 * so the wording cannot drift between surfaces.
 */
export const IssueFixTodoMessage: React.FC<IssueFixTodoMessageProps> = ({ todo }) => {
  const presentation = userTodoPresentation(todo);
  return (
    <span className="issue-fix__todo-message">
      <span className="issue-fix__todo-message-action">{presentation.action}</span>
      {presentation.context ? (
        <span className="issue-fix__todo-message-context">{presentation.context}</span>
      ) : null}
    </span>
  );
};
