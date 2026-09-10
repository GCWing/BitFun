import type { ComponentMeta } from "../../registry.types";

export const rollingTextMeta = {
  category: "primitive",
  description: "A single-line vertical text replacement with explicit identity, interruption handling, accessible text, and reduced-motion support.",
  maturity: "stable",
  name: "RollingText",
  props: [
    { name: "children", type: "string | number" },
    { defaultValue: "children", name: "transitionKey", type: "string | number" },
    { defaultValue: "marquee", name: "behavior", type: "fade | marquee" },
    { name: "marqueeActive", type: "boolean" },
  ],
  states: ["default", "replacing"],
  tokens: [
    "motion.duration.contentSwap",
    "motion.easing.smooth",
    "layout.overflowText.fadeExtent",
  ],
} as const satisfies ComponentMeta;
