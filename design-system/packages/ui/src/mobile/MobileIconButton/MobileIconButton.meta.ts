import type { ComponentMeta } from "../../registry.types";

export const mobileIconButtonMeta = {
  category: "mobile",
  description: "A touch-sized mobile icon action with plain, surfaced, and softly elevated appearances.",
  maturity: "stable",
  name: "MobileIconButton",
  props: [
    { name: "aria-label", type: "string" },
    { name: "icon", type: "ReactNode" },
    { defaultValue: "surface", name: "appearance", type: "plain | surface | floating" },
    { defaultValue: "md", name: "size", type: "sm | md" },
    { defaultValue: "false", name: "selected", type: "boolean" },
    { defaultValue: "false", name: "loading", type: "boolean" },
  ],
  states: ["default", "hover", "active", "focus-visible", "selected", "disabled", "loading"],
  tokens: [
    "color.action.neutral.content",
    "color.action.neutral.contentDisabled",
    "color.action.neutral.surface",
    "color.action.neutral.surfaceHover",
    "color.action.neutral.surfacePressed",
    "color.border.subtle",
    "color.border.strong",
    "color.focus.ring",
    "color.surface.panel",
    "motion.duration.fast",
    "motion.duration.loop",
    "radius.pill",
    "shadow.base",
    "shadow.sm",
  ],
} as const satisfies ComponentMeta;
