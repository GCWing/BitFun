import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Target } from 'lucide-react';
import { Button, Modal, Textarea } from '@/component-library';
import type { ThreadGoalController } from '../../hooks/useThreadGoalController';
import type { ThreadGoalUiAction } from '../../services/threadGoalActions';
import {
  buildThreadGoalWorkflowSteps,
  shouldShowThreadGoalWorkflow,
} from './threadGoalWorkflow';
import {
  resolveThreadGoalActionLabel,
  resolveThreadGoalStatusLabel,
} from '../../utils/threadGoalDisplay';
import './ThreadGoalDialogs.scss';

function formatUsageLine(
  goal: NonNullable<ThreadGoalController['goal']>,
  t: ReturnType<typeof useTranslation>['t']
): string | null {
  const parts: string[] = [];
  if (goal.tokenBudget != null) {
    parts.push(
      t('threadGoal.usageTokens', {
        used: goal.tokensUsed ?? 0,
        budget: goal.tokenBudget,
      })
    );
  }
  if ((goal.timeUsedSeconds ?? 0) > 0) {
    parts.push(
      t('threadGoal.usageTime', {
        seconds: goal.timeUsedSeconds,
      })
    );
  }
  return parts.length > 0 ? parts.join(' · ') : null;
}

function statusBadgeClass(status: string): string {
  const known = new Set([
    'active',
    'paused',
    'blocked',
    'usageLimited',
    'budgetLimited',
    'complete',
  ]);
  const key = known.has(status) ? status : 'active';
  return `bitfun-thread-goal-menu__status-badge bitfun-thread-goal-menu__status-badge--${key}`;
}

export interface ThreadGoalDialogsProps {
  controller: ThreadGoalController;
  disabled?: boolean;
}

