import type { ComponentMeta } from "../../registry.types";

export const contextCompressionToolCardMeta = {
  category: "flow-chat",
  description: "A concrete prominent FlowChat card with a compact context-length and compression-ratio result.",
  maturity: "stable",
  name: "ContextCompressionToolCard",
  props: [
    { name: "status", type: "FlowChatToolStatus" },
    { name: "title", type: "ReactNode" },
    { name: "summary", type: "ReactNode" },
    { name: "processingText", type: "ReactNode" },
    { name: "error", type: "ReactNode" },
  ],
  states: ["default", "loading", "error"],
  tokens: [
    "color.content.secondary",
    "color.content.muted",
    "color.status.danger.content",
    "font.family.sans",
    "font.size.sm",
  ],
} as const satisfies ComponentMeta;
