import { appendFile } from "node:fs/promises"
import { z } from "zod"

const moduleToken = crypto.randomUUID()
let entrypointRuns = 0

async function record(file, value) {
  if (!file) return
  await appendFile(file, `${value}\n`)
}

export default {
  id: "fixture.full",
  server: async (input, options = {}) => {
    entrypointRuns += 1
    const run = entrypointRuns

    input.experimental_workspace.register("fixture-remote", {
      name: "Fixture Remote",
      description: "Runtime test workspace",
      configure: async (config) => ({ ...config, name: `configured:${config.name}` }),
      create: async (config, env, from) => {
        await record(options.workspaceMarker, `create:${config.id}:${env.FIXTURE ?? "missing"}:${from?.id ?? "none"}`)
      },
      remove: async (config) => {
        await record(options.workspaceMarker, `remove:${config.id}`)
      },
      target: async (config) => ({
        type: "remote",
        url: new URL(`https://workspace.example/${config.id}?branch=${config.branch ?? "none"}`),
        headers: new Headers({ authorization: "Bearer fixture", "x-workspace": config.id }),
      }),
    })

    return {
      config: async (config) => {
        config.runtime = { moduleToken, run, directory: input.directory }
      },
      "chat.message": async (hookInput, output) => {
        hookInput.trace ??= []
        output.trace ??= []
        hookInput.trace.push("full")
        output.trace.push("full")
      },
      "tool.execute.before": async () => {
        await record(options.hookStartMarker, "started")
        if (options.hookDelayMs) await Bun.sleep(options.hookDelayMs)
      },
      dispose: async () => {
        await record(options.disposeMarker, `full:${run}`)
      },
      tool: {
        "fixture.echo": {
          description: "Exercise the tool bridge",
          args: {
            value: z.string().describe("Value to echo"),
            waitForAbort: z.boolean().optional(),
          },
          execute: async (args, context) => {
            context.metadata({ title: `metadata:${args.value}`, metadata: { phase: "before-ask" } })
            await context.ask({
              permission: "fixture.execute",
              patterns: [args.value],
              always: [],
              metadata: { value: args.value },
            })
            if (args.waitForAbort) {
              if (options.ignoreAbortMs) {
                await Bun.sleep(options.ignoreAbortMs)
              } else {
                await new Promise((resolve, reject) => {
                  if (context.abort.aborted) return reject(new Error("fixture aborted"))
                  context.abort.addEventListener("abort", () => reject(new Error("fixture aborted")), { once: true })
                })
              }
            }
            return {
              title: `echo:${args.value}`,
              output: `${args.value}:${context.directory}:${context.worktree}`,
              metadata: { sessionID: context.sessionID, callID: context.callID ?? null },
              attachments: [
                {
                  type: "file",
                  mime: "text/plain",
                  url: "data:text/plain,fixture",
                  filename: "fixture.txt",
                },
              ],
            }
          },
        },
      },
      auth: {
        provider: "fixture-auth",
        loader: async (getAuth, provider) => {
          const auth = await getAuth()
          return {
            credential: auth.key,
            providerID: provider.id,
            fetch: async (request, init) => {
              if (new URL(request).pathname === "/wait") {
                await new Promise((resolve, reject) => {
                  if (init?.signal?.aborted) return reject(init.signal.reason)
                  init?.signal?.addEventListener("abort", () => reject(init.signal.reason), { once: true })
                })
              }
              const body = init?.body ? await new Response(init.body).text() : ""
              return new Response(`${init?.method ?? "GET"}:${request.toString()}:${body}`, {
                status: 201,
                headers: { "content-type": "text/plain", "x-fixture-fetch": "yes" },
              })
            },
          }
        },
        methods: [
          {
            type: "api",
            label: "Fixture key",
            prompts: [
              {
                type: "text",
                key: "token",
                message: "Token",
                validate: (value) => (value.startsWith("ok-") ? undefined : "Token must start with ok-"),
                condition: (inputs) => inputs.enabled === "yes",
              },
            ],
            authorize: async (inputs) =>
              inputs?.token
                ? { type: "success", key: inputs.token, provider: "fixture-auth", metadata: { source: "fixture" } }
                : { type: "failed" },
          },
          {
            type: "oauth",
            label: "Fixture OAuth",
            authorize: async () => ({
              url: "https://auth.example/authorize",
              instructions: "Paste the fixture code",
              method: "code",
              callback: async (code) =>
                code === "good"
                  ? { type: "success", key: "oauth-key", provider: "fixture-auth", metadata: { code } }
                  : { type: "failed" },
            }),
          },
        ],
      },
      provider: {
        id: "fixture-provider",
        models: async (provider, context) => ({
          "fixture-model": {
            id: "fixture-model",
            providerID: provider.id,
            name: `Fixture ${context.auth?.type ?? "anonymous"}`,
          },
        }),
      },
    }
  },
}
