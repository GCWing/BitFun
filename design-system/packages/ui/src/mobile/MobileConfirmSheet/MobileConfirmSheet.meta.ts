import type { ComponentMeta } from "../../registry.types";

export const mobileConfirmSheetMeta = {
  category: "mobile",
  description: "A mobile confirmation sheet with neutral or destructive confirmation and pending state.",
  maturity: "stable",
  name: "MobileConfirmSheet",
  props: [
    { name: "open", type: "boolean" },
    { name: "title", type: "ReactNode" },
    { name: "description", type: "ReactNode" },
    { name: "cancelLabel", type: "ReactNode" },
    { name: "confirmLabel", type: "ReactNode" },
    { defaultValue: "primary", name: "confirmTone", type: "primary | danger" },
    { name: "pending", type: "boolean" },
    { name: "onConfirm", type: "() => void" },
  ],
  states: ["closed", "open", "danger", "pending", "disabled", "narrow"],
  tokens: [
    "color.content.secondary",
    "color.surface.tertiary",
    "radius.pill",
    "space.2",
    "space.3",
  ],
} as const satisfies ComponentMeta;
