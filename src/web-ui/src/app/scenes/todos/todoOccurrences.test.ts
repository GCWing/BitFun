/**
 * The Todos calendar is only trustworthy if its occurrence math agrees with the
 * scheduler. These cases pin the behaviors that differ from classic crontab,
 * plus the backend cases ported from `service/cron/schedule.rs`.
 */

import { describe, expect, it } from 'vitest';
import type { CronJob, CronJobState, CronSchedule } from '@/infrastructure/api';
import { canExpandLocally, cronOccurrencesInRange, parseCronExpression } from './cronExpression';
import {
  DAY_IN_MS,
  MAX_CALENDAR_OCCURRENCES_PER_JOB_PER_DAY,
  MAX_LIST_OCCURRENCES_PER_JOB,
  buildMonthGrid,
  buildTodoBuckets,
  groupOccurrencesByDay,
  localDayKey,
  scheduleOccurrencesInRange,
} from './todoOccurrences';

const HOUR_IN_MS = 60 * 60 * 1000;
const MINUTE_IN_MS = 60 * 1000;

function makeJob(overrides: Partial<CronJob> & { schedule: CronSchedule }): CronJob {
  const state: CronJobState = {
    consecutiveFailures: 0,
    coalescedRunCount: 0,
    ...overrides.state,
  };
  return {
    id: 'cron_test',
    name: 'test job',
    payload: { text: 'hello' },
    enabled: true,
    target: {
      kind: 'workspace',
      workspace: { workspacePath: '/tmp/workspace' },
      launch: { agentType: 'agentic' },
    },
    createdAtMs: 0,
    configUpdatedAtMs: 0,
    updatedAtMs: 0,
    ...overrides,
    state,
  };
}

/** Local-time timestamp, matching how the backend evaluates a tz-less cron. */
function localMs(
  year: number, month: number, day: number,
  hour = 0, minute = 0, second = 0,
): number {
  return new Date(year, month - 1, day, hour, minute, second, 0).getTime();
}

describe('parseCronExpression', () => {
  it('prefixes a seconds field onto a 5-field expression', () => {
    const parsed = parseCronExpression('0 8 * * *');
    expect(parsed.seconds).toEqual([0]);
    expect(parsed.minutes).toEqual([0]);
    expect(parsed.hours).toEqual([8]);
  });

  it('reads a 6-field expression as starting with seconds', () => {
    const parsed = parseCronExpression('30 0 8 * * *');
    expect(parsed.seconds).toEqual([30]);
    expect(parsed.minutes).toEqual([0]);
    expect(parsed.hours).toEqual([8]);
  });

  it('maps day-of-week names onto Sunday=1 ordinals', () => {
    expect(parseCronExpression('0 8 * * sun').daysOfWeek).toEqual([1]);
    expect(parseCronExpression('0 8 * * mon').daysOfWeek).toEqual([2]);
    expect(parseCronExpression('0 8 * * sat').daysOfWeek).toEqual([7]);
  });

  it('rejects day-of-week 0, which is out of range for the backend', () => {
    expect(() => parseCronExpression('0 8 * * 0')).toThrow();
  });

  it('expands steps, ranges and lists', () => {
    expect(parseCronExpression('*/15 * * * *').minutes).toEqual([0, 15, 30, 45]);
    expect(parseCronExpression('0 9-17 * * *').hours)
      .toEqual([9, 10, 11, 12, 13, 14, 15, 16, 17]);
    expect(parseCronExpression('0 8,12,18 * * *').hours).toEqual([8, 12, 18]);
    expect(parseCronExpression('0 8 * * mon-fri').daysOfWeek).toEqual([2, 3, 4, 5, 6]);
  });

  it('rejects an expression with the wrong field count', () => {
    expect(() => parseCronExpression('8 * *')).toThrow();
  });
});

