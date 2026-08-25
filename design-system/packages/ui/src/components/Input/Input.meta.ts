import type { ComponentMeta } from "../../registry.types";

export const inputMeta = {
  category: "form",
  description: "A pill-shaped text field with optional leading and trailing content, including the Figma search presentation.",
  figma: {
    fileKey: "5k2waflRzrRdd8yLYEOam8",
    nodeIds: ["133:6763"],
    sourceNames: ["input"],
  },
  maturity: "stable",
  name: "Input",
  props: [
    { defaultValue: "default", name: "variant", type: "default | search" },
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
    "control.input.searchHeight",
    "control.input.iconSize",
    "control.input.gap",
    "control.input.paddingInline",
    "control.input.radius",
    "shadow.sm",
  ],
} as const satisfies ComponentMeta;
