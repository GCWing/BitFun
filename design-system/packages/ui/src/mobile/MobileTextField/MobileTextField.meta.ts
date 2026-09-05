import type { ComponentMeta } from "../../registry.types";

export const mobileTextFieldMeta = {
  category: "mobile",
  description: "A mobile search and text-entry field with touch geometry and optional leading or trailing content.",
  maturity: "stable",
  name: "MobileTextField",
  props: [
    { defaultValue: "soft", name: "appearance", type: "soft | surface" },
    { name: "leading", type: "ReactNode" },
    { name: "trailing", type: "ReactNode" },
    { defaultValue: "false", name: "invalid", type: "boolean" },
  ],
  states: ["default", "focus-within", "invalid", "disabled"],
  tokens: [
    "color.accent.default",
    "color.content.disabled",
    "color.content.muted",
    "color.content.primary",
    "color.content.secondary",
    "color.field.background",
    "color.field.border",
    "color.field.borderFocus",
    "color.focus.ring",
    "color.status.danger.border",
    "color.surface.tertiary",
    "radius.md",
    "shadow.xs",
    "space.2",
    "space.4",
    "type.body.md.fontSize",
  ],
} as const satisfies ComponentMeta;
