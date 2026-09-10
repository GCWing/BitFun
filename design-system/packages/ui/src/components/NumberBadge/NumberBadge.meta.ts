import type { ComponentMeta } from "../../registry.types";

export const numberBadgeMeta = {
  category: "primitive",
  description: "A compact numeric marker with a 24px filled surface and caller-owned value. Long values grow without clipping.",
  maturity: "stable",
  name: "NumberBadge",
  props: [
    { name: "value", type: "ReactNode" },
    { name: "aria-label", type: "string" },
  ],
  states: ["default"],
  tokens: [
    "space.1", "space.6", "type.meta.fontSize", "radius.pill",
    "color.action.neutral.content", "color.numberBadge.background",
  ],
} as const satisfies ComponentMeta;
