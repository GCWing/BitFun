import type { ComponentMeta } from "../../registry.types";

export const ambientToolCardMeta = {
  category: "flow-chat",
  description: "A low-attention tool trace that stays glanceable in the conversation and can reveal supporting detail.",
  maturity: "stable",
  name: "AmbientToolCard",
  props: [
    { name: "status", type: "FlowChatToolStatus" },
    { name: "header", type: "ReactNode" },
    { defaultValue: "false", name: "isExpanded", type: "boolean" },
    { name: "expandedContent", type: "ReactNode" },
    { name: "onClick", type: "() => void" },
  ],
  states: ["default", "hover", "loading", "expanded", "error"],
  tokens: [
    "color.content.primary",
    "color.content.secondary",
    "color.content.muted",
    "color.surface.panel",
    "color.border.default",
    "color.status.danger.content",
    "control.height.md",
    "control.toolCard.ambientRowMinBlockSize",
    "font.family.sans",
    "font.size.sm",
    "font.size.xl",
    "font.weight.medium",
    "font.weight.regular",
    "radius.md",
    "radius.sm",
    "shadow.xs",
  ],
} as const satisfies ComponentMeta;
