/**
 * Run Code tool card.
 *
 * Code-mode agents answer a step by writing one program — DeepSeek Harness's
 * PTC preset calls it `run_code` — instead of one tool call per file read or
 * shell command. The program is the action, so the card shows it as code and
 * whatever it printed underneath, rather than squeezing a TypeScript source
 * file into a terminal card's command line.
 */

import React, { useCallback, useMemo, useState } from 'react';
import { Code2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { ToolCardProps } from '../types/flow-chat';
import { CodePreview } from '../components/CodePreview';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';
import { ToolCardCopyAction, ToolCardHeaderActions } from './ToolCardHeaderActions';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { formatSessionViewPreviewText } from '../utils/sessionViewPreview';
import './RunCodeToolCard.scss';

/** Enough of the program to recognize it in a collapsed header. */
const SUMMARY_CHARS = 80;

function asText(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

/**
 * The first line that says something about what the program does — skipping
 * the comment banners and blank lines a model tends to open with.
 */
function firstMeaningfulLine(code: string): string {
  for (const line of code.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('//') || trimmed.startsWith('/*') || trimmed.startsWith('*')) {
      continue;
    }
    return trimmed.length > SUMMARY_CHARS ? `${trimmed.slice(0, SUMMARY_CHARS)}...` : trimmed;
  }
  return '';
}

function readOutput(result: unknown): string {
  if (typeof result === 'string') return formatSessionViewPreviewText(result);
  if (!result || typeof result !== 'object') return '';

  const record = result as Record<string, unknown>;
  const stdout = asText(record.stdout);
  const stderr = asText(record.stderr);
  const combined = [stdout, stderr].filter((value) => value.length > 0).join('\n');
  return formatSessionViewPreviewText(asText(record.output) || combined);
}

export const RunCodeToolCard: React.FC<ToolCardProps> = ({
  toolItem,
  config,
  onExpand,
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);
  const toolId = toolItem.id ?? toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: config.toolName,
  });

  const code = asText(toolCall?.input?.code);
  const description = asText(toolCall?.input?.description).trim();
  const output = useMemo(() => readOutput(toolResult?.result), [toolResult?.result]);
  const errorMessage = status === 'error'
    ? toolResult?.error || t('toolCards.runCode.failed')
    : null;

  const canExpand = Boolean(code || output || errorMessage);
  const getCopyCodeText = useCallback(() => code, [code]);

  const handleToggleExpand = useCallback(() => {
    if (!canExpand) return;

    applyExpandedState(isExpanded, !isExpanded, setIsExpanded, { onExpand });
  }, [applyExpandedState, canExpand, isExpanded, onExpand]);

  const summary = useMemo(() => {
    if (errorMessage) return errorMessage;

    const subject = description || firstMeaningfulLine(code);
    if (status === 'completed') {
      return subject || t('toolCards.runCode.completed');
    }
    if (status === 'cancelled') {
      return t('toolCards.runCode.cancelled');
    }
    if (status === 'rejected') {
      return t('toolCards.runCode.rejected');
    }
    if (subject) return subject;

    return code ? t('toolCards.runCode.running') : t('toolCards.runCode.writing');
  }, [code, description, errorMessage, status, t]);

  return (
    <div
      data-bf-component="run-code-tool-card"
      data-bf-part="root"
      data-bf-state={[isExpanded && 'expanded', Boolean(errorMessage) && 'failed'].filter(Boolean).join(' ')}
      ref={cardRootRef}
      data-tool-card-id={toolId ?? ''}
    >
      <CompactToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleToggleExpand}
        className="run-code-tool-card"
        clickable={canExpand}
        header={(
          <CompactToolCardHeader
            icon={<ToolCardStatusSlot status={status} toolIcon={<Code2 size={16} />} />}
            action={t('toolCards.runCode.title')}
            content={summary}
            extra={code ? (
              <ToolCardHeaderActions className="run-code-tool-card__actions">
                <ToolCardCopyAction
                  className="run-code-tool-card__copy"
                  getText={getCopyCodeText}
                  tooltip={t('toolCards.runCode.copyCode')}
                  copiedTooltip={t('toolCards.runCode.codeCopied')}
                  successMessage={t('toolCards.runCode.codeCopied')}
                  failureMessage={t('toolCards.runCode.copyCodeFailed')}
                  ariaLabel={t('toolCards.runCode.copyCode')}
                  showSuccessNotification={false}
                />
              </ToolCardHeaderActions>
            ) : undefined}
          />
        )}
        expandedContent={canExpand ? (
          <div data-bf-component="run-code-tool-card" data-bf-part="expanded" className="run-code-tool-card__expanded">
            {code && (
              <section data-bf-component="run-code-tool-card" data-bf-part="section" className="run-code-tool-card__section">
                <div data-bf-component="run-code-tool-card" data-bf-part="label" className="run-code-tool-card__label">
                  {t('toolCards.runCode.programLabel')}
                </div>
                <CodePreview
                  content={code}
                  language="typescript"
                  showLineNumbers={false}
                  maxHeight={320}
                  autoScrollToBottom={false}
                />
              </section>
            )}

            {(output || errorMessage) && (
              <section data-bf-component="run-code-tool-card" data-bf-part="section" className="run-code-tool-card__section">
                <div data-bf-component="run-code-tool-card" data-bf-part="label" className="run-code-tool-card__label">
                  {t('toolCards.runCode.outputLabel')}
                </div>
                {errorMessage ? (
                  <div data-bf-component="run-code-tool-card" data-bf-part="error" className="run-code-tool-card__error">{errorMessage}</div>
                ) : (
                  <div data-bf-component="run-code-tool-card" data-bf-part="output" className="compact-result-content run-code-tool-card__output">
                    <pre>{output}</pre>
                  </div>
                )}
              </section>
            )}
          </div>
        ) : undefined}
      />
    </div>
  );
};

export default RunCodeToolCard;
