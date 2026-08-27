import type { ComponentMeta } from "../../registry.types";

export const scrollAreaMeta = {
  category: "primitive",
  description: "Provides native scrolling with consistent orientation and scrollbar visibility contracts.",
  maturity: "stable",
  name: "ScrollArea",
  props: [
    { defaultValue: "vertical", name: "orientation", type: "vertical | horizontal | both" },
    { defaultValue: "auto", name: "scrollbarVisibility", type: "auto | always | hidden" },
  ],
  states: ["auto", "always", "hidden"],
  tokens: [
    "color.scrollbar.thumb",
    "color.scrollbar.thumbHover",
    "scrollbar.width",
    "scrollbar.radius",
  ],
} as const satisfies ComponentMeta;
