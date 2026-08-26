import type { ComponentMeta } from "../../registry.types";

export const listItemMeta = {
  category: "navigation",
  description: "A compact option row with a leading icon, selected state, sibling actions, and shortcut content.",
  maturity: "stable",
  name: "ListItem",
  props: [
    { name: "children", type: "ReactNode" },
    { name: "leadingIcon", type: "ReactNode" },
    { defaultValue: "false", name: "selected", type: "boolean" },
    { name: "actions", type: "ReactNode" },
    { name: "shortcut", type: "ReactNode" },
    { defaultValue: "false", name: "disabled", type: "boolean" },
  ],
  states: ["default", "hover", "active", "disabled"],
  tokens: [
    "color.action.neutral.content",
    "color.action.neutral.contentDisabled",
    "color.action.neutral.surface",
    "color.action.neutral.surfacePressed",
    "color.focus.ring",
    "control.listItem.height",
    "control.listItem.gap",
    "control.listItem.outerGap",
    "control.listItem.padding",
    "control.listItem.iconSize",
    "control.listItem.radius",
  ],
} as const satisfies ComponentMeta;
