import type { ComponentMeta } from "../../registry.types";

export const mobileChoiceSheetMeta = {
  category: "mobile",
  description: "A responsive bottom-sheet selector for short, mutually exclusive mobile choices.",
  maturity: "stable",
  name: "MobileChoiceSheet",
  props: [
    { name: "open", type: "boolean" },
    { name: "title", type: "ReactNode" },
    { name: "options", type: "readonly MobileChoiceSheetOption[]" },
    { name: "emptyContent", type: "ReactNode" },
    { defaultValue: "surface", name: "optionAppearance", type: "surface | plain" },
    { name: "selectedValue", type: "string" },
    { name: "cancelLabel", type: "ReactNode" },
    { name: "onSelect", type: "(value: string) => void" },
    { name: "onOpenChange", type: "(open: false, reason: MobileSheetCloseReason) => void" },
  ],
  states: ["closed", "open", "selected", "disabled", "narrow", "wide"],
  tokens: [
    "color.content.secondary",
    "space.1",
    "space.2",
  ],
} as const satisfies ComponentMeta;
