/**
 * Skill tool display — compact row (same pattern as Read file).
 */

import React, { useMemo } from 'react';
import { Loader2, Clock, Check, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import type { CompactToolCardProps } from './CompactToolCard';

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

  const getStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
      case 'preparing':
        return <Loader2 className="animate-spin" size={16} />;
      case 'completed':
        return <Check size={16} className="icon-check-done" />;
      case 'error':
        return <X size={16} />;
      case 'pending':
      default:
        return <Clock size={16} />;
    }
  };

  const renderContent = () => {
    if (status === 'error') {
      return (
        <>
          {getErrorMessage()}
          {commandName ? <span className="read-file-meta"> {commandName}</span> : null}
        </>
      );
    }
    if (status === 'completed') {
      return (
        <>
          {t('toolCards.skill.skillAction')} {displayName}
        </>
      );
    }
    if (status === 'running' || status === 'streaming' || status === 'preparing') {
      return (
        <>
          {t('toolCards.skill.loadingSkill')} {displayName}...
        </>
      );
    }
    if (status === 'pending') {
      return (
        <>
          {t('toolCards.skill.preparingSkill')} {displayName}
        </>
      );
    }
    return (
      <>
        {t('toolCards.skill.skillAction')} {displayName}
      </>
    );
  };

  return (
    <CompactToolCard
      status={status as CompactToolCardProps['status']}
      isExpanded={false}
      className="skill-tool-compact"
      clickable={false}
      header={
        <CompactToolCardHeader
          icon={getStatusIcon()}
          content={renderContent()}
        />
      }
    />
  );
});
