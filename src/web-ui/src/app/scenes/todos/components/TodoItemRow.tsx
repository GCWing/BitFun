/**
 * One Todo row, shared by the 24-hour list and the calendar day detail.
 *
 * A row represents a single occurrence, so the same recurring job can appear on
 * several rows with different times.
 */

import { Icon, IconButton, Switch, Tooltip } from '@bitfun/ui';
import React from 'react';
import { CalendarClock } from 'lucide-react';

import { useI18n } from '@/infrastructure/i18n';
import type { CronJob } from '@/infrastructure/api';
import type { WorkspaceInfo } from '@/shared/types';
import {
  formatCountdown,
  formatJobTargetLabel,
  formatScheduleSummary,
  formatTimeOfDay,
  resolveJobWorkspaceLabel,
} from '../todoPresentation';

export interface TodoItemRowProps {
  job: CronJob;
  /** Occurrence time, or null for a row in the inactive group. */
  atMs: number | null;
  isOverdue?: boolean;
  isNextRun?: boolean;
  /** The scheduler is executing this run right now. */
  isRunning?: boolean;
  nowMs: number;
  workspaces: WorkspaceInfo[];
  /** Replaces the countdown, used for inactive rows ("Paused", "Done"). */
  statusLabel?: string;
  isSelected?: boolean;
  onEdit: (job: CronJob) => void;
  onDelete: (job: CronJob) => void;
  onToggleEnabled: (job: CronJob, enabled: boolean) => void;
}

const TodoItemRow: React.FC<TodoItemRowProps> = ({
  job,
  atMs,
  isOverdue = false,
  isNextRun = false,
  isRunning = false,
  nowMs,
  workspaces,
  statusLabel,
  isSelected = false,
  onEdit,
  onDelete,
  onToggleEnabled,
}) => {
  const { t, formatDate } = useI18n(['scenes/todos', 'shared']);

  const timeLabel = atMs != null ? formatTimeOfDay(atMs, formatDate) : null;
  const relativeLabel = statusLabel
    ?? (isRunning ? t('shared:statuses.running') : null)
    ?? (atMs != null ? formatCountdown(atMs, nowMs, t) : null);

  const rowState = [
    isSelected ? 'selected' : null,
    isRunning ? 'running' : null,
    isOverdue ? 'overdue' : null,
    job.enabled ? null : 'disabled',
  ].filter(Boolean).join(' ');

  return (
    <div
      className={[
        'bf-todos__row',
        isRunning ? 'bf-todos__row--running' : '',
        isOverdue ? 'bf-todos__row--overdue' : '',
        job.enabled ? '' : 'bf-todos__row--disabled',
        isSelected ? 'bf-todos__row--selected' : '',
      ].filter(Boolean).join(' ')}
      data-bf-scene="todos"
      data-bf-part="row"
      data-bf-state={rowState || undefined}
      data-testid="todos-row"
      role="group"
      tabIndex={0}
      aria-label={`${job.name}${timeLabel ? `, ${timeLabel}` : ''}`}
      onClick={() => onEdit(job)}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          onEdit(job);
        }
      }}
    >
      <div className="bf-todos__row-icon" data-bf-scene="todos" data-bf-part="rowIcon">
        <CalendarClock size={19} aria-hidden="true" />
      </div>

      <div className="bf-todos__row-body" data-bf-scene="todos" data-bf-part="rowBody">
        <div className="bf-todos__row-title-line">
          <span className="bf-todos__row-name">{job.name}</span>
          {isRunning ? (
            <span
              className="bf-todos__row-badge bf-todos__row-badge--running"
              data-bf-scene="todos"
              data-bf-part="rowBadge"
            >
              {t('shared:statuses.running')}
            </span>
          ) : isNextRun ? (
            <span className="bf-todos__row-badge" data-bf-scene="todos" data-bf-part="rowBadge">
              {t('badges.nextRun')}
            </span>
          ) : null}
          {isOverdue ? (
            <span
              className="bf-todos__row-badge bf-todos__row-badge--warn"
              data-bf-scene="todos"
              data-bf-part="rowBadge"
            >
              {t('badges.overdue')}
            </span>
          ) : null}
        </div>
        <div className="bf-todos__row-meta">
          <span>{resolveJobWorkspaceLabel(job, workspaces)}</span>
          <span className="bf-todos__row-meta-sep" aria-hidden="true">·</span>
          <span>{formatScheduleSummary(job.schedule, t, formatDate)}</span>
          <span className="bf-todos__row-meta-sep" aria-hidden="true">·</span>
          <span>{formatJobTargetLabel(job, t)}</span>
          {relativeLabel ? (
            <>
              <span className="bf-todos__row-meta-sep" aria-hidden="true">·</span>
              <span title={timeLabel ?? undefined}>{relativeLabel}</span>
            </>
          ) : null}
        </div>
        {job.state.lastError ? (
          <p className="bf-todos__row-error" data-bf-scene="todos" data-bf-part="rowError">
            {job.state.lastError}
          </p>
        ) : null}
      </div>

      <div
        className="bf-todos__row-actions"
        data-bf-scene="todos"
        data-bf-part="rowActions"
        onClick={(event) => event.stopPropagation()}
        role="presentation"
      >
        <Switch
          checked={job.enabled}
          aria-label={t('actions.toggleEnabled')}
          onChange={(event) => onToggleEnabled(job, event.currentTarget.checked)}
        />
        <div className="bf-todos__row-action-buttons">
          <Tooltip content={t('actions.edit')}>
            <IconButton
              type="button"
              size="sm"
              aria-label={t('actions.edit')}
              icon={<Icon name="edit" size="lg" />}
              onClick={() => onEdit(job)}
            />
          </Tooltip>
          <Tooltip content={t('actions.delete')}>
            <IconButton
              type="button"
              size="sm"
              tone="danger"
              aria-label={t('actions.delete')}
              icon={<Icon name="delete" size="lg" />}
              onClick={() => onDelete(job)}
            />
          </Tooltip>
        </div>
      </div>
    </div>
  );
};

export default TodoItemRow;
