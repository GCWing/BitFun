import type { ComponentMeta } from "../../registry.types";

export const keyHintMeta = {
  category: "feedback",
  description: "A compact keyboard shortcut label matching the Figma label component.",
  figma: {
    fileKey: "5k2waflRzrRdd8yLYEOam8",
    nodeIds: ["2:130"],
    sourceNames: ["label"],
  },
  maturity: "stable",
  name: "KeyHint",
  props: [
    { name: "children", type: "ReactNode" },
    { name: "leadingIcon", type: "ReactNode" },
  ],
  states: ["default"],
  tokens: [
    "color.action.neutral.surface",
    "color.content.muted",
    "control.keyHint.iconSize",
    "control.keyHint.gap",
    "control.keyHint.paddingBlock",
    "control.keyHint.paddingInline",
    "control.keyHint.radius",
    "font.size.micro",
  ],
} as const satisfies ComponentMeta;
