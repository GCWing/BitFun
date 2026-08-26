import { writeFileSync } from "node:fs"

export default {
  id: "fixture.opening",
  server: async (_input, options = {}) => {
    writeFileSync(options.started, "started")
    await new Promise((resolve) => setTimeout(resolve, 30))
    return {
      dispose() {
        writeFileSync(options.disposed, "disposed")
      },
    }
  },
}
