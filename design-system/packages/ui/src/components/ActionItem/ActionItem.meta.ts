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
    { name: "shortcut", type: "ReactNode" },
    { defaultValue: "[]", name: "actions", type: "readonly ActionItemAction[]" },
    { defaultValue: "false", name: "disabled", type: "boolean" },
  ],
  states: ["default", "hover", "active", "focus-visible", "disabled"],
  tokens: [
    "color.action.neutral.content",
    "color.action.neutral.contentDisabled",
    "color.action.neutral.surface",
    "color.action.neutral.surfacePressed",
    "color.focus.ring",
    "control.height.sm",
    "font.family.control",
    "font.size.small",
    "font.weight.regular",
    "font.weight.semibold",
    "radius.base",
  ],
} as const satisfies ComponentMeta;
