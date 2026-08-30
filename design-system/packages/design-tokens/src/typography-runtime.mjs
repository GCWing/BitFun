export const TYPOGRAPHY_BASE_MIN_PX = 12;
export const TYPOGRAPHY_BASE_MAX_PX = 20;

export const TYPOGRAPHY_SIZE_OFFSETS = Object.freeze({
  "4xs": -7,
  "3xs": -6,
  "2xs": -5,
  micro: -4,
  meta: -3,
  xs: -2,
  sm: -1,
  base: 0,
  lg: 1,
  xl: 2,
  "2xl": 4,
  "3xl": 8,
  "4xl": 12,
  "5xl": 18,
  "6xl": 26,
  "7xl": 34,
  "8xl": 42,
  "9xl": 50,
});

function normalizeBasePx(basePx) {
  if (!Number.isFinite(basePx)) {
    throw new TypeError("Typography base size must be a finite number.");
  }

  return Math.max(TYPOGRAPHY_BASE_MIN_PX, Math.min(TYPOGRAPHY_BASE_MAX_PX, basePx));
}

function formatPx(value) {
  return `${Number(value.toFixed(2))}px`;
}

/**
 * Creates the complete canonical font-size ladder for a user-selected base.
 * Runtime consumers override only the public `--bf-font-size-*` primitives;
 * semantic `--bf-type-*` roles keep following those primitives through CSS refs.
 */
export function createTypographySizeScale(basePx) {
  const normalizedBasePx = normalizeBasePx(basePx);

  return Object.freeze(Object.fromEntries(
    Object.entries(TYPOGRAPHY_SIZE_OFFSETS).map(([name, offset]) => [
      name,
      formatPx(Math.max(7, normalizedBasePx + offset)),
    ]),
  ));
}
