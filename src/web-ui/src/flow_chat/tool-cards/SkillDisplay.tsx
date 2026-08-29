/**
 * Skill tool display — compact row (same pattern as Read file).
 */

import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  SkillToolCard,
  type FlowChatToolStatus,
} from '@bitfun/ui/flow-chat';
import type { ToolCardProps } from '../types/flow-chat';

export const SkillDisplay: React.FC<ToolCardProps> = React.memo(({ toolItem }) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;

  const skillInfo = useMemo(() => {
    if (!toolResult?.result) return null;
    const result = toolResult.result as Record<string, unknown>;
    return {
      name: (result.skill_name || result.name || t('toolCards.skill.unknownSkill')) as string,
    };
  }, [toolResult?.result, t]);

  const commandName =
    (toolCall?.input?.command as string | undefined) ||
    (toolCall?.input?.skill_name as string | undefined) ||
    t('toolCards.skill.unknown');

  const displayName = status === 'completed' && skillInfo ? skillInfo.name : commandName;

  const getErrorMessage = () => {
    if (toolResult && 'error' in toolResult && toolResult.error) {
      return String(toolResult.error);
    }
    return t('toolCards.skill.loadSkillFailed');
  };

  const renderContent = () => {
    if (status === 'error') {
      return `${getErrorMessage()}${commandName ? ` ${commandName}` : ''}`;
    }
    if (status === 'completed') {
      return `${t('toolCards.skill.skillAction')} ${displayName}`;
    }
    if (status === 'running' || status === 'streaming' || status === 'preparing') {
      return `${t('toolCards.skill.loadingSkill')} ${displayName}...`;
    }
    if (status === 'pending') {
      return `${t('toolCards.skill.preparingSkill')} ${displayName}`;
    }
    return `${t('toolCards.skill.skillAction')} ${displayName}`;
  };

  return (
    <SkillToolCard
      status={status as FlowChatToolStatus}
      summary={renderContent()}
    />
  );
});
