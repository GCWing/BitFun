/**
 * Cron expression evaluation that mirrors the scheduler's backend semantics.
 *
 * The backend evaluates `CronSchedule::Cron` with the Rust `cron` crate, so the
 * calendar has to agree with it field for field or it would advertise runs that
 * never happen. The behaviors reproduced here, all of which differ from classic
 * crontab, are:
 *
 *  - Fields are `sec min hour day-of-month month day-of-week year`. A 5-field
 *    expression is prefixed with a `0` seconds field; 6 and 7 fields are used
 *    as written.
 *  - Day-of-week ordinals are 1-7 with Sunday = 1 (Quartz style), so `0` is out
 *    of range rather than meaning Sunday.
 *  - Day-of-month and day-of-week are ANDed. Classic crontab ORs them when both
 *    are restricted; the crate does not.
 *  - Without an explicit timezone the expression is evaluated in local time.
 */

/** Inclusive ordinal bounds per field, in canonical 7-field order. */
const FIELD_BOUNDS = {
  second: [0, 59],
  minute: [0, 59],
  hour: [0, 23],
  dayOfMonth: [1, 31],
  month: [1, 12],
  dayOfWeek: [1, 7],
  year: [1970, 2100],
} as const;

type FieldName = keyof typeof FIELD_BOUNDS;

const MONTH_NAMES: Record<string, number> = {
  jan: 1, january: 1,
  feb: 2, february: 2,
  mar: 3, march: 3,
  apr: 4, april: 4,
  may: 5,
  jun: 6, june: 6,
  jul: 7, july: 7,
  aug: 8, august: 8,
  sep: 9, sept: 9, september: 9,
  oct: 10, october: 10,
  nov: 11, november: 11,
  dec: 12, december: 12,
};

/** Sunday = 1, matching `DaysOfWeek::ordinal_from_name` in the `cron` crate. */
const DAY_OF_WEEK_NAMES: Record<string, number> = {
  sun: 1, sunday: 1,
  mon: 2, monday: 2,
  tue: 3, tues: 3, tuesday: 3,
  wed: 4, wednesday: 4,
  thu: 5, thurs: 5, thursday: 5,
  fri: 6, friday: 6,
  sat: 7, saturday: 7,
};

export interface ParsedCronExpression {
  seconds: number[];
  minutes: number[];
  hours: number[];
  daysOfMonth: number[];
  months: number[];
  daysOfWeek: number[];
  years: number[] | null;
}

function namesFor(field: FieldName): Record<string, number> | null {
  if (field === 'month') return MONTH_NAMES;
  if (field === 'dayOfWeek') return DAY_OF_WEEK_NAMES;
  return null;
}

function parseOrdinal(token: string, field: FieldName): number {
  const names = namesFor(field);
  const named = names?.[token.toLowerCase()];
  if (named !== undefined) return named;

  if (!/^\d+$/.test(token)) {
    throw new Error(`Invalid ${field} value '${token}'`);
  }
  return Number(token);
}

/** Expands one comma-separated field into the sorted set of ordinals it matches. */
function parseField(rawField: string, field: FieldName): number[] {
  const [min, max] = FIELD_BOUNDS[field];
  const matched = new Set<number>();

  for (const part of rawField.split(',')) {
    const token = part.trim();
    if (!token) throw new Error(`Empty ${field} field`);

    const [baseToken, stepToken, ...extra] = token.split('/');
    if (extra.length > 0) throw new Error(`Invalid ${field} step in '${token}'`);

    let step = 1;
    if (stepToken !== undefined) {
      if (!/^\d+$/.test(stepToken) || Number(stepToken) === 0) {
        throw new Error(`Invalid ${field} step in '${token}'`);
      }
      step = Number(stepToken);
    }

    let rangeStart: number;
    let rangeEnd: number;

    // `?` is accepted as a synonym for `*` in day fields, as Quartz-style
    // expressions commonly use it.
    if (baseToken === '*' || baseToken === '?') {
      rangeStart = min;
      rangeEnd = max;
    } else if (baseToken.includes('-')) {
      const [startToken, endToken, ...rest] = baseToken.split('-');
      if (rest.length > 0) throw new Error(`Invalid ${field} range in '${token}'`);
      rangeStart = parseOrdinal(startToken, field);
      rangeEnd = parseOrdinal(endToken, field);
    } else {
      rangeStart = parseOrdinal(baseToken, field);
      // A bare point with a step runs from that point to the field maximum.
      rangeEnd = stepToken !== undefined ? max : rangeStart;
    }

    if (rangeStart < min || rangeEnd > max || rangeStart > rangeEnd) {
      throw new Error(`${field} value out of range in '${token}'`);
    }

    for (let value = rangeStart; value <= rangeEnd; value += step) {
      matched.add(value);
    }
  }

  return [...matched].sort((a, b) => a - b);
}

/**
 * Parses a 5, 6, or 7 field expression. Throws on anything the backend would
 * also reject, so callers can treat a throw as "this job will not run".
 */
export function parseCronExpression(expr: string): ParsedCronExpression {
  const fields = expr.trim().split(/\s+/).filter(Boolean);

  let normalized: string[];
  if (fields.length === 5) {
    normalized = ['0', ...fields];
  } else if (fields.length === 6 || fields.length === 7) {
    normalized = fields;
  } else {
    throw new Error(
      `Cron expression '${expr}' must contain 5, 6, or 7 fields, found ${fields.length}`,
    );
  }

  return {
    seconds: parseField(normalized[0], 'second'),
    minutes: parseField(normalized[1], 'minute'),
    hours: parseField(normalized[2], 'hour'),
    daysOfMonth: parseField(normalized[3], 'dayOfMonth'),
    months: parseField(normalized[4], 'month'),
    daysOfWeek: parseField(normalized[5], 'dayOfWeek'),
    years: normalized[6] !== undefined ? parseField(normalized[6], 'year') : null,
  };
}