export const ThreadGoalDialogs: React.FC<ThreadGoalDialogsProps> = ({
  controller,
  disabled = false,
}) => {
  const { t } = useTranslation('flow-chat');
  const { goal } = controller;
  const [draft, setDraft] = useState(controller.editInitialObjective);

  useEffect(() => {
    if (controller.editOpen) {
      setDraft(controller.editInitialObjective);
    }
  }, [controller.editInitialObjective, controller.editOpen]);

  const statusLabel = goal ? resolveThreadGoalStatusLabel(t, goal.status) : '';

  const usageLine = goal ? formatUsageLine(goal, t) : null;

  const workflowSteps = useMemo(
    () => (goal ? buildThreadGoalWorkflowSteps(goal.status) : []),
    [goal]
  );

  const showWorkflow = goal ? shouldShowThreadGoalWorkflow(goal.status) : false;

  const workflowNote = goal
    ? t(`threadGoal.workflow.note.${goal.status}`, {
        defaultValue: '',
      }).trim() || null
    : null;

  const runAction = useCallback(
    async (action: ThreadGoalUiAction) => {
      if (disabled) return;
      if (action === 'edit') {
        controller.openEdit(goal ? 'update' : 'create');
        return;
      }
      if (action === 'set') {
        controller.openEdit('create');
        return;
      }
      if (action === 'clear' || action === 'pause' || action === 'resume') {
        await controller.runUiAction(action);
      }
    },
    [controller, disabled, goal]
  );

  const commandHint = goal
    ? t(`threadGoal.commandHint.${goal.status}`, {
        defaultValue: t('threadGoal.commandHint.default'),
      })
    : t('threadGoal.commandHint.none');

  return (
    <>
      <Modal
        isOpen={controller.menuOpen}
        onClose={controller.closeMenu}
        title={t('threadGoal.menuTitle')}
        size="medium"
        contentInset
        contentClassName="bitfun-thread-goal-modal__body"
      >
        {goal ? (
          <div className="bitfun-thread-goal-menu">
            <div className="bitfun-thread-goal-menu__header">
              <span className={statusBadgeClass(goal.status)}>
                <Target size={14} aria-hidden />
                {statusLabel}
              </span>
              {usageLine ? (
                <p className="bitfun-thread-goal-menu__usage">{usageLine}</p>
              ) : null}
            </div>

            <section className="bitfun-thread-goal-menu__section" aria-labelledby="thread-goal-objective">
              <h3 id="thread-goal-objective" className="bitfun-thread-goal-menu__section-title">
                {t('threadGoal.objectiveLabel')}
              </h3>
              <p className="bitfun-thread-goal-menu__objective">{goal.objective}</p>
            </section>

            {showWorkflow ? (
              <section
                className="bitfun-thread-goal-menu__section"
                aria-labelledby="thread-goal-workflow"
              >
                <h3 id="thread-goal-workflow" className="bitfun-thread-goal-menu__section-title">
                  {t('threadGoal.workflow.title')}
                </h3>
                <ol className="bitfun-thread-goal-menu__workflow">
                  {workflowSteps.map(step => (
                    <li
                      key={step.id}
                      className={[
                        'bitfun-thread-goal-menu__workflow-step',
                        `bitfun-thread-goal-menu__workflow-step--${step.state}`,
                      ].join(' ')}
                    >
                      <span
                        className={[
                          'bitfun-thread-goal-menu__workflow-marker',
                          `bitfun-thread-goal-menu__workflow-marker--${step.state}`,
                        ].join(' ')}
                        aria-hidden
                      />
                      <span className="bitfun-thread-goal-menu__workflow-text">
                        {t(`threadGoal.workflow.steps.${step.id}`)}
                      </span>
                    </li>
                  ))}
                </ol>
                {workflowNote ? (
                  <p className="bitfun-thread-goal-menu__workflow-note">{workflowNote}</p>
                ) : null}
              </section>
            ) : null}

            <div className="bitfun-thread-goal-menu__footer">
              <div className="bitfun-thread-goal-menu__actions">
                {controller.availableActions.map(action => (
                  <Button
                    key={action}
                    type="button"
                    variant={action === 'clear' ? 'danger' : 'secondary'}
                    size="small"
                    disabled={disabled}
                    onClick={() => void runAction(action)}
                  >
                    {resolveThreadGoalActionLabel(t, action)}
                  </Button>
                ))}
              </div>
              <p className="bitfun-thread-goal-menu__hint">{commandHint}</p>
            </div>
          </div>
        ) : (
          <div className="bitfun-thread-goal-menu bitfun-thread-goal-menu--empty">
            <p className="bitfun-thread-goal-menu__hint">{t('threadGoal.menuEmpty')}</p>
            <div className="bitfun-thread-goal-menu__actions">
              <Button
                type="button"
                variant="primary"
                size="small"
                disabled={disabled}
                onClick={() => controller.openEdit('create')}
              >
                {t('threadGoal.action.set')}
              </Button>
            </div>
          </div>
        )}
      </Modal>

      <Modal
        isOpen={controller.editOpen}
        onClose={controller.closeEdit}
        title={
          controller.editMode === 'create'
            ? t('threadGoal.editTitleCreate')
            : t('threadGoal.editTitleUpdate')
        }
        size="medium"
        contentInset
        contentClassName="bitfun-thread-goal-modal__body"
      >
        <div className="bitfun-thread-goal-edit">
          <p className="bitfun-thread-goal-edit__hint">{t('threadGoal.editHint')}</p>
          <Textarea
            value={draft}
            onChange={e => setDraft(e.target.value)}
            rows={4}
            autoFocus
            disabled={disabled}
            placeholder={t('threadGoal.editPlaceholder')}
          />
          <div className="bitfun-thread-goal-edit__actions">
            <Button type="button" variant="ghost" size="small" onClick={controller.closeEdit}>
              {t('threadGoal.editCancel')}
            </Button>
            <Button
              type="button"
              variant="primary"
              size="small"
              disabled={disabled || !draft.trim()}
              onClick={() => void controller.saveEdit(draft)}
            >
              {t('threadGoal.editSave')}
            </Button>
          </div>
        </div>
      </Modal>

      <Modal
        isOpen={controller.resumeOpen}
        onClose={controller.dismissResume}
        title={t('threadGoal.resumeTitle')}
        size="medium"
        contentInset
        contentClassName="bitfun-thread-goal-modal__body"
      >
        <div className="bitfun-thread-goal-resume">
          <p className="bitfun-thread-goal-resume__subtitle">
            {t('threadGoal.resumeSubtitle', { objective: goal?.objective ?? '' })}
          </p>
          <p className="bitfun-thread-goal-resume__hint">{t('threadGoal.resumeHint')}</p>
          <div className="bitfun-thread-goal-resume__actions">
            <Button
              type="button"
              variant="ghost"
              size="small"
              disabled={disabled}
              onClick={controller.dismissResume}
            >
              {t('threadGoal.resumeLeavePaused')}
            </Button>
            <Button
              type="button"
              variant="primary"
              size="small"
              disabled={disabled}
              onClick={() => void controller.confirmResume()}
            >
              {t('threadGoal.resumeConfirm')}
            </Button>
          </div>
        </div>
      </Modal>
    </>
  );
};

ThreadGoalDialogs.displayName = 'ThreadGoalDialogs';
