export type LocalizedNumberFormatter = (
  value: number,
  options?: Intl.NumberFormatOptions,
) => string;

const COMPACT_TOKEN_OPTIONS: Intl.NumberFormatOptions = {
  maximumFractionDigits: 2,
};

const TOKEN_UNITS = [
  { divisor: 1_000, suffix: 'K' },
  { divisor: 1_000_000, suffix: 'M' },
  { divisor: 1_000_000_000, suffix: 'B' },
] as const;

const CACHE_HIT_RATE_OPTIONS: Intl.NumberFormatOptions = {
  style: 'percent',
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
};

/** Format a token count with fixed K/M/B units in every locale. */
export function formatTokenCount(
  value: number,
  formatNumber: LocalizedNumberFormatter,
): string {
  const safeValue = Math.max(0, Math.round(value));
  if (safeValue < 1_000) {
    return formatNumber(safeValue);
  }

  let unitIndex = safeValue >= 1_000_000_000
    ? 2
    : safeValue >= 1_000_000
      ? 1
      : 0;
  let scaledValue = roundToTwoDecimals(safeValue / TOKEN_UNITS[unitIndex].divisor);
  if (scaledValue >= 1_000 && unitIndex < TOKEN_UNITS.length - 1) {
    unitIndex += 1;
    scaledValue = roundToTwoDecimals(safeValue / TOKEN_UNITS[unitIndex].divisor);
  }

  return `${formatNumber(scaledValue, COMPACT_TOKEN_OPTIONS)}${TOKEN_UNITS[unitIndex].suffix}`;
}

function roundToTwoDecimals(value: number): number {
  return Math.round((value + Number.EPSILON) * 100) / 100;
}

/** Format a 0..1 cache-hit ratio with two decimal places, rounded down. */
export function formatCacheHitRate(
  value: number,
  formatNumber: LocalizedNumberFormatter,
): string {
  const boundedValue = Math.min(1, Math.max(0, value));
  const roundedDownValue = Math.floor(boundedValue * 10_000) / 10_000;
  return formatNumber(roundedDownValue, CACHE_HIT_RATE_OPTIONS);
}
