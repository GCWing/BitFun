import type { ComponentMeta } from "../../registry.types";

export const fileOperationToolCardMeta = {
  category: "flow-chat",
  description: "A concrete FlowChat file-operation card with ambient delete and prominent write or edit presentation.",
  maturity: "stable",
  name: "FileOperationToolCard",
  props: [
    { name: "operation", type: "'write' | 'edit' | 'delete'" },
    { name: "status", type: "FlowChatToolStatus" },
    { name: "path", type: "string" },
    { name: "pathLabel", type: "ReactNode" },
    { name: "actionTestId", type: "string" },
    { name: "pathTestId", type: "string" },
    { name: "preview", type: "ReactNode" },
    { name: "isExpanded", type: "boolean" },
    { name: "changeSummary", type: "FileOperationToolCardProps['changeSummary']" },
  ],
  states: ["default", "hover", "loading", "expanded", "error", "confirmation"],
  tokens: [
    "color.action.neutral.surface",
    "color.content.primary",
    "color.content.secondary",
    "color.content.muted",
    "color.status.danger.content",
    "font.family.mono",
    "font.size.sm",
    "font.weight.semibold",
    "radius.sm",
  ],
} as const satisfies ComponentMeta;
