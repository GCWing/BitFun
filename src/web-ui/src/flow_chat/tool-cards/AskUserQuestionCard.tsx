/**
 * AskUserQuestion tool card component
 * Displays multiple questions, collects user answers and submits them
 */

import React, { useState, useCallback, useMemo, useLayoutEffect, useRef } from 'react';
import { Loader2, AlertCircle, ArrowUp, ChevronDown, ChevronRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { FlowToolItem, ToolCardProps } from '../types/flow-chat';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { createLogger } from '@/shared/utils/logger';
import { Button, Tooltip } from '@/component-library';
import { useToolCardHeightContract } from './useToolCardHeightContract';
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
  toolItem
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
  
  const [answers, setAnswers] = useState<Record<number, string | string[]>>({});
  const [otherInputs, setOtherInputs] = useState<Record<number, string>>({});
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isSubmitted, setIsSubmitted] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [showCompletedSummary, setShowCompletedSummary] = useState(status === 'completed');
  const toolId = toolItem.id ?? toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });
  const previousStatusRef = useRef(status);

  useLayoutEffect(() => {
    const previousStatus = previousStatusRef.current;
    previousStatusRef.current = status;

    if (previousStatus !== 'completed' && status === 'completed' && !showCompletedSummary) {
      applyExpandedState(true, false, (nextExpanded) => {
        setShowCompletedSummary(!nextExpanded);
      }, {
        reason: 'auto',
      });
      return;
    }

    if (status !== 'completed' && showCompletedSummary) {
      setShowCompletedSummary(false);
    }
  }, [applyExpandedState, showCompletedSummary, status]);

  const isAllAnswered = useCallback(() => {
    if (questions.length === 0) return false;
    
    for (let i = 0; i < questions.length; i++) {
      const answer = answers[i];
      if (!answer) return false;
      if (Array.isArray(answer) && answer.length === 0) return false;
      if (typeof answer === 'string' && answer === '') return false;
    }
    return true;
  }, [answers, questions.length]);

  const handleSingleChange = useCallback((questionIndex: number, value: string) => {
    setAnswers(prev => ({
      ...prev,
      [questionIndex]: value
    }));
  }, []);

  const handleMultiChange = useCallback((questionIndex: number, value: string, checked: boolean) => {
    setAnswers(prev => {
      const current = prev[questionIndex];
      const currentArray = Array.isArray(current) ? current : [];
      
      if (checked) {
        return { ...prev, [questionIndex]: [...currentArray, value] };
      } else {
        return { ...prev, [questionIndex]: currentArray.filter(v => v !== value) };
      }
    });
  }, []);

  const handleOtherInputChange = useCallback((questionIndex: number, value: string) => {
    setOtherInputs(prev => ({
      ...prev,
      [questionIndex]: value
    }));
  }, []);

  const handleSubmit = useCallback(async () => {
    if (!isAllAnswered() || isSubmitting || isSubmitted) return;

    const toolId = toolItem.id;
    setIsSubmitting(true);
    try {
      const processedAnswers: Record<string, string | string[]> = {};
      
      for (let i = 0; i < questions.length; i++) {
        const answer = answers[i];
        const otherInput = otherInputs[i] || '';
        
        if (Array.isArray(answer)) {
          processedAnswers[String(i)] = answer.map(v => 
            v === 'Other' ? (otherInput || 'Other') : v
          );
        } else {
          processedAnswers[String(i)] = answer === 'Other' ? (otherInput || 'Other') : answer;
        }
      }

      const answersPayload = processedAnswers;
      
      await toolAPI.submitUserAnswers(toolId, answersPayload);
      
      setIsSubmitted(true);
    } catch (error) {
      log.error('Failed to submit answers', { toolId, error });
    } finally {
      setIsSubmitting(false);
    }
  }, [toolItem.id, answers, otherInputs, questions.length, isAllAnswered, isSubmitting, isSubmitted]);

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
      <div key={questionIndex} className="ask-question-item">
        <div className="question-item-header">
          <span className="question-header-chip">{q.header}</span>
          <span className="question-text">{q.question}</span>
        </div>
        
        <div className="question-options">
          {q.options.map((option, optIdx) => (
            <label key={optIdx} className="option-label">
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
            <label className="option-label option-other">
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
            <div className="option-other-input">
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
      return answer.map(v => v === 'Other' ? otherInput || 'Other' : v).join(', ');
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
        <div className="result-timeout">
          <AlertCircle size={16} />
          <span>{t('toolCards.askUser.timeout')}</span>
        </div>
      );
    }
    
    return null;
  };

  if (awaitingPayload) {
    return (
      <div
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
      <div className="ask-user-question-card status-error">
        <div className="error-message">{t('toolCards.askUser.parseError')}</div>
      </div>
    );
  }

  return (
    <div
      ref={cardRootRef}
      data-tool-card-id={toolId ?? ''}
      className={`ask-user-question-card status-${status}`}
    >
      {!showCompletedSummary ? (
        <>
          <div className="card-header-row">
            <div className="card-title">
              <span className="questions-count">{t('toolCards.askUser.questionsCount', { count: questions.length })}</span>
            </div>
          </div>

          <div className="questions-container">
            {questions.map((q, idx) => renderQuestion(q, idx))}
          </div>

          <div className="card-footer-row">
            <div className="footer-actions">
              <Button
                variant="primary"
                size="small"
                className="submit-button"
                onClick={handleSubmit}
                disabled={!isAllAnswered() || isSubmitting || Boolean(isParamsStreaming)}
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
              <div className="tool-status">
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

          {isExpanded && (
            <div className="questions-container expanded">
              {questions.map((q, idx) => renderQuestion(q, idx))}
            </div>
          )}

          {renderResult()}
        </>
      )}
    </div>
  );
};
