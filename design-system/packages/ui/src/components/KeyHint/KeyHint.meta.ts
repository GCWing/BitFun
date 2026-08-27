import type { ComponentMeta } from "../../registry.types";

export const keyHintMeta = {
  category: "primitive",
  description: "A semantic keyboard hint with optional decorative icon content.",
  maturity: "stable",
  name: "KeyHint",
  props: [
    { name: "children", type: "ReactNode" },
    { name: "icon", type: "ReactNode" },
  ],
  states: ["default"],
  tokens: [
    "color.content.muted",
    "color.keyHint.background",
    "font.family.control",
    "font.size.micro",
    "font.weight.regular",
    "radius.xs",
  ],
} as const satisfies ComponentMeta;
