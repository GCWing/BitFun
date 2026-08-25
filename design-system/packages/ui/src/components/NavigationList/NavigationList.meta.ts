import type { ComponentMeta } from "../../registry.types";

export const navigationListMeta = {
  category: "navigation",
  description: "A scrollable sidebar navigation composition with optional header, grouped sections, and footer.",
  figma: {
    fileKey: "5k2waflRzrRdd8yLYEOam8",
    nodeIds: ["98:966"],
    sourceNames: ["accordion"],
  },
  maturity: "stable",
  name: "NavigationList",
  props: [
    { name: "children", type: "ReactNode" },
    { name: "header", type: "ReactNode" },
    { name: "footer", type: "ReactNode" },
  ],
  states: ["homepage", "settings"],
  tokens: [
    "color.surface.canvas",
    "color.content.primary",
    "color.content.muted",
    "color.border.subtle",
    "control.navigationList.width",
    "control.navigationList.padding",
    "control.navigationList.gap",
    "control.navigationList.sectionGap",
    "control.navigationList.titleHeight",
    "control.navigationList.titlePaddingInline",
    "control.navigationList.footerHeight",
  ],
} as const satisfies ComponentMeta;