/** Wall-clock fields of an instant in the viewer's local zone. */
interface WallClock {
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
  second: number;
}

/**
 * Whether this expression can be expanded in the browser.
 *
 * Evaluating a named timezone needs a timezone database, and the only one a
 * browser exposes is `Intl`, which the i18n contract reserves for the shared
 * formatting APIs. Rather than approximate — and print run times that never
 * happen — schedules pinned to an explicit timezone are not expanded here; the
 * caller falls back to the next run the scheduler itself reports.
 */
export function canExpandLocally(tz: string | null | undefined): boolean {
  return !tz?.trim();
}

function toWallClock(utcMs: number): WallClock {
  const local = new Date(utcMs);
  return {
    year: local.getFullYear(),
    month: local.getMonth() + 1,
    day: local.getDate(),
    hour: local.getHours(),
    minute: local.getMinutes(),
    second: local.getSeconds(),
  };
}

/**
 * Converts local wall-clock fields back to a timestamp.
 *
 * Returns null when the wall clock does not exist — a spring-forward gap —
 * mirroring the backend's `LocalResult::None` skip.
 */
function fromWallClock(wall: WallClock): number | null {
  const local = new Date(
    wall.year, wall.month - 1, wall.day, wall.hour, wall.minute, wall.second, 0,
  );
  // Rolled-over components mean the wall clock does not exist on this date.
  if (
    local.getFullYear() !== wall.year
    || local.getMonth() !== wall.month - 1
    || local.getDate() !== wall.day
    || local.getHours() !== wall.hour
  ) {
    return null;
  }
  return local.getTime();
}

/** Day of week for a civil date, as a 1-7 ordinal with Sunday = 1. */
function dayOfWeekOrdinal(year: number, month: number, day: number): number {
  return new Date(Date.UTC(year, month - 1, day)).getUTCDay() + 1;
}

function daysInMonth(year: number, month: number): number {
  return new Date(Date.UTC(year, month, 0)).getUTCDate();
}

export interface CronOccurrenceOptions {
  /** Exclusive lower bound; only occurrences strictly after this are produced. */
  afterMs: number;
  /** Inclusive upper bound. */
  untilMs: number;
  /** Safety cap so a per-second expression cannot allocate without bound. */
  limit: number;
}

/**
 * Lists occurrences of a cron expression in `(afterMs, untilMs]`.
 *
 * Iterates candidate days rather than stepping second by second, so a month
 * long window costs at most ~31 day checks plus the matching times on each.
 *
 * Returns an empty list for a schedule pinned to an explicit timezone — see
 * `canExpandLocally`. The expression is still parsed first, so an invalid one
 * is reported as invalid rather than silently treated as un-expandable.
 */
export function cronOccurrencesInRange(
  expr: string,
  tz: string | null | undefined,
  { afterMs, untilMs, limit }: CronOccurrenceOptions,
): number[] {
  const parsed = parseCronExpression(expr);
  if (untilMs <= afterMs || limit <= 0 || !canExpandLocally(tz)) return [];

  const monthSet = new Set(parsed.months);
  const dayOfMonthSet = new Set(parsed.daysOfMonth);
  const dayOfWeekSet = new Set(parsed.daysOfWeek);
  const yearSet = parsed.years ? new Set(parsed.years) : null;

  const occurrences: number[] = [];
  const startWall = toWallClock(afterMs);
  const endWall = toWallClock(untilMs);

  let { year, month, day } = startWall;

  // Walk local civil days until past the window's last day.
  while (
    year < endWall.year
    || (year === endWall.year && month < endWall.month)
    || (year === endWall.year && month === endWall.month && day <= endWall.day)
  ) {
    const dayMatches =
      (yearSet === null || yearSet.has(year))
      && monthSet.has(month)
      && dayOfMonthSet.has(day)
      && dayOfWeekSet.has(dayOfWeekOrdinal(year, month, day));

    if (dayMatches) {
      for (const hour of parsed.hours) {
        for (const minute of parsed.minutes) {
          for (const second of parsed.seconds) {
            const atMs = fromWallClock({ year, month, day, hour, minute, second });
            if (atMs === null || atMs <= afterMs) continue;
            if (atMs > untilMs) continue;

            occurrences.push(atMs);
            if (occurrences.length >= limit) {
              return occurrences.sort((a, b) => a - b);
            }
          }
        }
      }
    }

    day += 1;
    if (day > daysInMonth(year, month)) {
      day = 1;
      month += 1;
      if (month > 12) {
        month = 1;
        year += 1;
        if (yearSet && year > Math.max(...yearSet)) break;
      }
    }
  }

  return occurrences.sort((a, b) => a - b);
}

/** First occurrence strictly after `afterMs`, or null if none within `withinMs`. */
export function nextCronOccurrence(
  expr: string,
  tz: string | null | undefined,
  afterMs: number,
  withinMs: number,
): number | null {
  const [first] = cronOccurrencesInRange(expr, tz, {
    afterMs,
    untilMs: afterMs + withinMs,
    limit: 1,
  });
  return first ?? null;
}
