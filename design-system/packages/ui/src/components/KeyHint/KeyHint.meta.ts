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
    "color.surface.subtle",
    "font.family.control",
    "font.size.caption",
    "font.weight.regular",
    "radius.sm",
  ],
} as const satisfies ComponentMeta;
