import { RemoteI18n } from '../i18n/RemoteI18n';

export class TimeFormat {
  static relative(value: string, nowMs: number = Date.now()): string {
    const timestamp = TimeFormat.timestampMs(value);
    if (Number.isNaN(timestamp)) {
      return RemoteI18n.t('time.unknown');
    }
    const minute = 60 * 1000;
    const hour = 60 * minute;
    const day = 24 * hour;
    const rawDiffMs = nowMs - timestamp;
    if (rawDiffMs < -minute) {
      return TimeFormat.absoluteDate(timestamp);
    }
    const diffMs = Math.max(0, rawDiffMs);
    if (diffMs < minute) {
      return RemoteI18n.t('time.justNow');
    }
    if (diffMs < hour) {
      return RemoteI18n.f('time.minutesAgo', `${Math.max(1, Math.floor(diffMs / minute))}`);
    }
    if (diffMs < day) {
      return RemoteI18n.f('time.hoursAgo', `${Math.floor(diffMs / hour)}`);
    }
    if (diffMs < 7 * day) {
      return RemoteI18n.f('time.daysAgo', `${Math.floor(diffMs / day)}`);
    }
    return TimeFormat.absoluteDate(timestamp);
  }

  private static timestampMs(value: string): number {
    const text = (value || '').trim();
    if (text.length === 0) {
      return Number.NaN;
    }

    if (/^\d+(\.\d+)?$/.test(text)) {
      const numeric = Number(text);
      if (!Number.isNaN(numeric)) {
        return numeric < 100000000000 ? numeric * 1000 : numeric;
      }
    }

    const direct = Date.parse(text);
    if (!Number.isNaN(direct)) {
      return direct;
    }

    const normalized = text.replace(' ', 'T');
    const parsed = Date.parse(normalized);
    return Number.isNaN(parsed) ? Number.NaN : parsed;
  }

  private static absoluteDate(timestamp: number): string {
    const date = new Date(timestamp);
    const year = date.getFullYear();
    const month = `${date.getMonth() + 1}`.padStart(2, '0');
    const day = `${date.getDate()}`.padStart(2, '0');
    return `${year}-${month}-${day}`;
  }
}
