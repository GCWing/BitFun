import React, { useMemo, useState } from 'react';
import { MobileButton, MobileCard, MobileTextField } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';
import { messages } from '../i18n/messages';
import type { RemoteToolStatus } from '../services/RemoteSessionManager';

interface ChatAskQuestionCardProps {
  onAnswer: (toolId: string, answers: Record<string, unknown>) => Promise<void>;
  tool: RemoteToolStatus;
}

interface QuestionOption {
  description?: string;
  label: string;
}

interface Question {
  hasBuiltInOther: boolean;
  header?: string;
  multiSelect?: boolean;
  options: QuestionOption[];
  question?: string;
}

function getMessageByPath(source: unknown, path: string): string | null {
  const segments = path.split('.');
  let current: unknown = source;
  for (const segment of segments) {
    if (!current || typeof current !== 'object' || !(segment in current)) return null;
    current = (current as Record<string, unknown>)[segment];
  }
  return typeof current === 'string' ? current : null;
}

const otherOptionLabels = new Set([
  'other',
  ...Object.values(messages)
    .map((localeMessages) => getMessageByPath(localeMessages, 'common.other'))
    .filter((label): label is string => !!label)
    .map((label) => label.trim().toLowerCase()),
]);

function isOtherOption(label: string | undefined): boolean {
  return otherOptionLabels.has((label || '').trim().toLowerCase());
}

export default function ChatAskQuestionCard({ onAnswer, tool }: ChatAskQuestionCardProps) {
  const { t, language } = useI18n();
  const questions = (tool.tool_input?.questions || []) as Array<Omit<Question, 'hasBuiltInOther'>>;
  const [selected, setSelected] = useState<Record<number, string | string[]>>({});
  const [customTexts, setCustomTexts] = useState<Record<number, string>>({});
  const [submitting, setSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState(false);

  const normalizedQuestions = useMemo<Question[]>(() => questions.map((question) => {
    const options = Array.isArray(question.options) ? question.options : [];
    return {
      ...question,
      hasBuiltInOther: options.some((option) => isOtherOption(option?.label)),
      options,
    };
  }), [questions]);

  if (normalizedQuestions.length === 0) return null;

  const handleSelect = (questionIndex: number, label: string, multiple: boolean) => {
    setSelected((current) => {
      if (multiple) {
        const values = (current[questionIndex] as string[] | undefined) || [];
        return {
          ...current,
          [questionIndex]: values.includes(label)
            ? values.filter((value) => value !== label)
            : [...values, label],
        };
      }
      return { ...current, [questionIndex]: current[questionIndex] === label ? undefined! : label };
    });
  };

  const allAnswered = normalizedQuestions.every((question, index) => {
    const answer = selected[index];
    const hasSelection = question.multiSelect ? Array.isArray(answer) && answer.length > 0 : !!answer;
    if (!hasSelection) return false;
    const needsCustomText = Array.isArray(answer)
      ? answer.some((value) => isOtherOption(value))
      : isOtherOption(answer);
    return !needsCustomText || !!(customTexts[index] || '').trim();
  });

  const handleSubmit = async () => {
    if (!allAnswered || submitting || submitted) return;
    const answers: Record<string, unknown> = {};
    normalizedQuestions.forEach((_question, index) => {
      const answer = selected[index];
      const customText = (customTexts[index] || '').trim();
      if (Array.isArray(answer)) {
        answers[String(index)] = answer.map((value) => isOtherOption(value) ? (customText || value) : value);
      } else if (isOtherOption(answer)) {
        answers[String(index)] = customText || answer;
      } else {
        answers[String(index)] = answer ?? '';
      }
    });
    setSubmitting(true);
    try {
      await onAnswer(tool.id, answers);
      setSubmitted(true);
    } finally {
      setSubmitting(false);
    }
  };

  const pluralSuffix = language === 'en-US' && questions.length !== 1 ? 's' : '';

  return (
    <MobileCard padding="none" className="chat-ask-card">
      <div className="chat-ask-card__header">
        <span className="chat-ask-card__count">{t('chat.askQuestionCount', { count: questions.length, suffix: pluralSuffix })}</span>
        {!submitted && !submitting && <span className="chat-ask-card__waiting">{t('chat.waiting')}</span>}
      </div>
      {normalizedQuestions.map((question, questionIndex) => {
        const answer = selected[questionIndex];
        const otherSelected = Array.isArray(answer)
          ? answer.some((value) => isOtherOption(value))
          : isOtherOption(answer);
        return (
          <div key={questionIndex} className="chat-ask-card__question">
            <div className="chat-ask-card__question-header">
              <span className="chat-ask-card__tag">{question.header}</span>
              <span className="chat-ask-card__question-text">{question.question}</span>
            </div>
            <div className="chat-ask-card__options">
              {question.options.map((option, optionIndex) => {
                const optionSelected = question.multiSelect
                  ? (selected[questionIndex] as string[] || []).includes(option.label)
                  : selected[questionIndex] === option.label;
                return (
                  <MobileButton
                    appearance="secondary"
                    className={`chat-ask-card__option ${optionSelected ? 'is-selected' : ''}`}
                    disabled={submitted || submitting}
                    key={optionIndex}
                    onClick={() => handleSelect(questionIndex, option.label, !!question.multiSelect)}
                  >
                    <span className={`chat-ask-card__radio ${question.multiSelect ? 'chat-ask-card__radio--multi' : ''}`}>
                      {optionSelected && <svg width="8" height="8" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M3 8L6.5 11.5L13 4.5" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" /></svg>}
                    </span>
                    <span className="chat-ask-card__option-label">{option.label}</span>
                    {option.description && <span className="chat-ask-card__option-desc">{option.description}</span>}
                  </MobileButton>
                );
              })}
              {!question.hasBuiltInOther && (
                <MobileButton
                  appearance="secondary"
                  className={`chat-ask-card__option ${otherSelected ? 'is-selected' : ''}`}
                  disabled={submitted || submitting}
                  onClick={() => handleSelect(questionIndex, 'Other', !!question.multiSelect)}
                >
                  <span className={`chat-ask-card__radio ${question.multiSelect ? 'chat-ask-card__radio--multi' : ''}`}>
                    {otherSelected && <svg width="8" height="8" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M3 8L6.5 11.5L13 4.5" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" /></svg>}
                  </span>
                  <span className="chat-ask-card__option-label">{t('common.other')}</span>
                  <span className="chat-ask-card__option-desc">{t('common.customTextInput')}</span>
                </MobileButton>
              )}
              {otherSelected && (
                <MobileTextField
                  appearance="surface"
                  disabled={submitted || submitting}
                  className="chat-ask-card__custom-input"
                  onChange={(event) => setCustomTexts((current) => ({ ...current, [questionIndex]: event.target.value }))}
                  placeholder={t('common.typeYourAnswer')}
                  value={customTexts[questionIndex] || ''}
                />
              )}
            </div>
          </div>
        );
      })}
      <MobileButton appearance="primary" block className="chat-ask-card__submit chat-ask-card__submit--bottom" disabled={!allAnswered || submitted || submitting} onClick={() => void handleSubmit()}>
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M2 8L6 12L14 4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" /></svg>
        {submitted ? t('common.submitted') : submitting ? t('common.submitting') : t('common.submit')}
      </MobileButton>
    </MobileCard>
  );
}
