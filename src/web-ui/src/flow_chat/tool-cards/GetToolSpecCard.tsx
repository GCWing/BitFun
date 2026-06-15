import React, { useCallback, useMemo, useState } from 'react';
import { Info } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import './GetToolSpecCard.scss';

interface ParsedGetToolSpecResult {
  toolName: string;
  description: string | null;
  inputSchema: unknown;
  alreadyLoaded: boolean;
}

function parseGetToolSpecResult(toolItem: ToolCardProps['toolItem']): ParsedGetToolSpecResult | null {
  const result = toolItem.toolResult?.result;
  const toolName = result?.tool_name || toolItem.toolCall?.input?.tool_name || '';

  if (!toolName && !result) {
    return null;
  }

  return {
    toolName,
    description: typeof result?.description === 'string' ? result.description : null,
    inputSchema: result?.input_schema,
    alreadyLoaded: result?.already_loaded === true,
  };
}

function stringifySchema(value: unknown): string | null {
  if (value == null) return null;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export const GetToolSpecCard: React.FC<ToolCardProps> = ({
  toolItem,
  onExpand,
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);
  const toolId = toolItem.id ?? toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const parsedResult = useMemo(() => parseGetToolSpecResult(toolItem), [toolItem]);
  const targetToolName = parsedResult?.toolName || toolCall?.input?.tool_name || t('toolCards.getToolSpec.unknownTool');
  const errorMessage = toolResult?.error || t('toolCards.getToolSpec.readFailed');
  const schemaText = useMemo(() => stringifySchema(parsedResult?.inputSchema), [parsedResult?.inputSchema]);
  const hasExpandedDetail = Boolean(parsedResult?.description || schemaText || status === 'error');
  const isExpandable = status === 'completed'
    ? !parsedResult?.alreadyLoaded && hasExpandedDetail
    : status === 'error';

  const handleClick = useCallback(() => {
    if (!isExpandable) return;

    applyExpandedState(isExpanded, !isExpanded, setIsExpanded, {
      onExpand,
    });
  }, [applyExpandedState, isExpandable, isExpanded, onExpand]);

  const renderContent = () => {
    if (status === 'completed') {
      if (parsedResult?.alreadyLoaded) {
        return t('toolCards.getToolSpec.alreadyLoaded', { toolName: targetToolName });
      }
      return t('toolCards.getToolSpec.loaded', { toolName: targetToolName });
    }

    if (status === 'error') {
      return errorMessage;
    }

    if (status === 'running' || status === 'streaming' || status === 'preparing') {
      return t('toolCards.getToolSpec.reading', { toolName: targetToolName });
    }

    if (status === 'pending') {
      return t('toolCards.getToolSpec.preparingRead', { toolName: targetToolName });
    }

    return t('toolCards.getToolSpec.readTitle', { toolName: targetToolName });
  };

  const renderExpandedContent = () => {
    if (status === 'error') {
      return (
        <div className="compact-result-content get-tool-spec-card__content">
          <pre>{errorMessage}</pre>
        </div>
      );
    }

    return (
      <div className="get-tool-spec-card__expanded">
        {parsedResult?.description && (
          <section className="get-tool-spec-card__section">
            <div className="get-tool-spec-card__label">
              {t('toolCards.getToolSpec.descriptionLabel')}
            </div>
            <div className="get-tool-spec-card__description">{parsedResult.description}</div>
          </section>
        )}

        {schemaText && (
          <section className="get-tool-spec-card__section">
            <div className="get-tool-spec-card__label">
              {t('toolCards.getToolSpec.schemaLabel')}
            </div>
            <div className="compact-result-content get-tool-spec-card__content">
              <pre>{schemaText}</pre>
            </div>
          </section>
        )}
      </div>
    );
  };

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <CompactToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleClick}
        className="get-tool-spec-card"
        clickable={isExpandable}
        header={(
          <CompactToolCardHeader
            icon={<ToolCardStatusSlot status={status} toolIcon={<Info size={16} />} />}
            action={t('toolCards.getToolSpec.title')}
            content={renderContent()}
          />
        )}
        expandedContent={isExpandable ? renderExpandedContent() : undefined}
      />
    </div>
  );
};
