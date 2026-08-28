import type { PluginModule } from "@opencode-ai/plugin"

const server: PluginModule["server"] = async (input) => {
  const project = await input.client.project.current()
  const raw = await fetch(new URL("/raw?fixture=1", input.serverUrl)).then((response) => response.text())
  // `echo` is provided by Bun Shell on every supported platform; `printf`
  // is not available in the Windows implementation.
  const shell = (await input.$`echo injected-shell`.text()).trim()

  input.experimental_workspace.register("fixture-remote", {
    name: "Fixture remote",
    description: "Workspace registered by the injected API fixture",
    configure(config) {
      return {
        ...config,
        name: `${config.name}-configured`,
      }
    },
    async create() {},
    async remove() {},
    target() {
      return {
        type: "remote",
        url: new URL("https://workspace.example.test/root"),
        headers: new Headers([
          ["x-fixture", "yes"],
          ["x-second", "two"],
        ]),
      }
    },
  })

  return {
    async config(config) {
      Object.assign(config, {
        injectedFixture: {
          projectID: project.data?.id,
          raw,
          shell,
          serverURL: input.serverUrl.href,
          directory: input.directory,
          worktree: input.worktree,
        },
      })
    },
  }
}

export default {
  id: "gateway-injected-fixture",
  server,
} satisfies PluginModule
