import React from 'react';
import { useTranslation } from 'react-i18next';
import { Target } from 'lucide-react';
import { IconButton, Tooltip } from '@/component-library';
import type { ThreadGoalSnapshot } from '../../services/goalService';
import { resolveThreadGoalStatusLabel } from '../../utils/threadGoalDisplay';
import { resolveThreadGoalStripIconTone } from './threadGoalStripIconTone';

export interface ThreadGoalStripButtonProps {
  goal: ThreadGoalSnapshot | null;
  onOpen: () => void;
}

export const ThreadGoalStripButton: React.FC<ThreadGoalStripButtonProps> = ({
  goal,
  onOpen,
}) => {
  const { t } = useTranslation('flow-chat');

  const iconTone = resolveThreadGoalStripIconTone(goal);
  const statusKey = goal?.status ?? 'none';
  const tooltip = goal
    ? t('threadGoal.stripTooltipWithGoal', {
        status: resolveThreadGoalStatusLabel(t, statusKey),
        objective: goal.objective,
      })
    : t('threadGoal.stripTooltipEmpty');

  const ariaLabel = goal ? t('threadGoal.stripOpenWithGoal') : t('threadGoal.stripOpenEmpty');

  return (
    <Tooltip content={tooltip}>
      <IconButton
        className={`bitfun-chat-input-workspace-strip__goal-btn bitfun-chat-input-workspace-strip__goal-btn--${iconTone}`}
        variant="ghost"
        size="xs"
        type="button"
        aria-label={ariaLabel}
        data-testid="thread-goal-strip-button"
        onClick={e => {
          e.stopPropagation();
          onOpen();
        }}
      >
        <Target size={14} strokeWidth={2} aria-hidden />
      </IconButton>
    </Tooltip>
  );
};

ThreadGoalStripButton.displayName = 'ThreadGoalStripButton';
