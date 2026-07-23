import { type Plugin, tool } from "@opencode-ai/plugin"

export const WorkspaceToolsPlugin: Plugin = async () => {
  return {
    "tool.execute.before": async (input, output) => {
      if (input.tool === "bash") {
        output.args.command = output.args.command.trim()
      }
    },
    tool: {
      workspaceSummary: tool({
        description: "Summarize the active workspace for BitFun",
        args: {
          topic: tool.schema.string(),
        },
        async execute(args, context) {
          return `Workspace ${context.directory}: ${args.topic}`
        },
      }),
    },
  }
}
