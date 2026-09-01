import type { ComponentMeta } from "../../registry.types";

export const actionItemMeta = {
  category: "action",
  description: "A native button row with independent leading, shortcut, and sibling action areas.",
  maturity: "stable",
  name: "ActionItem",
  props: [
    { name: "children", type: "ReactNode" },
    { name: "leading", type: "ReactNode" },
    { defaultValue: "false", name: "reserveLeadingSpace", type: "boolean" },
    { name: "metadata", type: "ReactNode" },
    { name: "shortcut", type: "ReactNode" },
    { defaultValue: "[]", name: "actions", type: "readonly ActionItemAction[]" },
    { defaultValue: "false", name: "disabled", type: "boolean" },
    { defaultValue: "neutral", name: "tone", type: "neutral | danger" },
  ],
  states: ["default", "hover", "active", "focus-visible", "disabled", "danger"],
  tokens: [
    "color.action.neutral.content",
    "color.action.neutral.contentDisabled",
    "color.action.neutral.surface",
    "color.action.neutral.surfacePressed",
    "color.content.muted",
    "color.focus.ring",
    "color.status.danger.content",
    "color.status.danger.surface",
    "control.height.sm",
    "type.label.md.fontSize",
    "type.label.selected.fontWeight",
    "type.meta.fontSize",
    "radius.base",
  ],
} as const satisfies ComponentMeta;
