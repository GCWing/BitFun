/**
 * R-WF-18: workflow-member Claw card.
 *
 * Reuses the AssistantCard skeleton (same data-bf structure and surface
 * tokens) so the independent workflow-Claw list shares the Claw look while
 * keeping its own data source. Naming follows the R-WF-15 convention:
 * workflow word root, no legion.
 */

import React from 'react';
import { Bot, ChevronRight, Settings2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/component-library';
import type { WorkspaceInfo } from '@/shared/types';

interface WorkflowClawCardProps {
  workspace: WorkspaceInfo;
  onClick: () => void;
}

const WorkflowClawCard: React.FC<WorkflowClawCardProps> = ({ workspace, onClick }) => {
  const { t } = useTranslation('scenes/profile');
  const identity = workspace.identity;

  const name = identity?.name?.trim() || workspace.name || t('nursery.card.unnamed');
  const emoji = identity?.emoji?.trim() ?? '';
  const creature = identity?.creature?.trim() || '';

  return (
    <article
      data-bf-component="workflow-claw-card"
      data-bf-part="root"
      className="assistant-card workflow-claw-card"
      role="listitem"
    >
      <button
        data-bf-component="workflow-claw-card"
        data-bf-part="main"
        type="button"
        className="assistant-card__main"
        onClick={onClick}
        aria-label={`${t('nursery.card.configure')}: ${name}`}
      >
        <span className="assistant-card__header" data-bf-component="workflow-claw-card" data-bf-part="header">
          <span className="assistant-card__avatar" data-bf-component="workflow-claw-card" data-bf-part="avatar">
            {emoji ? (
              <span className="assistant-card__emoji">{emoji}</span>
            ) : (
              <Bot className="assistant-card__avatar-icon" size={20} strokeWidth={1.6} aria-hidden="true" />
            )}
          </span>
          <span className="assistant-card__header-info" data-bf-component="workflow-claw-card" data-bf-part="headerInfo">
            <span className="assistant-card__title-row">
              <span className="assistant-card__name" data-bf-component="workflow-claw-card" data-bf-part="name">{name}</span>
            </span>
            {creature ? (
              <span className="assistant-card__badges" data-bf-component="workflow-claw-card" data-bf-part="badges">
                <Badge variant="neutral">{creature}</Badge>
              </span>
            ) : null}
          </span>
          <ChevronRight
            data-bf-component="workflow-claw-card"
            data-bf-part="chevron"
            className="assistant-card__chevron"
            size={16}
            strokeWidth={1.7}
            aria-hidden="true"
          />
        </span>

        <span className="assistant-card__configure" data-bf-component="workflow-claw-card" data-bf-part="configure">
          <Settings2 size={13} strokeWidth={1.8} aria-hidden="true" />
          <span>{t('nursery.card.configure')}</span>
        </span>
      </button>
    </article>
  );
};

export default WorkflowClawCard;
