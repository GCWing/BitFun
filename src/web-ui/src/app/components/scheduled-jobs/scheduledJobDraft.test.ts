/**
 * Interval units are entered in one scale and stored in another, so the
 * round trip is the part worth pinning: what the editor shows must rebuild the
 * exact millisecond value the scheduler already had.
 */

import { describe, expect, it } from 'vitest';
import type { CronJob } from '@/infrastructure/api';
import {
  DAY_IN_MS,
  HOUR_IN_MS,
  MINUTE_IN_MS,
  buildScheduleFromDraft,
  createEmptyDraft,
  formatIntervalValue,
  jobToDraft,
  splitInterval,
} from './scheduledJobDraft';

function jobWithInterval(everyMs: number): CronJob {
  return {
    id: 'cron_test',
    name: 'test',
    schedule: { kind: 'every', everyMs },
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
    state: { consecutiveFailures: 0, coalescedRunCount: 0 },
  };
}

describe('splitInterval', () => {
  it('reads a daily interval as days rather than 1440 minutes', () => {
    expect(splitInterval(DAY_IN_MS)).toEqual({ value: 1, unit: 'day' });
    expect(splitInterval(3 * DAY_IN_MS)).toEqual({ value: 3, unit: 'day' });
  });

  it('prefers hours when days do not divide evenly', () => {
    expect(splitInterval(6 * HOUR_IN_MS)).toEqual({ value: 6, unit: 'hour' });
    expect(splitInterval(36 * HOUR_IN_MS)).toEqual({ value: 36, unit: 'hour' });
  });

  it('falls back to minutes for anything finer', () => {
    expect(splitInterval(30 * MINUTE_IN_MS)).toEqual({ value: 30, unit: 'minute' });
    expect(splitInterval(90 * MINUTE_IN_MS)).toEqual({ value: 90, unit: 'minute' });
  });

  it('does not claim a unit for a non-positive interval', () => {
    expect(splitInterval(0).unit).toBe('minute');
  });
});

describe('interval round trip', () => {
  it.each([
    ['daily', DAY_IN_MS],
    ['six hourly', 6 * HOUR_IN_MS],
    ['half hourly', 30 * MINUTE_IN_MS],
    ['awkward', 90 * MINUTE_IN_MS],
  ])('rebuilds the exact interval for a %s job', (_label, everyMs) => {
    const draft = jobToDraft(jobWithInterval(everyMs), 'agentic');
    const schedule = buildScheduleFromDraft(draft);

    expect(schedule).toMatchObject({ kind: 'every', everyMs });
  });
});

describe('formatIntervalValue', () => {
  it('keeps whole numbers whole and trims trailing zeros', () => {
    expect(formatIntervalValue(3)).toBe('3');
    expect(formatIntervalValue(1.5)).toBe('1.5');
    expect(formatIntervalValue(2.0)).toBe('2');
  });
});

describe('createEmptyDraft', () => {
  it('defaults to a readable hourly interval', () => {
    const draft = createEmptyDraft();
    expect(draft.everyValue).toBe('1');
    expect(draft.everyUnit).toBe('hour');
    expect(buildScheduleFromDraft({ ...draft, scheduleKind: 'every' }))
      .toMatchObject({ kind: 'every', everyMs: HOUR_IN_MS });
  });
});
