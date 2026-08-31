/** One-based editor line interval. `end` is inclusive when present. */
export interface LineRange {
  start: number;
  end?: number;
}
