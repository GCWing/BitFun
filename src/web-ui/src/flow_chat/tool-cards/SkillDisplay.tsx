/**
 * Skill tool display — compact row (same pattern as Read file).
 */

import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  SkillToolCard,
  type FlowChatToolStatus,
} from '@openbitfun/ui/flow-chat';
import { getSkillSourceLabelFromIdentity } from '@/infrastructure/config/skillSourcePresentation';
import type { ToolCardProps } from '../types/flow-chat';

function nonEmptyString(value: unknown): string | undefined {
  return typeof value === 'string' ? value.trim() || undefined : undefined;
}

export const SkillDisplay: React.FC<ToolCardProps> = React.memo(({ toolItem }) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;

  const skillInfo = useMemo(() => {
    if (!toolResult?.result) return null;
    const result = toolResult.result as Record<string, unknown>;
    const sourceLabel = getSkillSourceLabelFromIdentity(
      nonEmptyString(result.source_label),
      nonEmptyString(result.source_id),
      nonEmptyString(result.source_slot),
      '',
    );
    const displayName = nonEmptyString(result.skill_name)
      || nonEmptyString(result.name)
      || t('toolCards.skill.unknownSkill');
    return {
      displayName,
      sourceLabel,
    };
  }, [toolResult?.result, t]);

  const commandName =
    (toolCall?.input?.command as string | undefined) ||
    (toolCall?.input?.skill_name as string | undefined) ||
    t('toolCards.skill.unknown');

  const displayName = status === 'completed' && skillInfo ? skillInfo.displayName : commandName;
  const completedLabel = skillInfo?.sourceLabel
    ? `${displayName} · ${skillInfo.sourceLabel}`
    : displayName;

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
      return `${t('toolCards.skill.skillAction')} ${completedLabel}`;
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
