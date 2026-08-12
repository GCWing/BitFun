/**
 * Pure date-time segment helpers for `LocalizedDateTimeField`.
 *
 * Kept out of the component file so the ordering and value rules can be tested
 * directly, and so the component module only exports a component.
 */

/** Locales that read year-first. Everything else falls back to month-first. */
const YEAR_FIRST_LOCALE_PREFIXES = ['zh', 'ja', 'ko'];

export type SegmentId = 'year' | 'month' | 'day' | 'hour' | 'minute';

export interface SegmentSpec {
  id: SegmentId;
  length: number;
  min: number;
  max: number;
}

export const SEGMENTS: Record<SegmentId, SegmentSpec> = {
  year: { id: 'year', length: 4, min: 1970, max: 9999 },
  month: { id: 'month', length: 2, min: 1, max: 12 },
  day: { id: 'day', length: 2, min: 1, max: 31 },
  hour: { id: 'hour', length: 2, min: 0, max: 23 },
  minute: { id: 'minute', length: 2, min: 0, max: 59 },
};

export interface ParsedValue {
  year: string;
  month: string;
  day: string;
  hour: string;
  minute: string;
}

const EMPTY_PARSED: ParsedValue = { year: '', month: '', day: '', hour: '', minute: '' };

export function parseDateTimeValue(value: string): ParsedValue {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(value.trim());
  if (!match) return EMPTY_PARSED;
  return { year: match[1], month: match[2], day: match[3], hour: match[4], minute: match[5] };
}

export function clampSegmentToRange(raw: string, spec: SegmentSpec): string {
  const digits = raw.replace(/\D/g, '').slice(0, spec.length);
  if (!digits) return '';
  const numeric = Math.min(Math.max(Number(digits), spec.min), spec.max);
  return String(numeric).padStart(spec.length, '0');
}

/** Days in the given month, so 31 February normalizes instead of rolling over. */
function daysInMonth(year: number, month: number): number {
  return new Date(Date.UTC(year, month, 0)).getUTCDate();
}

export function composeDateTimeValue(parsed: ParsedValue): string | null {
  const { year, month, day, hour, minute } = parsed;
  if (!year || !month || !day || !hour || !minute) return null;

  const yearNumber = Number(year);
  const monthNumber = Math.min(Math.max(Number(month), 1), 12);
  const dayNumber = Math.min(
    Math.max(Number(day), 1),
    daysInMonth(yearNumber, monthNumber),
  );

  return [
    String(yearNumber).padStart(4, '0'),
    String(monthNumber).padStart(2, '0'),
    String(dayNumber).padStart(2, '0'),
  ].join('-')
    + 'T'
    + [hour, minute].join(':');
}

/**
 * Date segment order for a locale: year-first for CJK, month-first otherwise.
 *
 * Driven by the app's own locale rather than the browser's, which is the whole
 * point of this control.
 */
export function resolveDateSegmentOrder(locale: string | null | undefined): SegmentId[] {
  const language = String(locale ?? '').toLowerCase();
  const yearFirst = YEAR_FIRST_LOCALE_PREFIXES.some(prefix => language.startsWith(prefix));
  return yearFirst ? ['year', 'month', 'day'] : ['month', 'day', 'year'];
}
