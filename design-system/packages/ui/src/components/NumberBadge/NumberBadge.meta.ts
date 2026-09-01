import type { ComponentMeta } from "../../registry.types";

export const numberBadgeMeta = {
  category: "primitive",
  description: "A compact numeric marker with a 24px slot, 20px surface and caller-owned value. Long values grow without clipping.",
  maturity: "stable",
  name: "NumberBadge",
  props: [
    { name: "value", type: "ReactNode" },
    { name: "aria-label", type: "string" },
  ],
  states: ["default"],
  tokens: [
    "space.1", "space.5", "space.6", "font.family.control", "font.size.meta",
    "font.weight.medium", "lineHeight.tight", "radius.pill",
    "color.action.neutral.content", "color.action.neutral.surface",
  ],
} as const satisfies ComponentMeta;
