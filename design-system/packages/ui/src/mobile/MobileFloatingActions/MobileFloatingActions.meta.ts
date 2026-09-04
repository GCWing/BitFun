import type { ComponentMeta } from "../../registry.types";

export const mobileFloatingActionsMeta = {
  category: "mobile",
  description: "A transparent floating action row that keeps controls interactive without obscuring scroll content.",
  maturity: "stable",
  name: "MobileFloatingActions",
  props: [
    { name: "leading", type: "ReactNode" },
    { name: "trailing", type: "ReactNode" },
  ],
  states: ["default", "narrow", "safe-area"],
  tokens: ["space.2", "space.3"],
} as const satisfies ComponentMeta;
