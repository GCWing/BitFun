export type TypographySizeName =
  | "4xs"
  | "3xs"
  | "2xs"
  | "micro"
  | "meta"
  | "xs"
  | "sm"
  | "base"
  | "lg"
  | "xl"
  | "2xl"
  | "3xl"
  | "4xl"
  | "5xl"
  | "6xl"
  | "7xl"
  | "8xl"
  | "9xl";

export type TypographySizeScale = Readonly<Record<TypographySizeName, `${number}px`>>;

export declare const TYPOGRAPHY_BASE_MIN_PX = 12;
export declare const TYPOGRAPHY_BASE_MAX_PX = 20;
export declare const TYPOGRAPHY_SIZE_OFFSETS: Readonly<Record<TypographySizeName, number>>;
export declare function createTypographySizeScale(basePx: number): TypographySizeScale;
