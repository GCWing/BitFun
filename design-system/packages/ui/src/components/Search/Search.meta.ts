import type { ComponentMeta } from "../../registry.types";

export const searchMeta = {
  category: "form",
  description: "A compact search field matching the Figma search scene, with an intrinsic search icon and optional trailing content.",
  figma: {
    fileKey: "5k2waflRzrRdd8yLYEOam8",
    nodeIds: ["2:66"],
    sourceNames: ["input/scene3 (search)"],
  },
  maturity: "stable",
  name: "Search",
  props: [
    { name: "leadingIcon", type: "ReactNode" },
    { name: "trailingContent", type: "ReactNode" },
    { defaultValue: "false", name: "disabled", type: "boolean" },
  ],
  states: ["default", "hover", "focus", "disabled"],
  tokens: [
    "color.field.background",
    "color.field.backgroundHover",
    "color.field.border",
    "color.field.borderHover",
    "color.field.borderFocus",
    "color.content.primary",
    "color.content.muted",
    "color.content.disabled",
    "control.search.height",
    "control.search.iconSize",
    "control.search.contentGap",
    "control.search.trailingGap",
    "control.search.paddingBlock",
    "control.search.paddingInline",
    "control.search.radius",
  ],
} as const satisfies ComponentMeta;
