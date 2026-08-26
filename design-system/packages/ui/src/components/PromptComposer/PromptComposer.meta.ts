import type { ComponentMeta } from "../../registry.types";

export const promptComposerMeta = {
  category: "form",
  description: "A multiline prompt field with start and end control areas.",
  maturity: "stable",
  name: "PromptComposer",
  props: [
    { name: "startControls", type: "ReactNode" },
    { name: "endControls", type: "ReactNode" },
    { name: "placeholder", type: "string" },
    { defaultValue: "false", name: "disabled", type: "boolean" },
  ],
  states: ["default", "focus", "disabled"],
  tokens: [
    "color.field.background",
    "color.field.border",
    "color.field.borderFocus",
    "color.content.primary",
    "color.content.muted",
    "color.content.disabled",
    "control.promptComposer.height",
    "control.promptComposer.gap",
    "control.promptComposer.padding",
    "control.promptComposer.inputPadding",
    "control.promptComposer.radius",
    "shadow.sm",
  ],
} as const satisfies ComponentMeta;
