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
    "color.content.requiredIndicator",
    "color.content.primary",
    "color.content.muted",
    "type.heading.page.fontSize",
    "type.heading.section.fontSize",
    "type.heading.display.fontSize",
    "type.display.sm.fontSize",
    "type.body.lg.fontSize",
    "type.body.sm.fontSize",
  ],
} as const satisfies ComponentMeta;
