import { appendFile } from "node:fs/promises"

async function record(file, value) {
  if (!file) return
  await appendFile(file, `${value}\n`)
}

export default {
  id: "fixture.sequence-b",
  server: async (_input, options = {}) => ({
    config: async (config) => {
      config.order ??= []
      config.order.push("b")
    },
    "chat.message": async (input, output) => {
      input.order ??= []
      output.order ??= []
      input.order.push("b")
      output.order.push("b")
      await record(options.hookMarker, "b")
    },
    event: async () => {
      await record(options.eventMarker, "b")
    },
    dispose: async () => {
      await record(options.disposeMarker, "b")
    },
  }),
}
