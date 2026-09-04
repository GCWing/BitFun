import type { ComponentMeta } from "../../registry.types";

export const dialogMeta = {
  category: "feedback",
  description: "A compound dialog with attached or opaque floating footer anatomy on the shared overlay kernel.",
  maturity: "stable",
  name: "Dialog",
  props: [
    { name: "open", type: "boolean" },
    { name: "onOpenChange", type: "(open: false, reason: DialogCloseReason) => void" },
    { defaultValue: "md", name: "size", type: "sm | md | lg | xl | 2xl" },
    { defaultValue: "true", name: "closeOnEscape", type: "boolean" },
    { defaultValue: "true", name: "closeOnPointerOutside", type: "boolean" },
  ],
  states: ["default", "open", "alert", "scrolling", "opaque-floating-footer"],
  tokens: [
    "color.overlay.scrim",
    "color.surface.raised",
    "color.border.subtle",
    "color.content.primary",
    "color.content.secondary",
    "color.content.muted",
    "overlay.dialog.surfaceRadius",
    "overlay.dialog.viewportGutter",
    "overlay.dialog.headerPaddingBlockStart",
    "overlay.dialog.headerPaddingBlockEnd",
    "overlay.dialog.footerPaddingBlockStart",
    "overlay.dialog.footerPaddingBlockEnd",
    "overlay.dialog.footerActionMinWidth",
    "shadow.overlay",
    "motion.distance.sm",
    "type.support",
    "type.heading.dialog.fontFamily",
    "type.heading.dialog.fontSize",
    "type.heading.dialog.fontWeight",
  ],
} as const satisfies ComponentMeta;

export const sheetMeta = {
  ...dialogMeta,
  description: "A compound edge-aligned sheet using the same dismissal, focus, scroll-lock, and anatomy contracts as Dialog.",
  name: "Sheet",
  props: [
    { name: "open", type: "boolean" },
    { name: "onOpenChange", type: "(open: false, reason: DialogCloseReason) => void" },
    { defaultValue: "right", name: "placement", type: "left | right | bottom" },
    { defaultValue: "md", name: "size", type: "sm | md | lg" },
  ],
  states: ["default", "open", "left", "right", "bottom"],
} as const satisfies ComponentMeta;
