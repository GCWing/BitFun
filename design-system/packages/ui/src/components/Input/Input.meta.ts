import type { ComponentMeta } from "../../registry.types";

export const inputMeta = {
  category: "form",
  description: "A pill-shaped single-line text field with optional leading and trailing content.",
  maturity: "stable",
  name: "Input",
  props: [
    { name: "leadingIcon", type: "ReactNode" },
    { name: "trailingContent", type: "ReactNode" },
    { defaultValue: "false", name: "disabled", type: "boolean" },
  ],
  states: ["default", "hover", "focus", "disabled"],
  tokens: [
    "color.field.background",
    "color.field.backgroundHover",
    "color.field.border",
    "color.field.borderHover",
    "color.field.borderFocus",
    "color.content.primary",
    "color.content.muted",
    "color.content.disabled",
    "control.height.md",
    "control.input.iconSize",
    "control.input.gap",
    "control.input.paddingInline",
    "control.input.radius",
    "shadow.sm",
  ],
} as const satisfies ComponentMeta;
