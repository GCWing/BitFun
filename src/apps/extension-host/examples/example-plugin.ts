import type { Plugin, PluginModule } from "@opencode-ai/plugin"
import { tool } from "@opencode-ai/plugin"
import { z } from "zod"

const ExamplePlugin: Plugin = async (input) => {
  input.experimental_workspace.register("example-local", {
    name: "Example local workspace",
    description: "Use the plugin instance directory as a local workspace",
    configure(workspace) {
      return {
        ...workspace,
        directory: workspace.directory ?? input.directory,
      }
    },
    async create() {},
    async remove() {},
    target(workspace) {
      if (!workspace.directory) throw new Error("Example workspace has no directory")
      return {
        type: "local",
        directory: workspace.directory,
      }
    },
  })

  return {
    config: async (config) => {
      config.bitfunDemo = {
        enabled: true,
        directory: input.directory,
      }
      config.agent = {
        ...(config.agent ?? {}),
        Cowork: {
          mode: "primary",
          description: "Demo agent for exercising the BitFun OpenCode plugin bridge",
          prompt: "Use the bitfun_demo_echo tool when the user asks you to echo text.",
          permission: {
            bitfun_demo_echo: "allow",
          },
        },
      }
    },
    tool: {
      bitfun_demo_echo: tool({
        description: "Echo text from the BitFun OpenCode plugin demo",
        args: {
          text: z.string().describe("Text to echo"),
        },
        async execute(args, context) {
          context.metadata({
            title: "Preparing BitFun demo echo",
            metadata: { phase: "before-ask" },
          })
          await context.ask({
            permission: "bitfun_demo_echo",
            patterns: ["demo-echo"],
            always: ["demo-echo"],
            metadata: {
              riskDescription: "Allow the demo plugin to echo text",
            },
          })
          context.metadata({
            title: "BitFun demo echo approved",
            metadata: { phase: "after-ask" },
          })
          return {
            title: "BitFun demo echo",
            output: `${args.text}:${input.directory}`,
          }
        },
      }),
      extension_host_info: tool({
        description: "Exercise the injected OpenCode SDK client and raw HTTP gateway",
        args: {},
        async execute(_args, context) {
          context.metadata({
            title: "Inspect extension host",
            metadata: { projectID: input.project.id },
          })

          await input.client.project.current()
          const response = await fetch(new URL("/project/current", input.serverUrl), {
            signal: context.abort,
          })
          await response.body?.cancel()

          return {
            title: "Extension host",
            output: JSON.stringify(
              {
                projectID: input.project.id,
                directory: input.directory,
                worktree: input.worktree,
                serverUrl: input.serverUrl.href,
                rawGatewayStatus: response.status,
              },
              null,
              2,
            ),
          }
        },
      }),
      extension_host_bun_version: tool({
        description: "Exercise the injected Bun shell after requesting permission",
        args: {},
        async execute(_args, context) {
          await context.ask({
            permission: "example_shell",
            patterns: ["bun --version"],
            always: [],
            metadata: { command: "bun --version" },
          })
          context.abort.throwIfAborted()
          const version = (await input.$`bun --version`.text()).trim()
          context.abort.throwIfAborted()
          return `Bun ${version}`
        },
      }),
    },
    async "chat.headers"(_hookInput, output) {
      output.headers["x-example-extension-host"] = "1"
    },
  }
}

export default {
  id: "example-extension-host",
  server: ExamplePlugin,
} satisfies PluginModule
