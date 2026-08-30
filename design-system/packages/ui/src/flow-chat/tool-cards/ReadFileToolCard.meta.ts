import type { ComponentMeta } from "../../registry.types";

export const readFileToolCardMeta = {
  category: "flow-chat",
  description: "A concrete ambient FlowChat card for read-file progress, permission, and completion summaries.",
  maturity: "stable",
  name: "ReadFileToolCard",
  props: [
    { name: "status", type: "FlowChatToolStatus" },
    { name: "summary", type: "ReactNode" },
    { defaultValue: "false", name: "interactive", type: "boolean" },
    { name: "onOpen", type: "() => void" },
  ],
  states: ["default", "hover", "loading", "error"],
  tokens: [
    "color.content.primary",
    "color.content.secondary",
    "color.content.muted",
    "color.status.danger.content",
    "color.status.success.content",
    "font.family.control",
    "font.size.sm",
  ],
} as const satisfies ComponentMeta;
