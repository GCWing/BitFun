import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import type { ToolCardProps } from '../types/flow-chat';
import { GetToolSpecToolCard } from '@openbitfun/ui/flow-chat';

interface ParsedGetToolSpecResult {
  toolName: string;
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
    alreadyLoaded: result?.already_loaded === true,
  };
}

export const GetToolSpecCard: React.FC<ToolCardProps> = ({ toolItem }) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;
  const toolId = toolItem.id ?? toolCall?.id;

  const parsedResult = useMemo(() => parseGetToolSpecResult(toolItem), [toolItem]);
  const targetToolName = parsedResult?.toolName || toolCall?.input?.tool_name || t('toolCards.getToolSpec.unknownTool');
  const errorMessage = toolResult?.error || t('toolCards.getToolSpec.readFailed');

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

  return (
    <GetToolSpecToolCard
      action={t('toolCards.getToolSpec.title')}
      data-tool-card-id={toolId ?? ''}
      status={status}
      summary={renderContent()}
    />
  );
};
