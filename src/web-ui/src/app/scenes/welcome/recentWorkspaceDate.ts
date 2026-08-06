const MILLISECONDS_PER_DAY = 24 * 60 * 60 * 1000;

type Translate = (key: string, options?: Record<string, unknown>) => string;
type FormatLocaleDate = (date: Date) => string;

function localCalendarDayNumber(date: Date): number {
  return Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / MILLISECONDS_PER_DAY;
}

export function differenceInLocalCalendarDays(now: Date, date: Date): number {
  return localCalendarDayNumber(now) - localCalendarDayNumber(date);
}

export function formatRecentWorkspaceDate(
  dateString: string,
  now: Date,
  t: Translate,
  formatLocaleDate: FormatLocaleDate,
): string {
  const date = new Date(dateString);
  const diffDays = differenceInLocalCalendarDays(now, date);

  if (diffDays === 0) return t('time.today');
  if (diffDays === 1) return t('time.yesterday');
  if (diffDays > 1 && diffDays < 7) return t('startup.daysAgo', { count: diffDays });
  if (diffDays >= 7 && diffDays < 30) {
    return t('startup.weeksAgo', { count: Math.ceil(diffDays / 7) });
  }
  return formatLocaleDate(date);
}
