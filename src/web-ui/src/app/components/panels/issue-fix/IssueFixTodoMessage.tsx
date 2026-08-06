import React from 'react';
import { useTranslation } from 'react-i18next';
import type { IssueFixUserTodo } from '@/infrastructure/api';
import { userTodoPresentation } from './issueFixRunState';

interface IssueFixTodoMessageProps {
  todo: IssueFixUserTodo;
}

/**
 * Compact two-line rendering of a pending user-lane todo. The first line says
 * what already happened and what is asked of the user, phrased in the UI
 * language when the todo matches a known action shape (merge PR / close issue
 * / post comment); the second line carries the agent-supplied state/reason.
 * Shared by the app-wide toast and the notification center so the wording
 * cannot drift between surfaces.
 */
export const IssueFixTodoMessage: React.FC<IssueFixTodoMessageProps> = ({ todo }) => {
  const { t } = useTranslation('panels/issue-fix');
  const presentation = userTodoPresentation(todo);
  const kind = presentation.kind;
  const action =
    kind?.type === 'mergePr'
      ? kind.issue
        ? t('autonomous.actionLine.mergePrForIssue', { pr: kind.pr, issue: kind.issue })
        : t('autonomous.actionLine.mergePr', { pr: kind.pr })
      : kind?.type === 'closeIssue'
        ? kind.pr
          ? t('autonomous.actionLine.closeIssueByPr', { issue: kind.issue, pr: kind.pr })
          : t('autonomous.actionLine.closeIssue', { issue: kind.issue })
        : kind?.type === 'postComment'
          ? t('autonomous.actionLine.postComment', { issue: kind.issue })
          : presentation.action;
  return (
    <span className="issue-fix__todo-message">
      <span className="issue-fix__todo-message-action">{action}</span>
      {presentation.context ? (
        <span className="issue-fix__todo-message-context">{presentation.context}</span>
      ) : null}
    </span>
  );
};
