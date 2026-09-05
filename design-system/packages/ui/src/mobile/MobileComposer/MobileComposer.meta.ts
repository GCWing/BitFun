import type { ComponentMeta } from "../../registry.types";

export const mobileComposerMeta = {
  category: "mobile",
  description: "A compact-to-expanded mobile composer surface with semantic editor and action slots.",
  maturity: "stable",
  name: "MobileComposer",
  props: [
    { defaultValue: "false", name: "expanded", type: "boolean" },
    { name: "leading", type: "ReactNode" },
    { name: "startActions", type: "ReactNode" },
    { name: "endActions", type: "ReactNode" },
    { name: "onActivate", type: "() => void" },
  ],
  states: ["collapsed", "expanded", "active", "focus-visible"],
  tokens: [
    "color.border.subtle",
    "color.content.primary",
    "color.focus.ring",
    "color.surface.panel",
    "radius.pill",
    "radius.xl",
    "shadow.base",
    "shadow.lg",
    "space.1",
    "space.2",
  ],
} as const satisfies ComponentMeta;
