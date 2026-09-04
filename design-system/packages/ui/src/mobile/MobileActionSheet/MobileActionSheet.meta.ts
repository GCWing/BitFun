import type { ComponentMeta } from "../../registry.types";

export const mobileActionSheetMeta = {
  category: "mobile",
  description: "A dismissible mobile sheet for neutral and destructive action lists.",
  maturity: "stable",
  name: "MobileActionSheet",
  props: [
    { name: "open", type: "boolean" },
    { name: "title", type: "ReactNode" },
    { name: "actions", type: "readonly MobileActionSheetItem[]" },
    { name: "cancelLabel", type: "ReactNode" },
    { defaultValue: "true", name: "closeOnAction", type: "boolean" },
    { name: "onAction", type: "(id: string) => void" },
  ],
  states: ["closed", "open", "danger", "disabled", "narrow", "wide"],
  tokens: ["color.content.secondary", "space.1"],
} as const satisfies ComponentMeta;
