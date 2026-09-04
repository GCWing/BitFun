import type { ComponentMeta } from "../../registry.types";

export const mobileLinkMeta = {
  category: "mobile",
  description: "A touch-aware mobile link with consistent inline and surfaced focus treatment.",
  maturity: "stable",
  name: "MobileLink",
  props: [
    { defaultValue: "inline", name: "appearance", type: '"inline" | "surface"' },
    { name: "href", type: "string" },
  ],
  states: ["default", "hover", "active", "focus-visible"],
  tokens: [
    "color.accent.default",
    "color.accent.hover",
    "color.focus.ring",
    "color.surface.panel",
    "radius.sm",
    "radius.pill",
    "shadow.xs",
  ],
} as const satisfies ComponentMeta;
