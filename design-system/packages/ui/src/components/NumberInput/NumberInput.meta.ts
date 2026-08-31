import type { ComponentMeta } from "../../registry.types";

export const numberInputMeta = {
  category: "form",
  description: "A numeric text field with clamping, keyboard stepping, optional units, and step controls.",
  maturity: "stable",
  name: "NumberInput",
  props: [
    { name: "value", type: "number" }, { name: "onValueChange", type: "(value: number) => void" },
    { name: "min", type: "number" }, { name: "max", type: "number" }, { name: "step", type: "number", defaultValue: "1" },
    { name: "unit", type: "string" }, { name: "variant", type: "default | compact | stepper", defaultValue: "default" },
  ],
  states: ["default", "hover", "focus-visible", "disabled"],
  tokens: ["color.content.primary", "color.content.muted", "color.content.disabled", "color.field.background", "color.field.border", "color.field.borderFocus", "color.focus.ring", "control.height.sm", "control.height.md", "control.height.lg"],
} as const satisfies ComponentMeta;
