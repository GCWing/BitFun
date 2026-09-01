import type { ComponentMeta } from "../../registry.types";

export const emptyMeta = {
  category: "feedback",
  description: "Explains an empty result or unavailable collection with optional identity and actions.",
  maturity: "stable",
  name: "Empty",
  props: [
    { name: "title", type: "ReactNode" },
    { name: "description", type: "ReactNode" },
    { name: "icon", type: "ReactNode" },
    { name: "actions", type: "ReactNode" },
    { defaultValue: "md", name: "imageSize", type: "sm | md | lg" },
  ],
  states: ["default", "with-title", "with-actions"],
  tokens: [
    "color.content.primary",
    "color.content.secondary",
    "color.content.muted",
    "control.height.md",
    "control.height.lg",
  ],
} as const satisfies ComponentMeta;
