import { appendFile } from "node:fs/promises"

async function record(file, value) {
  if (!file) return
  await appendFile(file, `${value}\n`)
}

export default {
  id: "fixture.sequence-a",
  server: async (_input, options = {}) => ({
    config: async (config) => {
      config.order ??= []
      config.order.push("a")
      if (options.configFails) throw new Error("a config failed")
    },
    "chat.message": async (input, output) => {
      input.order ??= []
      output.order ??= []
      input.order.push("a")
      output.order.push("a")
      await record(options.hookMarker, "a")
      if (options.hookFails) throw new Error("a hook failed")
    },
    event: async () => {
      await record(options.eventMarker, "a")
      if (options.eventFails) throw new Error("a event failed")
    },
    dispose: async () => {
      await record(options.disposeMarker, "a")
      if (options.disposeFails) throw new Error("a dispose failed")
    },
  }),
}
