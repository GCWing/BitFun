/**
 * AskUserQuestion tool card component
 * Displays multiple questions, collects user answers and submits them
 */

import React, { useState, useCallback, useEffect, useMemo, useLayoutEffect, useRef } from 'react';
import { Loader2, AlertCircle, ArrowUp, ChevronDown, ChevronRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { FlowToolItem, ToolCardProps } from '../types/flow-chat';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { createLogger } from '@/shared/utils/logger';
import { Button, Tooltip } from '@/component-library';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { SmoothHeightCollapse } from '../components/modern/SmoothHeightCollapse';
import {
  askUserQuestionDraftKey,
  askUserQuestionDraftStore,
  createEmptyAskUserQuestionDraft,
  useAskUserQuestionDraftStore,
  type AskUserQuestionSubmissionPhase,
} from '../store/askUserQuestionDraftStore';
import { getActiveSurfaceId } from '@/infrastructure/peer-device/deviceSurface';
import './AskUserQuestionCard.scss';

const log = createLogger('AskUserQuestionCard');

interface QuestionOption {
  label: string;
  description: string;
}

interface QuestionData {
  question: string;
  header: string;
  options: QuestionOption[];
  multiSelect: boolean;
}

/** Renders option description with tooltip for truncated text */
const OptionDescription: React.FC<{ description: string }> = ({ description }) => {
  const descRef = useRef<HTMLDivElement>(null);
  const [isTruncated, setIsTruncated] = useState(false);

  useLayoutEffect(() => {
    const el = descRef.current;
    if (el) {
      setIsTruncated(el.scrollWidth > el.clientWidth);
    }
  }, [description]);

  const descElement = (
    <div ref={descRef} className="option-description">{description}</div>
  );

  if (isTruncated) {
    return (
      <Tooltip content={description} placement="top" delay={300}>
        {descElement}
      </Tooltip>
    );
  }

  return descElement;
};

function normalizeQuestionsFromParams(input: unknown): QuestionData[] {
  if (!input || typeof input !== 'object') return [];
  const raw = input as Record<string, unknown>;
  const qs = raw.questions;
  if (!Array.isArray(qs)) return [];
  return qs.map((q: any) => ({
    question: q.question || '',
    header: q.header || '',
    options: Array.isArray(q.options) ? q.options : [],
    multiSelect: Boolean(q.multiSelect),
  }));
}

/** Same source as FileOperationToolCard: partial JSON while streaming, then final toolCall.input. */
function isAwaitingQuestionPayload(
  questionsLength: number,
  isParamsStreaming: boolean | undefined,
  status: FlowToolItem['status']
): boolean {
  if (questionsLength > 0) return false;
  if (isParamsStreaming) return true;
  const s = status as string;
  return (
    status === 'preparing' ||
    status === 'streaming' ||
    status === 'pending' ||
    s === 'receiving'
  );
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
    [paramsSource]
  );

  const awaitingPayload = isAwaitingQuestionPayload(
    questions.length,
    isParamsStreaming,
    status
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
  const storedDraft = useAskUserQuestionDraftStore(state => (
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
    const shouldCompactCompleted =
      status === 'completed' &&
      isLastItem !== true &&
      !showCompletedSummary;

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
    setLocalDraft(current => ({
      ...current,
      submissionPhase: phase,
      updatedAt: Date.now(),
    }));
  }, [draftKey]);

  const isAllAnswered = useCallback(() => {
    if (questions.length === 0) return false;
    
    for (let i = 0; i < questions.length; i++) {
      const answer = answers[i];
      if (!answer) return false;
      const otherInput = otherInputs[i]?.trim() || '';
      if (
        Array.isArray(answer)
        && !answer.some(value => value !== 'Other' || otherInput.length > 0)
      ) return false;
      if (typeof answer === 'string' && answer === '') return false;
      if (answer === 'Other' && otherInput.length === 0) return false;
    }
    return true;
  }, [answers, otherInputs, questions.length]);

  const handleSingleChange = useCallback((questionIndex: number, value: string) => {
    if (draftKey) {
      askUserQuestionDraftStore.getState().setSingleAnswer(draftKey, questionIndex, value);
      return;
    }
    setLocalDraft(current => ({
      ...current,
      answers: {
        ...current.answers,
        [questionIndex]: value,
      },
      updatedAt: Date.now(),
    }));
  }, [draftKey]);

  const handleMultiChange = useCallback((questionIndex: number, value: string, checked: boolean) => {
    if (draftKey) {
      askUserQuestionDraftStore.getState().setMultiAnswer(
        draftKey,
        questionIndex,
        value,
        checked,
      );
      return;
    }
    setLocalDraft(current => {
      const currentAnswer = current.answers[questionIndex];
      const currentValues = Array.isArray(currentAnswer) ? currentAnswer : [];
      const nextValues = checked
        ? (currentValues.includes(value) ? currentValues : [...currentValues, value])
        : currentValues.filter(candidate => candidate !== value);
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

  const handleOtherInputChange = useCallback((questionIndex: number, value: string) => {
    if (draftKey) {
      askUserQuestionDraftStore.getState().setOtherInput(draftKey, questionIndex, value);
      return;
    }
    setLocalDraft(current => {
      const isEmpty = value.trim().length === 0;
      const currentAnswer = current.answers[questionIndex];
      let nextAnswers = current.answers;
      if (isEmpty && Array.isArray(currentAnswer) && currentAnswer.includes('Other')) {
        nextAnswers = {
          ...current.answers,
          [questionIndex]: currentAnswer.filter(answer => answer !== 'Other'),
        };
      } else if (isEmpty && currentAnswer === 'Other') {
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
      
      for (let i = 0; i < questions.length; i++) {
        const answer = answers[i];
        const otherInput = otherInputs[i]?.trim() || '';
        
        if (Array.isArray(answer)) {
          processedAnswers[String(i)] = answer.flatMap(value => (
            value === 'Other'
              ? (otherInput ? [otherInput] : [])
              : [value]
          ));
        } else if (answer === 'Other') {
          if (otherInput) {
            processedAnswers[String(i)] = otherInput;
          }
        } else {
          processedAnswers[String(i)] = answer;
        }
      }

      const answersPayload = processedAnswers;
      
      await toolAPI.submitUserAnswers(toolId, answersPayload);
      
      setSubmissionPhase('submitted');
    } catch (error) {
      log.error('Failed to submit answers', { toolId, error });
      setSubmissionPhase('idle');
    }
  }, [answers, isAllAnswered, isSubmitted, isSubmitting, otherInputs, questions.length, setSubmissionPhase, toolId]);

  const getStatusIcon = () => {
    if (status === 'completed') {
      return null;
    }
    if (isSubmitting) {
      return <Loader2 size={16} className="status-icon-loading animate-spin" />;
    }
    return <AlertCircle size={16} className="status-icon-waiting" />;
  };

  const getStatusText = () => {
    if (status === 'completed') return t('toolCards.askUser.completed');
    if (isSubmitted) return t('toolCards.askUser.submittedWaiting');
    if (isSubmitting) return t('toolCards.askUser.submitting');
    return t('toolCards.askUser.waitingAnswer');
  };

  const getEffectiveAnswer = useCallback((questionIndex: number): string | string[] | undefined => {
    const localAnswer = answers[questionIndex];
    if (localAnswer !== undefined) return localAnswer;

    if (status === 'completed' && toolResult?.result) {
      const result = typeof toolResult.result === 'string'
        ? JSON.parse(toolResult.result)
        : toolResult.result;
      return result?.answers?.[String(questionIndex)];
    }
    return undefined;
  }, [answers, status, toolResult]);

  const renderQuestion = (q: QuestionData, questionIndex: number) => {
    const answer = getEffectiveAnswer(questionIndex);
    const otherInput = otherInputs[questionIndex] || '';
    
    const isOtherSelected = q.multiSelect 
      ? Array.isArray(answer) && answer.includes('Other')
      : answer === 'Other';

    const inputName = `question-${questionIndex}`;

    return (
      <div data-bf-component="ask-user-question-card" data-bf-part="question" key={questionIndex} className="ask-question-item">
        <div className="question-item-header" data-bf-component="ask-user-question-card" data-bf-part="header">
          <span className="question-header-chip">{q.header}</span>
          <span className="question-text">{q.question}</span>
        </div>
        
        <div className="question-options" data-bf-component="ask-user-question-card" data-bf-part="options">
          {q.options.map((option, optIdx) => (
            <label key={optIdx} className="option-label" data-bf-component="ask-user-question-card" data-bf-part="option">
              {q.multiSelect ? (
                <>
                  <input
                    type="checkbox"
                    name={inputName}
                    value={option.label}
                    checked={Array.isArray(answer) && answer.includes(option.label)}
                    onChange={(e) => handleMultiChange(questionIndex, option.label, e.target.checked)}
                    disabled={isSubmitted || status === 'completed' || Boolean(isParamsStreaming)}
                  />
                  <span className="custom-checkbox" />
                </>
              ) : (
                <>
                  <input
                    type="radio"
                    name={inputName}
                    value={option.label}
                    checked={answer === option.label}
                    onChange={(e) => handleSingleChange(questionIndex, e.target.value)}
                    disabled={isSubmitted || status === 'completed' || Boolean(isParamsStreaming)}
                  />
                  <span className="custom-radio" />
                </>
              )}
              <div className="option-content">
                <div className="option-label-text">{option.label}</div>
                <OptionDescription description={option.description} />
              </div>
            </label>
          ))}
          
          {!isOtherSelected ? (
            <label className="option-label option-other" data-bf-component="ask-user-question-card" data-bf-part="option">
              {q.multiSelect ? (
                <>
                  <input
                    type="checkbox"
                    name={inputName}
                    value="Other"
                    checked={false}
                    onChange={(e) => {
                      if (e.target.checked) {
                        handleMultiChange(questionIndex, 'Other', true);
                      }
                    }}
                    disabled={isSubmitted || status === 'completed' || Boolean(isParamsStreaming)}
                  />
                  <span className="custom-checkbox" />
                </>
              ) : (
                <>
                  <input
                    type="radio"
                    name={inputName}
                    value="Other"
                    checked={false}
                    onChange={() => handleSingleChange(questionIndex, 'Other')}
                    disabled={isSubmitted || status === 'completed' || Boolean(isParamsStreaming)}
                  />
                  <span className="custom-radio" />
                </>
              )}
              <div className="option-content">
                <div className="option-label-text">{t('toolCards.askUser.other')}</div>
                <div className="option-description">{t('toolCards.askUser.customInputHint')}</div>
              </div>
            </label>
          ) : (
            <div className="option-other-input" data-bf-component="ask-user-question-card" data-bf-part="customInput">
              {q.multiSelect ? (
                <>
                  <input
                    type="checkbox"
                    name={inputName}
                    value="Other"
                    checked={true}
                    onChange={(e) => {
                      if (!e.target.checked) {
                        handleMultiChange(questionIndex, 'Other', false);
                      }
                    }}
                    disabled={isSubmitted || status === 'completed' || Boolean(isParamsStreaming)}
                  />
                  <span className="custom-checkbox" />
                </>
              ) : (
                <>
                  <input
                    type="radio"
                    name={inputName}
                    value="Other"
                    checked={true}
                    onChange={() => {}}
                    disabled={isSubmitted || status === 'completed' || Boolean(isParamsStreaming)}
                  />
                  <span className="custom-radio" />
                </>
              )}
              <input
                type="text"
                className="other-input-inline"
                placeholder={t('toolCards.askUser.pleaseSpecify')}
                value={otherInput}
                onChange={(e) => handleOtherInputChange(questionIndex, e.target.value)}
                disabled={isSubmitted || status === 'completed' || Boolean(isParamsStreaming)}
                autoFocus
              />
            </div>
          )}
        </div>
      </div>
    );
  };

  const getAnswerDisplay = (questionIndex: number): string => {
    const answer = getEffectiveAnswer(questionIndex);
    const otherInput = otherInputs[questionIndex] || '';
    
    if (!answer) return '';
    if (Array.isArray(answer)) {
      return answer.map(value => value === 'Other' ? otherInput || 'Other' : value).join(', ');
    }
    return answer === 'Other' ? otherInput || 'Other' : String(answer);
  };

  const getAnswersSummary = (): string => {
    return questions.map((q, idx) => {
      const answerText = getAnswerDisplay(idx);
      return `${q.header}: ${answerText || t('toolCards.askUser.notAnswered')}`;
    }).join(' | ');
  };

  const renderResult = () => {
    if (!toolResult?.result) return null;
    
    const result = typeof toolResult.result === 'string' 
      ? JSON.parse(toolResult.result) 
      : toolResult.result;
    
    if (result.status === 'timeout') {
      return (
        <div data-bf-component="ask-user-question-card" data-bf-part="status" className="result-timeout">
          <AlertCircle size={16} />
          <span>{t('toolCards.askUser.timeout')}</span>
        </div>
      );
    }
    
    return null;
  };

  if (awaitingPayload) {
    return (
      <div data-bf-component="ask-user-question-card" data-bf-part="loading" data-bf-state="loading"
        ref={cardRootRef}
        data-tool-card-id={toolId ?? ''}
        className={`ask-user-question-card params-loading status-${status}`}
      >
        <div className="params-loading-row">
          <Loader2 size={16} className="status-icon-loading animate-spin" />
          <span className="params-loading-text">{t('toolCards.askUser.loadingQuestions')}</span>
        </div>
      </div>
    );
  }

  if (questions.length === 0) {
    return (
      <div data-bf-component="ask-user-question-card" data-bf-part="root" data-bf-state="error" className="ask-user-question-card status-error">
        <div className="error-message" data-bf-component="ask-user-question-card" data-bf-part="error">{t('toolCards.askUser.parseError')}</div>
      </div>
    );
  }

  return (
    <div data-bf-component="ask-user-question-card" data-bf-part="root"
      data-bf-state={status === 'completed' ? 'completed' : undefined}
      ref={cardRootRef}
      data-tool-card-id={toolId ?? ''}
      className={`ask-user-question-card status-${status}`}
    >
      {!showCompletedSummary ? (
        <>
          <div className="card-header-row" data-bf-component="ask-user-question-card" data-bf-part="header">
            <div className="card-title">
              <span className="questions-count">{t('toolCards.askUser.questionsCount', { count: questions.length })}</span>
            </div>
          </div>

          <div className="questions-container" data-bf-component="ask-user-question-card" data-bf-part="questions">
            {questions.map((q, idx) => renderQuestion(q, idx))}
          </div>

          <div className="card-footer-row" data-bf-component="ask-user-question-card" data-bf-part="footer">
            <div className="footer-actions">
              <Button
                variant="primary"
                size="small"
                className="submit-button"
                onClick={handleSubmit}
                disabled={!isAllAnswered() || isSubmitting || isSubmitted || Boolean(isParamsStreaming)}
                isLoading={isSubmitting}
                title={!isAllAnswered() ? t('toolCards.askUser.answerAllBeforeSubmit') : ""}
              >
                {isSubmitting ? (
                  <span>{t('toolCards.askUser.submitting')}</span>
                ) : (
                  <>
                    <ArrowUp size={14} />
                    <span>{t('toolCards.askUser.submit')}</span>
                  </>
                )}
              </Button>
              <div className="tool-status" data-bf-component="ask-user-question-card" data-bf-part="status">
                {getStatusIcon()}
                <span className="status-text">{getStatusText()}</span>
              </div>
            </div>
          </div>
        </>
      ) : (
        <>
          <div 
            className="completed-summary"
            data-bf-component="ask-user-question-card"
            data-bf-part="summary"
            onClick={() => applyExpandedState(isExpanded, !isExpanded, setIsExpanded)}
          >
            <div className="summary-content">
              {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
              <span className="summary-questions-count">{t('toolCards.askUser.questionsAnswered', { count: questions.length })}</span>
              <span className="summary-arrow">→</span>
              <span className="summary-answer">{getAnswersSummary()}</span>
            </div>
            <div className="tool-status">
              {getStatusIcon()}
              <span className="status-text">{getStatusText()}</span>
            </div>
          </div>

          <SmoothHeightCollapse
            isOpen={isExpanded}
            className="ask-user-question-card__answers-collapse"
          >
            <div className="questions-container expanded" data-bf-component="ask-user-question-card" data-bf-part="questions">
              {questions.map((q, idx) => renderQuestion(q, idx))}
            </div>
          </SmoothHeightCollapse>

          {renderResult()}
        </>
      )}
    </div>
  );
};
