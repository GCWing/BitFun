/**
 * Month calendar covering the whole remaining agenda.
 *
 * Near-term runs appear here as well as in the 24-hour list: the list answers
 * "what is about to happen", the calendar answers "when is everything
 * happening", and hiding today's runs from it only made day cells look wrong.
 * Jobs with nothing left to run are the only ones left out.
 */

import { ScrollArea } from '@bitfun/ui';
import React, { useMemo } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import {
  buildMonthGrid,
  groupOccurrencesByDay,
  localDayKey,
  type TodoOccurrence,
} from '../todoOccurrences';
import { formatTimeOfDay } from '../todoPresentation';

/** Chips rendered inside a day cell before collapsing into a "+N" counter. */
const MAX_CHIPS_PER_DAY = 3;

export interface TodoCalendarProps {
  occurrences: TodoOccurrence[];
  monthAnchorMs: number;
  selectedDayKey: string | null;
  nowMs: number;
  onSelectDay: (dayKey: string | null) => void;
}

const TodoCalendar: React.FC<TodoCalendarProps> = ({
  occurrences,
  monthAnchorMs,
  selectedDayKey,
  nowMs,
  onSelectDay,
}) => {
  const { t, formatDate } = useI18n('scenes/todos');

  const grid = useMemo(() => buildMonthGrid(monthAnchorMs), [monthAnchorMs]);
  const byDay = useMemo(() => groupOccurrencesByDay(occurrences), [occurrences]);

  const anchorDate = useMemo(() => new Date(monthAnchorMs), [monthAnchorMs]);
  const anchorMonth = anchorDate.getMonth();
  const todayKey = localDayKey(nowMs);

  // Weekday headers are derived from the grid so they follow the active locale
  // instead of a hard-coded English list.
  const weekdayLabels = useMemo(
    () => grid.slice(0, 7).map((day) => formatDate(day, { weekday: 'short' })),
    [formatDate, grid],
  );

  return (
    <ScrollArea
      className="bf-todos__calendar"
      aria-label={t('calendar.title')}
      data-bf-scene="todos"
      data-bf-part="calendar"
      data-testid="todos-calendar"
    >
      <header className="bf-todos__calendar-head" data-bf-scene="todos" data-bf-part="calendarHead">
        <div className="bf-todos__calendar-heading">
          <h3 className="bf-todos__pane-title">{t('calendar.title')}</h3>
          <p className="bf-todos__pane-hint">{t('calendar.hint')}</p>
        </div>
      </header>

      <div className="bf-todos__calendar-weekdays" aria-hidden="true">
        {weekdayLabels.map((label, index) => (
          <span key={index} className="bf-todos__calendar-weekday">{label}</span>
        ))}
      </div>

      <div className="bf-todos__calendar-grid" role="grid" data-bf-scene="todos" data-bf-part="calendarGrid">
        {grid.map((day) => {
          const dayKey = localDayKey(day.getTime());
          const dayOccurrences = byDay.get(dayKey) ?? [];
          const isCurrentMonth = day.getMonth() === anchorMonth;
          const isToday = dayKey === todayKey;
          const isSelected = dayKey === selectedDayKey;
          const cellState = [
            isSelected ? 'selected' : null,
            isToday ? 'today' : null,
            isCurrentMonth ? null : 'outside',
          ].filter(Boolean).join(' ');

          return (
            <button
              key={dayKey}
              type="button"
              role="gridcell"
              className={[
                'bf-todos__calendar-cell',
                isCurrentMonth ? '' : 'bf-todos__calendar-cell--outside',
                isToday ? 'bf-todos__calendar-cell--today' : '',
                isSelected ? 'bf-todos__calendar-cell--selected' : '',
                dayOccurrences.length > 0 ? 'bf-todos__calendar-cell--has-items' : '',
              ].filter(Boolean).join(' ')}
              data-bf-scene="todos"
              data-bf-part="calendarCell"
              data-bf-state={cellState || undefined}
              data-testid="todos-calendar-cell"
              data-day-key={dayKey}
              aria-pressed={isSelected}
              aria-label={`${formatDate(day, { year: 'numeric', month: 'long', day: 'numeric' })}, ${
                t('calendar.dayCount', { total: dayOccurrences.length })
              }`}
              onClick={() => onSelectDay(isSelected ? null : dayKey)}
            >
              <span className="bf-todos__calendar-daynum">{day.getDate()}</span>
              <span className="bf-todos__calendar-chips">
                {dayOccurrences.slice(0, MAX_CHIPS_PER_DAY).map((occurrence, index) => (
                  <span
                    key={`${occurrence.job.id}-${occurrence.atMs}-${index}`}
                    className="bf-todos__calendar-chip"
                    title={`${formatTimeOfDay(occurrence.atMs, formatDate)} ${occurrence.job.name}`}
                  >
                    <span className="bf-todos__calendar-chip-time">
                      {formatTimeOfDay(occurrence.atMs, formatDate)}
                    </span>
                    <span className="bf-todos__calendar-chip-name">{occurrence.job.name}</span>
                  </span>
                ))}
                {dayOccurrences.length > MAX_CHIPS_PER_DAY ? (
                  <span className="bf-todos__calendar-more">
                    {t('calendar.moreCount', { total: dayOccurrences.length - MAX_CHIPS_PER_DAY })}
                  </span>
                ) : null}
              </span>
            </button>
          );
        })}
      </div>
    </ScrollArea>
  );
};

export default TodoCalendar;
