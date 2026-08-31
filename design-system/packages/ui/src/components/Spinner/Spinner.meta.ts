import type { ComponentMeta } from "../../registry.types";

export const spinnerMeta = {
  category: "feedback",
  description: "A compact decorative or labeled activity indicator with canonical matrix and bar presentations.",
  maturity: "stable",
  name: "Spinner",
  props: [
    { defaultValue: "matrix", name: "variant", type: "matrix | bars" },
    { defaultValue: "md", name: "size", type: "xs | sm | md | lg" },
    { name: "aria-label", type: "string" },
  ],
  states: ["matrix", "bars", "reduced-motion"],
  tokens: [
    "color.content.secondary",
    "color.content.muted",
    "layout.spinner.matrixCellMd",
    "layout.spinner.matrixGapMd",
  ],
} as const satisfies ComponentMeta;

export const loadingStateMeta = {
  category: "feedback",
  description: "A centered loading composition that pairs Spinner with optional support copy.",
  maturity: "stable",
  name: "LoadingState",
  props: [
    { defaultValue: "md", name: "size", type: "xs | sm | md | lg" },
    { name: "children", type: "ReactNode" },
  ],
  states: ["indicator", "with-label"],
  tokens: [
    "color.content.secondary",
    "color.content.muted",
    "space.2",
    "type.support",
  ],
} as const satisfies ComponentMeta;
