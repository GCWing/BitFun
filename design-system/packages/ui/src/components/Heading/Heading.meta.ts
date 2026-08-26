import type { ComponentMeta } from "../../registry.types";

export const headingMeta = {
  category: "primitive",
  description: "A semantic title and description composition for page, section, subsection, and centered hero contexts.",
  maturity: "stable",
  name: "Heading",
  props: [
    { name: "title", type: "ReactNode" },
    { name: "description", type: "ReactNode" },
    { name: "action", type: "ReactNode" },
    { defaultValue: "section", name: "variant", type: "hero | page | section | subsection" },
    { defaultValue: "2", name: "level", type: "1 | 2 | 3 | 4 | 5 | 6" },
  ],
  states: ["default"],
  tokens: [
    "color.content.primary",
    "color.content.secondary",
    "color.content.muted",
    "font.family.control",
    "font.size.small",
    "font.size.caption",
    "font.size.lg",
    "font.size.xl",
    "font.size.pageTitle",
    "font.size.hero",
    "font.weight.regular",
    "font.weight.medium",
    "font.weight.semibold",
  ],
} as const satisfies ComponentMeta;
