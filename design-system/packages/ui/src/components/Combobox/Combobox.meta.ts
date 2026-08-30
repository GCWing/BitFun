import type { ComponentMeta } from "../../registry.types";
export const comboboxMeta = {
  name: "Combobox", category: "form", maturity: "stable",
  description: "A searchable single or multiple picker with grouped rich options, async loading and custom values.",
  props: [
    { name: "options", type: "readonly ComboboxOption[]" },
    { name: "value", type: "string | number | (string | number)[]" },
    { name: "onChange", type: "(value: ComboboxValue) => void" },
    { name: "multiple", type: "boolean", defaultValue: "false" },
    { name: "searchable", type: "boolean", defaultValue: "true" },
    { name: "allowCustomValue", type: "boolean", defaultValue: "false" },
    { name: "loading", type: "boolean" },
    { name: "renderOption / renderValue", type: "render function" },
    { name: "portalContainer", type: "Element | DocumentFragment | (() => Element | DocumentFragment | null)" },
  ],
  states: ["default", "open", "searching", "multiple", "custom", "loading", "empty", "disabled", "invalid"],
  tokens: ["color.field.background", "color.field.border", "color.focus.ring", "color.content.primary", "color.surface.panel", "overlay.menu.itemHeight", "shadow.menu"],
} as const satisfies ComponentMeta;
