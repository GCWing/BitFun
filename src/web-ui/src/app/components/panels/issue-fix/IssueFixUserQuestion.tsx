import React, { useEffect, useMemo, useState } from 'react';
import { AlertCircle, ArrowUp, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, Textarea } from '@/component-library';
import {
  UserQuestionItem,
  type UserQuestionData,
} from '@/flow_chat/tool-cards/AskUserQuestionCard';
import type {
  IssueFixUserDecision,
  IssueFixUserQuestion as IssueFixUserQuestionData,
} from '@/infrastructure/api';

interface IssueFixUserQuestionProps {
  question: IssueFixUserQuestionData;
  submitting: boolean;
  error?: string | null;
  onSubmit: (decision: IssueFixUserDecision, reason: string) => void;
}

export const IssueFixUserQuestion: React.FC<IssueFixUserQuestionProps> = ({
  question,
  submitting,
  error,
  onSubmit,
}) => {
  const { t } = useTranslation('panels/issue-fix');
  const [decision, setDecision] = useState<IssueFixUserDecision | undefined>();
  const [reason, setReason] = useState('');

  useEffect(() => {
    setDecision(undefined);
    setReason('');
  }, [question.todoId]);

  const questionData = useMemo<UserQuestionData>(() => ({
    header: t('autonomous.userQuestion.header'),
    question: question.prompt,
    multiSelect: false,
    options: [
      {
        value: 'approve',
        label: t('autonomous.userQuestion.options.approve.label'),
        description: t('autonomous.userQuestion.options.approve.description'),
      },
      {
        value: 'reject',
        label: t('autonomous.userQuestion.options.reject.label'),
        description: t('autonomous.userQuestion.options.reject.description'),
      },
      {
        value: 'cancel',
        label: t('autonomous.userQuestion.options.cancel.label'),
        description: t('autonomous.userQuestion.options.cancel.description'),
      },
    ],
  }), [question.prompt, t]);

  return (
    <section
      className="issue-fix__user-question ask-user-question-card"
      aria-label={t('autonomous.userQuestion.title')}
    >
      <div className="card-header-row">
        <div className="card-title">
          <span className="questions-count">{t('autonomous.userQuestion.title')}</span>
        </div>
      </div>
      <div className="questions-container">
        <UserQuestionItem
          question={questionData}
          inputName={`issue-fix-gate-${question.todoId}`}
          value={decision}
          disabled={submitting}
          allowOther={false}
          onValueChange={(value) => {
            if (typeof value === 'string') setDecision(value as IssueFixUserDecision);
          }}
        />
        <Textarea
          className="issue-fix__user-question-reason"
          label={t('autonomous.userQuestion.reasonLabel')}
          placeholder={t('autonomous.userQuestion.reasonPlaceholder')}
          value={reason}
          onChange={(event) => setReason(event.target.value)}
          maxLength={240}
          rows={2}
          autoResize
          disabled={submitting}
        />
        {error ? <p className="issue-fix__error">{error}</p> : null}
      </div>
      <div className="card-footer-row">
        <div className="footer-actions">
          <Button
            size="small"
            variant="primary"
            className="issue-fix__user-question-submit"
            disabled={!decision || submitting}
            isLoading={submitting}
            onClick={() => {
              if (decision) onSubmit(decision, reason.trim());
            }}
          >
            {submitting ? <Loader2 size={14} /> : <ArrowUp size={14} />}
            <span>
              {submitting
                ? t('autonomous.userQuestion.submitting')
                : t('autonomous.userQuestion.submit')}
            </span>
          </Button>
          <div className="tool-status">
            <AlertCircle size={16} className="status-icon-waiting" />
            <span className="status-text">{t('autonomous.userQuestion.waiting')}</span>
          </div>
        </div>
      </div>
    </section>
  );
};