describe('cronOccurrencesInRange', () => {
  it('lists a daily run once per day', () => {
    const occurrences = cronOccurrencesInRange('0 8 * * *', null, {
      afterMs: localMs(2026, 8, 12, 9, 0),
      untilMs: localMs(2026, 8, 15, 23, 59, 59),
      limit: 50,
    });

    expect(occurrences).toEqual([
      localMs(2026, 8, 13, 8, 0),
      localMs(2026, 8, 14, 8, 0),
      localMs(2026, 8, 15, 8, 0),
    ]);
  });

  it('ANDs day-of-month and day-of-week instead of ORing them', () => {
    // 2026-08-14 is a Friday and the only 14th in range; a Monday constraint
    // must therefore exclude it. Classic crontab would include it.
    const occurrences = cronOccurrencesInRange('0 8 14 * mon', null, {
      afterMs: localMs(2026, 8, 1),
      untilMs: localMs(2026, 8, 31, 23, 59, 59),
      limit: 50,
    });

    expect(occurrences).toEqual([]);
  });

  it('does not expand a schedule pinned to an explicit timezone', () => {
    // Evaluating a named zone needs a timezone database the i18n contract keeps
    // behind the shared formatters, so these fall back to the scheduler's own
    // next run instead of being approximated here.
    expect(canExpandLocally(null)).toBe(true);
    expect(canExpandLocally('  ')).toBe(true);
    expect(canExpandLocally('Asia/Shanghai')).toBe(false);

    expect(
      cronOccurrencesInRange('0 8 * * *', 'Asia/Shanghai', {
        afterMs: localMs(2026, 8, 12),
        untilMs: localMs(2026, 8, 20),
        limit: 10,
      }),
    ).toEqual([]);
  });

  it('still reports an invalid expression when a timezone is set', () => {
    expect(() =>
      cronOccurrencesInRange('nope', 'Asia/Shanghai', {
        afterMs: localMs(2026, 8, 12),
        untilMs: localMs(2026, 8, 20),
        limit: 10,
      }),
    ).toThrow();
  });

  it('excludes the lower bound and includes the upper bound', () => {
    const at8am = localMs(2026, 8, 13, 8, 0);
    expect(
      cronOccurrencesInRange('0 8 * * *', null, {
        afterMs: at8am,
        untilMs: at8am,
        limit: 5,
      }),
    ).toEqual([]);

    expect(
      cronOccurrencesInRange('0 8 * * *', null, {
        afterMs: at8am - 1,
        untilMs: at8am,
        limit: 5,
      }),
    ).toEqual([at8am]);
  });

  it('stops at the limit for a high-frequency expression', () => {
    const occurrences = cronOccurrencesInRange('* * * * *', null, {
      afterMs: localMs(2026, 8, 12),
      untilMs: localMs(2026, 9, 12),
      limit: 10,
    });

    expect(occurrences).toHaveLength(10);
  });
});

describe('scheduleOccurrencesInRange', () => {
  it('keeps an interval series aligned to its anchor', () => {
    // Ported from `every_schedule_keeps_anchor_alignment` in schedule.rs.
    const occurrences = scheduleOccurrencesInRange(
      { kind: 'every', everyMs: 60_000, anchorMs: 1_000 },
      1_000,
      181_000,
      301_000,
    );

    expect(occurrences[0]).toBe(241_000);
  });

  it('falls back to the creation time when an interval has no anchor', () => {
    const createdAtMs = localMs(2026, 8, 12, 8, 3);
    const occurrences = scheduleOccurrencesInRange(
      { kind: 'every', everyMs: HOUR_IN_MS },
      createdAtMs,
      localMs(2026, 8, 12, 10, 0),
      localMs(2026, 8, 12, 12, 0),
    );

    expect(occurrences).toEqual([
      localMs(2026, 8, 12, 10, 3),
      localMs(2026, 8, 12, 11, 3),
    ]);
  });

  it('returns a one-shot run only when it falls inside the range', () => {
    const schedule: CronSchedule = {
      kind: 'at',
      at: new Date(localMs(2026, 8, 13, 9, 0)).toISOString(),
    };

    expect(
      scheduleOccurrencesInRange(schedule, 0, localMs(2026, 8, 12), localMs(2026, 8, 14)),
    ).toEqual([localMs(2026, 8, 13, 9, 0)]);

    expect(
      scheduleOccurrencesInRange(schedule, 0, localMs(2026, 8, 14), localMs(2026, 8, 15)),
    ).toEqual([]);
  });

  it('throws for a zero interval, as the backend rejects it', () => {
    expect(() =>
      scheduleOccurrencesInRange({ kind: 'every', everyMs: 0 }, 0, 0, DAY_IN_MS),
    ).toThrow();
  });
});

