import type { ComponentMeta } from "../../registry.types";

export const textareaMeta = {
  category: "form",
  description: "A multiline native text field with IME safety, support text, count, and auto-resize behavior.",
  maturity: "stable",
  name: "Textarea",
  props: [
    { name: "value", type: "string" }, { name: "label", type: "string" },
    { name: "hint", type: "string" }, { name: "invalid", type: "boolean", defaultValue: "false" },
    { name: "autoResize", type: "boolean", defaultValue: "false" }, { name: "showCount", type: "boolean", defaultValue: "false" },
  ],
  states: ["default", "hover", "focus-visible", "invalid", "disabled"],
  tokens: ["color.content.primary", "color.content.muted", "color.content.disabled", "color.field.background", "color.field.border", "color.field.borderFocus", "color.focus.ring", "color.status.danger.border"],
} as const satisfies ComponentMeta;
