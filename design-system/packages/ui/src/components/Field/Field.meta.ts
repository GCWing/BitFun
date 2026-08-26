import type { ComponentMeta } from "../../registry.types";

export const fieldMeta = {
  category: "form",
  description: "Associates a label, optional description, and required state with one form control.",
  maturity: "stable",
  name: "Field",
  props: [
    { name: "label", type: "ReactNode" },
    { name: "description", type: "ReactNode" },
    { defaultValue: "false", name: "required", type: "boolean" },
    { defaultValue: "vertical", name: "orientation", type: "horizontal | vertical" },
    { name: "children", type: "ReactElement" },
  ],
  states: ["default"],
  tokens: [
    "color.accent.default",
    "color.content.primary",
    "color.content.muted",
    "font.family.control",
    "font.size.caption",
    "font.size.small",
    "font.weight.regular",
    "font.weight.semibold",
  ],
} as const satisfies ComponentMeta;