describe('buildTodoBuckets', () => {
  const nowMs = localMs(2026, 8, 12, 10, 0);
  const rangeEndMs = localMs(2026, 8, 31, 23, 59, 59);

  it('splits occurrences at the 24 hour boundary', () => {
    const job = makeJob({
      schedule: { kind: 'cron', expr: '0 9 * * *' },
      state: { consecutiveFailures: 0, coalescedRunCount: 0, nextRunAtMs: localMs(2026, 8, 13, 9, 0) },
    });

    const { upcoming, later } = buildTodoBuckets([job], { nowMs, rangeEndMs });

    // 09:00 tomorrow is 23h out and lists; every later day goes to the calendar.
    expect(upcoming.map((o) => o.atMs)).toEqual([localMs(2026, 8, 13, 9, 0)]);
    expect(later[0].atMs).toBe(localMs(2026, 8, 14, 9, 0));
    expect(later.every((o) => o.atMs > nowMs + DAY_IN_MS)).toBe(true);
  });

  it('marks the scheduler-reported run as the next run', () => {
    const nextRunAtMs = localMs(2026, 8, 12, 14, 0);
    const job = makeJob({
      schedule: { kind: 'every', everyMs: 4 * HOUR_IN_MS, anchorMs: localMs(2026, 8, 12, 2, 0) },
      state: { consecutiveFailures: 0, coalescedRunCount: 0, nextRunAtMs },
    });

    const { upcoming } = buildTodoBuckets([job], { nowMs, rangeEndMs });

    expect(upcoming[0].atMs).toBe(nextRunAtMs);
    expect(upcoming[0].isNextRun).toBe(true);
    expect(upcoming.filter((o) => o.isNextRun)).toHaveLength(1);
  });

  it('surfaces an overdue pending trigger instead of hiding it', () => {
    const pendingTriggerAtMs = nowMs - 30 * MINUTE_IN_MS;
    const job = makeJob({
      schedule: { kind: 'at', at: new Date(pendingTriggerAtMs).toISOString() },
      state: { consecutiveFailures: 0, coalescedRunCount: 0, pendingTriggerAtMs },
    });

    const { upcoming, inactive } = buildTodoBuckets([job], { nowMs, rangeEndMs });

    expect(inactive).toHaveLength(0);
    expect(upcoming).toHaveLength(1);
    expect(upcoming[0].isOverdue).toBe(true);
  });

  it('includes a retry time that sits off the nominal series', () => {
    const retryAtMs = nowMs + 5 * MINUTE_IN_MS;
    const job = makeJob({
      schedule: { kind: 'cron', expr: '0 9 * * *' },
      state: { consecutiveFailures: 1, coalescedRunCount: 0, retryAtMs },
    });

    const { upcoming } = buildTodoBuckets([job], { nowMs, rangeEndMs });

    expect(upcoming[0].atMs).toBe(retryAtMs);
    expect(upcoming[0].isNextRun).toBe(true);
  });

  it('groups a disabled job as inactive rather than dropping it', () => {
    const job = makeJob({
      enabled: false,
      schedule: { kind: 'cron', expr: '0 9 * * *' },
    });

    const { upcoming, later, inactive } = buildTodoBuckets([job], { nowMs, rangeEndMs });

    expect(upcoming).toHaveLength(0);
    expect(later).toHaveLength(0);
    expect(inactive).toEqual([{ job, reason: 'disabled' }]);
  });

  it('reports a fired one-shot as completed', () => {
    const firedAtMs = nowMs - 2 * HOUR_IN_MS;
    const job = makeJob({
      schedule: { kind: 'at', at: new Date(firedAtMs).toISOString() },
      state: {
        consecutiveFailures: 0,
        coalescedRunCount: 0,
        lastEnqueuedAtMs: firedAtMs,
        lastRunFinishedAtMs: firedAtMs + 1_000,
        lastRunStatus: 'ok',
      },
    });

    const { inactive } = buildTodoBuckets([job], { nowMs, rangeEndMs });

    expect(inactive).toEqual([{ job, reason: 'completed' }]);
  });

  it('falls back to the scheduler next run for a timezone-pinned schedule', () => {
    const nextRunAtMs = localMs(2026, 8, 20, 8, 0);
    const job = makeJob({
      schedule: { kind: 'cron', expr: '0 8 * * *', tz: 'Asia/Shanghai' },
      state: { consecutiveFailures: 0, coalescedRunCount: 0, nextRunAtMs },
    });

    const { later, inactive } = buildTodoBuckets([job], { nowMs, rangeEndMs });

    // Exactly the run the backend reports — no locally invented occurrences.
    expect(inactive).toHaveLength(0);
    expect(later.map((o) => o.atMs)).toEqual([nextRunAtMs]);
    expect(later[0].isNextRun).toBe(true);
  });

  it('puts near-term runs on the calendar as well as in the list', () => {
    const job = makeJob({
      schedule: { kind: 'cron', expr: '0 9 * * *' },
      state: { consecutiveFailures: 0, coalescedRunCount: 0, nextRunAtMs: localMs(2026, 8, 13, 9, 0) },
    });

    const { upcoming, later, calendar } = buildTodoBuckets([job], { nowMs, rangeEndMs });

    // The list keeps its 24-hour scope, but the calendar carries the whole
    // agenda so today's and tomorrow's cells are not mysteriously empty.
    expect(upcoming).toHaveLength(1);
    expect(calendar).toHaveLength(upcoming.length + later.length);
    expect(calendar.map((o) => o.atMs)).toContain(localMs(2026, 8, 13, 9, 0));
    expect(calendar).toEqual([...calendar].sort((a, b) => a.atMs - b.atMs));
  });

  it('keeps a finished job off the calendar', () => {
    const firedAtMs = nowMs - 2 * HOUR_IN_MS;
    const job = makeJob({
      schedule: { kind: 'at', at: new Date(firedAtMs).toISOString() },
      state: {
        consecutiveFailures: 0,
        coalescedRunCount: 0,
        lastEnqueuedAtMs: firedAtMs,
        lastRunStatus: 'ok',
      },
    });

    const { calendar, inactive } = buildTodoBuckets([job], { nowMs, rangeEndMs });

    expect(calendar).toHaveLength(0);
    expect(inactive[0].reason).toBe('completed');
  });

  it('marks an in-flight run as running rather than overdue', () => {
    const pendingTriggerAtMs = nowMs - 10 * MINUTE_IN_MS;
    const job = makeJob({
      schedule: { kind: 'every', everyMs: DAY_IN_MS, anchorMs: pendingTriggerAtMs },
      state: {
        consecutiveFailures: 0,
        coalescedRunCount: 0,
        pendingTriggerAtMs,
        activeTurnId: 'cronjob_test_1',
        lastRunStatus: 'running',
      },
    });

    const { upcoming } = buildTodoBuckets([job], { nowMs, rangeEndMs });
    const current = upcoming.find((o) => o.atMs === pendingTriggerAtMs);

    expect(current?.isRunning).toBe(true);
    expect(current?.isOverdue).toBe(false);
    // A job past its time but not executing is still genuinely overdue.
    expect(
      buildTodoBuckets(
        [makeJob({
          schedule: { kind: 'at', at: new Date(pendingTriggerAtMs).toISOString() },
          state: { consecutiveFailures: 0, coalescedRunCount: 0, pendingTriggerAtMs },
        })],
        { nowMs, rangeEndMs },
      ).upcoming[0].isOverdue,
    ).toBe(true);
  });

  it('reports an unparseable expression as invalid with its error', () => {
    const job = makeJob({ schedule: { kind: 'cron', expr: 'not a cron' } });

    const { inactive } = buildTodoBuckets([job], { nowMs, rangeEndMs });

    expect(inactive).toHaveLength(1);
    expect(inactive[0].reason).toBe('invalid');
    expect(inactive[0].error).toBeTruthy();
  });

  it('caps a high-frequency job in the list instead of flooding it', () => {
    const job = makeJob({
      schedule: { kind: 'every', everyMs: 5 * MINUTE_IN_MS, anchorMs: nowMs },
    });

    const { upcoming } = buildTodoBuckets([job], { nowMs, rangeEndMs });

    // 5 minutes over 24 hours is 288 runs; the row's schedule summary carries
    // the recurrence, so only the leading few are listed.
    expect(upcoming).toHaveLength(MAX_LIST_OCCURRENCES_PER_JOB);
    expect(upcoming.map((o) => o.atMs)).toEqual([
      nowMs + 5 * MINUTE_IN_MS,
      nowMs + 10 * MINUTE_IN_MS,
      nowMs + 15 * MINUTE_IN_MS,
    ]);
  });

  it('keeps a high-frequency job visible on every calendar day it runs', () => {
    const job = makeJob({
      schedule: { kind: 'every', everyMs: 5 * MINUTE_IN_MS, anchorMs: nowMs },
    });

    const { later } = buildTodoBuckets([job], { nowMs, rangeEndMs });
    const days = new Set(later.map((o) => localDayKey(o.atMs)));

    // A single windowed budget would have been spent on the first day alone.
    expect(days.size).toBeGreaterThan(10);
    for (const [, dayOccurrences] of groupOccurrencesByDay(later)) {
      expect(dayOccurrences.length).toBeLessThanOrEqual(
        MAX_CALENDAR_OCCURRENCES_PER_JOB_PER_DAY,
      );
    }
  });

  it('keeps the 24 hour list complete while the calendar shows a later month', () => {
    const job = makeJob({
      schedule: { kind: 'cron', expr: '0 9 * * *' },
    });

    // Calendar moved back to a past month: rangeEndMs is behind "now".
    const { upcoming } = buildTodoBuckets([job], {
      nowMs,
      rangeEndMs: localMs(2026, 7, 31, 23, 59, 59),
    });

    expect(upcoming.map((o) => o.atMs)).toEqual([localMs(2026, 8, 13, 9, 0)]);
  });
});

describe('calendar helpers', () => {
  it('builds a six week grid starting on the Sunday before the month', () => {
    const grid = buildMonthGrid(localMs(2026, 8, 12));

    expect(grid).toHaveLength(42);
    expect(grid[0].getDay()).toBe(0);
    expect(localDayKey(grid[0].getTime())).toBe('2026-07-26');
    expect(grid.some((day) => localDayKey(day.getTime()) === '2026-08-01')).toBe(true);
    expect(grid.some((day) => localDayKey(day.getTime()) === '2026-08-31')).toBe(true);
  });

  it('buckets occurrences by local day', () => {
    const job = makeJob({ schedule: { kind: 'cron', expr: '0 9 * * *' } });
    const byDay = groupOccurrencesByDay([
      { job, atMs: localMs(2026, 8, 13, 9, 0), isNextRun: false, isOverdue: false },
      { job, atMs: localMs(2026, 8, 13, 18, 0), isNextRun: false, isOverdue: false },
      { job, atMs: localMs(2026, 8, 14, 9, 0), isNextRun: false, isOverdue: false },
    ]);

    expect(byDay.get('2026-08-13')).toHaveLength(2);
    expect(byDay.get('2026-08-14')).toHaveLength(1);
  });
});
