/**
 * Product adapter for the public AskUser design-system component.
 *
 * This file owns tool payload parsing, per-surface draft persistence,
 * localization, submission, and FlowChat virtualization coordination.
 * AskUser owns the rendered anatomy, interaction semantics, and styling.
 */

import {
  AskUser,
  type AskUserAnswers,
  type AskUserQuestion,
  type AskUserState,
} from '@bitfun/ui/flow-chat';
import React, {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { getActiveSurfaceId } from '@/infrastructure/peer-device/deviceSurface';
import { createLogger } from '@/shared/utils/logger';
import type { FlowToolItem, ToolCardProps } from '../types/flow-chat';
import {
  askUserQuestionDraftKey,
  askUserQuestionDraftStore,
  createEmptyAskUserQuestionDraft,
  useAskUserQuestionDraftStore,
  type AskUserQuestionSubmissionPhase,
} from '../store/askUserQuestionDraftStore';
import { useToolCardHeightContract } from './useToolCardHeightContract';

const log = createLogger('AskUserQuestionCard');
const OTHER_OPTION_VALUE = 'Other';

interface QuestionOption {
  description: string;
  label: string;
}

interface QuestionData {
  header: string;
  multiSelect: boolean;
  options: QuestionOption[];
  question: string;
}

type ToolAnswer = string | string[];

function normalizeQuestionsFromParams(input: unknown): QuestionData[] {
  if (!input || typeof input !== 'object') return [];
  const rawQuestions = (input as Record<string, unknown>).questions;
  if (!Array.isArray(rawQuestions)) return [];

  return rawQuestions.flatMap((candidate): QuestionData[] => {
    if (!candidate || typeof candidate !== 'object') return [];
    const rawQuestion = candidate as Record<string, unknown>;
    const rawOptions = Array.isArray(rawQuestion.options)
      ? rawQuestion.options
      : [];
    const options = rawOptions.flatMap((option): QuestionOption[] => {
      if (!option || typeof option !== 'object') return [];
      const rawOption = option as Record<string, unknown>;
      if (typeof rawOption.label !== 'string') return [];
      return [{
        description: typeof rawOption.description === 'string'
          ? rawOption.description
          : '',
        label: rawOption.label,
      }];
    });

    return [{
      header: typeof rawQuestion.header === 'string' ? rawQuestion.header : '',
      multiSelect: Boolean(rawQuestion.multiSelect),
      options,
      question: typeof rawQuestion.question === 'string'
        ? rawQuestion.question
        : '',
    }];
  });
}

function normalizeToolResult(input: unknown): Record<string, unknown> | null {
  if (input === null || input === undefined) return null;
  try {
    const parsed = typeof input === 'string' ? JSON.parse(input) : input;
    return parsed && typeof parsed === 'object'
      ? parsed as Record<string, unknown>
      : null;
  } catch {
    return null;
  }
}

function normalizeToolAnswer(input: unknown): ToolAnswer | undefined {
  if (typeof input === 'string') return input;
  if (Array.isArray(input)) {
    return input.filter((value): value is string => typeof value === 'string');
  }
  return undefined;
}

/** Same source as FileOperationToolCard: partial JSON while streaming, then final toolCall.input. */
function isAwaitingQuestionPayload(
  questionsLength: number,
  isParamsStreaming: boolean | undefined,
  status: FlowToolItem['status'],
): boolean {
  if (questionsLength > 0) return false;
  if (isParamsStreaming) return true;
  const rawStatus = status as string;
  return status === 'preparing'
    || status === 'streaming'
    || status === 'pending'
    || rawStatus === 'receiving';
}

export const AskUserQuestionCard: React.FC<ToolCardProps> = ({
  toolItem,
  isLastItem,
  sessionId,
}) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolCall, toolResult, isParamsStreaming, partialParams } = toolItem;
  const paramsSource = partialParams || toolCall?.input;
  const questions = useMemo(
    () => normalizeQuestionsFromParams(paramsSource),
    [paramsSource],
  );
  const awaitingPayload = isAwaitingQuestionPayload(
    questions.length,
    isParamsStreaming,
    status,
  );

  const toolId = toolItem.id ?? toolCall?.id;
  const draftToolId = toolCall?.id || toolId;
  const activeSurfaceId = getActiveSurfaceId();
  const draftKey = useMemo(
    () => sessionId && draftToolId
      ? askUserQuestionDraftKey(sessionId, draftToolId, activeSurfaceId)
      : null,
    [activeSurfaceId, draftToolId, sessionId],
  );
  const storedDraft = useAskUserQuestionDraftStore((state) => (
    draftKey ? state.drafts[draftKey] : undefined
  ));
  const [localDraft, setLocalDraft] = useState(createEmptyAskUserQuestionDraft);
  const draft = storedDraft ?? localDraft;
  const { answers, otherInputs, submissionPhase } = draft;
  const isSubmitting = submissionPhase === 'submitting';
  const isSubmitted = submissionPhase === 'submitted';
  const [isExpanded, setIsExpanded] = useState(false);
  const [showCompletedSummary, setShowCompletedSummary] = useState(status === 'completed');
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  useLayoutEffect(() => {
    const shouldCompactCompleted = status === 'completed'
      && isLastItem !== true
      && !showCompletedSummary;

    if (shouldCompactCompleted) {
      applyExpandedState(true, false, (nextExpanded) => {
        setShowCompletedSummary(!nextExpanded);
      });
      return;
    }

    if (status !== 'completed' && showCompletedSummary) {
      setShowCompletedSummary(false);
    }
  }, [applyExpandedState, isLastItem, showCompletedSummary, status]);

  useEffect(() => {
    if (
      draftKey
      && (
        status === 'completed'
        || status === 'cancelled'
        || status === 'rejected'
        || status === 'error'
      )
    ) {
      askUserQuestionDraftStore.getState().clearDraft(draftKey);
    }
  }, [draftKey, status]);

  const setSubmissionPhase = useCallback((phase: AskUserQuestionSubmissionPhase) => {
    if (draftKey) {
      askUserQuestionDraftStore.getState().setSubmissionPhase(draftKey, phase);
      return;
    }
    setLocalDraft((current) => ({
      ...current,
      submissionPhase: phase,
      updatedAt: Date.now(),
    }));
  }, [draftKey]);

  const isAllAnswered = useCallback(() => {
    if (questions.length === 0) return false;

    for (let index = 0; index < questions.length; index += 1) {
      const answer = answers[index];
      if (!answer) return false;
      const otherInput = otherInputs[index]?.trim() || '';
      if (
        Array.isArray(answer)
        && !answer.some((value) => value !== OTHER_OPTION_VALUE || otherInput.length > 0)
      ) return false;
      if (typeof answer === 'string' && answer === '') return false;
      if (answer === OTHER_OPTION_VALUE && otherInput.length === 0) return false;
    }
    return true;
  }, [answers, otherInputs, questions.length]);

  const handleSingleChange = useCallback((questionIndex: number, value: string) => {
    if (draftKey) {
      askUserQuestionDraftStore.getState().setSingleAnswer(draftKey, questionIndex, value);
      return;
    }
    setLocalDraft((current) => ({
      ...current,
      answers: {
        ...current.answers,
        [questionIndex]: value,
      },
      updatedAt: Date.now(),
    }));
  }, [draftKey]);

  const handleMultiChange = useCallback((
    questionIndex: number,
    value: string,
    checked: boolean,
  ) => {
    if (draftKey) {
      askUserQuestionDraftStore.getState().setMultiAnswer(
        draftKey,
        questionIndex,
        value,
        checked,
      );
      return;
    }
    setLocalDraft((current) => {
      const currentAnswer = current.answers[questionIndex];
      const currentValues = Array.isArray(currentAnswer) ? currentAnswer : [];
      const nextValues = checked
        ? currentValues.includes(value) ? currentValues : [...currentValues, value]
        : currentValues.filter((candidate) => candidate !== value);
      return {
        ...current,
        answers: {
          ...current.answers,
          [questionIndex]: nextValues,
        },
        updatedAt: Date.now(),
      };
    });
  }, [draftKey]);

  const handleOtherInputChange = useCallback((
    questionIndex: number,
    value: string,
    preserveOtherSelection = false,
  ) => {
    if (draftKey) {
      askUserQuestionDraftStore.getState().setOtherInput(
        draftKey,
        questionIndex,
        value,
        preserveOtherSelection,
      );
      return;
    }
    setLocalDraft((current) => {
      const isEmpty = value.trim().length === 0;
      const currentAnswer = current.answers[questionIndex];
      let nextAnswers = current.answers;
      if (
        isEmpty
        && !preserveOtherSelection
        && Array.isArray(currentAnswer)
        && currentAnswer.includes(OTHER_OPTION_VALUE)
      ) {
        nextAnswers = {
          ...current.answers,
          [questionIndex]: currentAnswer.filter((answer) => answer !== OTHER_OPTION_VALUE),
        };
      } else if (
        isEmpty
        && !preserveOtherSelection
        && currentAnswer === OTHER_OPTION_VALUE
      ) {
        nextAnswers = { ...current.answers };
        delete nextAnswers[questionIndex];
      }
      return {
        ...current,
        answers: nextAnswers,
        otherInputs: {
          ...current.otherInputs,
          [questionIndex]: isEmpty ? '' : value,
        },
        updatedAt: Date.now(),
      };
    });
  }, [draftKey]);

  const handleSubmit = useCallback(async () => {
    if (!isAllAnswered() || isSubmitting || isSubmitted) return;

    setSubmissionPhase('submitting');
    try {
      const processedAnswers: Record<string, string | string[]> = {};

      for (let index = 0; index < questions.length; index += 1) {
        const answer = answers[index];
        const otherInput = otherInputs[index]?.trim() || '';

        if (Array.isArray(answer)) {
          processedAnswers[String(index)] = answer.flatMap((value) => (
            value === OTHER_OPTION_VALUE
              ? otherInput ? [otherInput] : []
              : [value]
          ));
        } else if (answer === OTHER_OPTION_VALUE) {
          if (otherInput) processedAnswers[String(index)] = otherInput;
        } else {
          processedAnswers[String(index)] = answer;
        }
      }

      await toolAPI.submitUserAnswers(toolId, processedAnswers);
      setSubmissionPhase('submitted');
    } catch (error) {
      log.error('Failed to submit answers', { toolId, error });
      setSubmissionPhase('idle');
    }
  }, [
    answers,
    isAllAnswered,
    isSubmitted,
    isSubmitting,
    otherInputs,
    questions.length,
    setSubmissionPhase,
    toolId,
  ]);

  const normalizedResult = useMemo(
    () => normalizeToolResult(toolResult?.result),
    [toolResult?.result],
  );
  const resultAnswers = normalizedResult?.answers;

  const getEffectiveAnswer = useCallback((questionIndex: number): ToolAnswer | undefined => {
    const localAnswer = answers[questionIndex];
    if (localAnswer !== undefined) return localAnswer;
    if (
      status === 'completed'
      && resultAnswers
      && typeof resultAnswers === 'object'
      && !Array.isArray(resultAnswers)
    ) {
      return normalizeToolAnswer(
        (resultAnswers as Record<string, unknown>)[String(questionIndex)],
      );
    }
    return undefined;
  }, [answers, resultAnswers, status]);

  const presentation = useMemo(() => {
    const nextAnswers: Record<string, readonly string[]> = {};
    const nextCustomAnswers: Record<string, string> = {};

    questions.forEach((question, questionIndex) => {
      const answer = getEffectiveAnswer(questionIndex);
      const answerValues = Array.isArray(answer)
        ? answer
        : answer === undefined || answer === '' ? [] : [answer];
      const knownValues = new Set(question.options.map((option) => option.label));
      const selectedValues: string[] = [];
      const customValues: string[] = [];

      answerValues.forEach((value) => {
        if (value === OTHER_OPTION_VALUE) {
          selectedValues.push(OTHER_OPTION_VALUE);
        } else if (knownValues.has(value)) {
          selectedValues.push(value);
        } else if (value) {
          if (!selectedValues.includes(OTHER_OPTION_VALUE)) {
            selectedValues.push(OTHER_OPTION_VALUE);
          }
          customValues.push(value);
        }
      });

      nextAnswers[String(questionIndex)] = selectedValues;
      nextCustomAnswers[String(questionIndex)] = otherInputs[questionIndex]
        || customValues.join(', ');
    });

    return {
      answers: nextAnswers as AskUserAnswers,
      customAnswers: nextCustomAnswers,
    };
  }, [getEffectiveAnswer, otherInputs, questions]);

  const designQuestions = useMemo<AskUserQuestion[]>(
    () => questions.map((question, questionIndex) => ({
      customOption: {
        description: t('toolCards.askUser.customInputHint'),
        inputLabel: t('toolCards.askUser.pleaseSpecify'),
        label: t('toolCards.askUser.other'),
        placeholder: t('toolCards.askUser.pleaseSpecify'),
        value: OTHER_OPTION_VALUE,
      },
      id: String(questionIndex),
      options: question.options.map((option) => ({
        description: option.description,
        label: option.label,
        value: option.label,
      })),
      prompt: question.question,
      selectionMode: question.multiSelect ? 'multiple' : 'single',
    })),
    [questions, t],
  );

  const handleAnswersChange = useCallback((
    questionId: string,
    nextValues: readonly string[],
  ) => {
    const questionIndex = Number(questionId);
    const question = questions[questionIndex];
    if (!question || !Number.isInteger(questionIndex)) return;

    if (!question.multiSelect) {
      handleSingleChange(questionIndex, nextValues[0] ?? '');
      return;
    }

    const currentAnswer = answers[questionIndex];
    const currentValues = Array.isArray(currentAnswer) ? currentAnswer : [];
    const changedValue = [...new Set([...currentValues, ...nextValues])]
      .find((value) => currentValues.includes(value) !== nextValues.includes(value));
    if (changedValue !== undefined) {
      handleMultiChange(questionIndex, changedValue, nextValues.includes(changedValue));
    }
  }, [answers, handleMultiChange, handleSingleChange, questions]);

  const getAnswerDisplay = useCallback((questionIndex: number): string => {
    const answer = getEffectiveAnswer(questionIndex);
    const otherInput = otherInputs[questionIndex] || '';
    if (!answer) return '';
    if (Array.isArray(answer)) {
      return answer.map((value) => (
        value === OTHER_OPTION_VALUE ? otherInput || OTHER_OPTION_VALUE : value
      )).join(', ');
    }
    return answer === OTHER_OPTION_VALUE
      ? otherInput || OTHER_OPTION_VALUE
      : String(answer);
  }, [getEffectiveAnswer, otherInputs]);

  const answersSummary = useMemo(
    () => questions.map((question, questionIndex) => {
      const answerText = getAnswerDisplay(questionIndex);
      const label = question.header || question.question;
      return `${label}: ${answerText || t('toolCards.askUser.notAnswered')}`;
    }).join(' | '),
    [getAnswerDisplay, questions, t],
  );

  const statusText = status === 'completed'
    ? t('toolCards.askUser.completed')
    : isSubmitted
      ? t('toolCards.askUser.submittedWaiting')
      : isSubmitting
        ? t('toolCards.askUser.submitting')
        : t('toolCards.askUser.waitingAnswer');

  if (awaitingPayload) {
    return (
      <AskUser
        data-tool-card-id={toolId ?? ''}
        questions={[]}
        ref={cardRootRef}
        state="loading"
        statusLabel={t('toolCards.askUser.loadingQuestions')}
      />
    );
  }

  if (questions.length === 0) {
    return (
      <AskUser
        data-tool-card-id={toolId ?? ''}
        questions={[]}
        ref={cardRootRef}
        state="error"
        statusLabel={t('toolCards.askUser.parseError')}
      />
    );
  }

  const timedOut = normalizedResult?.status === 'timeout';
  const componentState: AskUserState = timedOut
    ? 'timeout'
    : status === 'completed'
      ? 'completed'
      : isSubmitting
        ? 'submitting'
        : isSubmitted
          ? 'submitted'
          : 'asking';
  const showSubmit = componentState === 'asking'
    || componentState === 'submitting'
    || componentState === 'submitted';

  return (
    <AskUser
      answers={presentation.answers}
      aria-label={t('toolCards.askUser.questionsCount', { count: questions.length })}
      customAnswers={presentation.customAnswers}
      data-tool-card-id={toolId ?? ''}
      disabled={Boolean(isParamsStreaming)}
      expanded={showCompletedSummary ? isExpanded : undefined}
      header={showCompletedSummary
        ? undefined
        : t('toolCards.askUser.questionsCount', { count: questions.length })}
      onAnswersChange={handleAnswersChange}
      onCustomAnswerChange={(questionId, value, meta) => {
        handleOtherInputChange(Number(questionId), value, meta.isComposing);
      }}
      onExpandedChange={showCompletedSummary
        ? (nextExpanded) => {
            applyExpandedState(isExpanded, nextExpanded, setIsExpanded);
          }
        : undefined}
      onSubmit={handleSubmit}
      questions={designQuestions}
      ref={cardRootRef}
      state={componentState}
      statusLabel={timedOut
        ? t('toolCards.askUser.timeout')
        : showCompletedSummary ? undefined : statusText}
      submitDisabled={!isAllAnswered() || isSubmitted || Boolean(isParamsStreaming)}
      submitLabel={showSubmit ? t('toolCards.askUser.submit') : undefined}
      submittingLabel={t('toolCards.askUser.submitting')}
      submitTitle={!isAllAnswered()
        ? t('toolCards.askUser.answerAllBeforeSubmit')
        : undefined}
      summaryDetail={showCompletedSummary ? answersSummary : undefined}
      summaryLabel={showCompletedSummary
        ? t('toolCards.askUser.questionsAnswered', { count: questions.length })
        : undefined}
    />
  );
};
