import type { ComponentMeta } from "../../registry.types";

export const mediaThumbnailMeta = {
  category: "media",
  description: "A fixed-ratio media preview with stacked screenshot, contained product, and placeholder presentations.",
  figma: {
    fileKey: "5k2waflRzrRdd8yLYEOam8",
    nodeIds: ["149:12404"],
    sourceNames: ["image"],
  },
  maturity: "stable",
  name: "MediaThumbnail",
  props: [
    { name: "src", type: "string" },
    { name: "alt", type: "string" },
    { defaultValue: "contain", name: "presentation", type: "contain | placeholder | stacked" },
  ],
  states: ["stacked", "contain", "placeholder-device", "placeholder-server"],
  tokens: [
    "color.surface.panel",
    "color.status.success.surface",
    "control.mediaThumbnail.width",
    "control.mediaThumbnail.height",
    "control.mediaThumbnail.stackedWidth",
    "control.mediaThumbnail.stackedHeight",
    "control.mediaThumbnail.containSize",
    "control.mediaThumbnail.radius",
    "shadow.overlay",
  ],
} as const satisfies ComponentMeta;
