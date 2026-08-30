import type { ComponentMeta } from "../../registry.types";

export const pageHeaderMeta = {
  category: "primitive",
  description: "A title composition with independent leading content, semantic level, visual size, description, action, and alignment.",
  maturity: "stable",
  name: "PageHeader",
  props: [
    { name: "title", type: "ReactNode" },
    { name: "description", type: "ReactNode" },
    { name: "leading", type: "ReactNode" },
    { name: "action", type: "ReactNode" },
    { defaultValue: "1", name: "level", type: "1 | 2 | 3 | 4 | 5 | 6" },
    { defaultValue: "md", name: "size", type: "sm | md | lg | display" },
    { defaultValue: "start", name: "align", type: "start | center" },
    { defaultValue: "false", name: "required", type: "boolean" },
  ],
  states: ["default"],
  tokens: [
    "color.accent.default",
    "color.content.primary",
    "color.content.muted",
    "font.family.sans",
    "font.size.sm",
    "font.size.base",
    "font.size.lg",
    "font.size.2xl",
    "font.size.3xl",
    "font.size.4xl",
    "font.weight.regular",
    "font.weight.semibold",
  ],
} as const satisfies ComponentMeta;
