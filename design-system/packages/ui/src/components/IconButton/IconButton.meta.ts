import type { ComponentMeta } from "../../registry.types";

export const iconButtonMeta = {
  category: "action",
  description: "A compact, accessible icon-only action with semantic tones and density-aware sizes.",
  maturity: "stable",
  name: "IconButton",
  props: [
    { name: "aria-label | aria-labelledby", type: "accessible name" },
    { defaultValue: "sm", name: "size", type: "sm | md | lg" },
    { defaultValue: "neutral", name: "tone", type: "neutral | primary | danger" },
    { defaultValue: "false", name: "loading", type: "boolean" },
  ],
  states: ["default", "hover", "active", "focus-visible", "disabled", "loading"],
  tokens: [
    "color.action.neutral.content",
    "color.action.neutral.contentDisabled",
    "color.action.neutral.surface",
    "color.action.neutral.surfacePressed",
    "color.accent.default",
    "color.focus.ring",
    "color.status.danger.content",
    "color.status.danger.surface",
    "control.iconButton.size.sm",
    "control.iconButton.size.md",
    "control.iconButton.size.lg",
    "control.iconButton.iconSize.sm",
    "control.iconButton.iconSize.md",
    "control.iconButton.iconSize.lg",
    "control.iconButton.radius",
  ],
} as const satisfies ComponentMeta;
