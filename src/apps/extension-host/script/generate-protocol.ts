import { z } from "zod"
import {
  BackendMethodSchemas,
  HostMethodSchemas,
  PROTOCOL_VERSION,
  RpcErrorResponseSchema,
  RpcNotificationSchema,
  RpcRequestSchema,
  RpcSuccessResponseSchema,
} from "../src/protocol"

const definitions: Record<string, unknown> = {}
const methods: Record<string, { direction: "rust-to-host" | "host-to-rust"; params: string; result: string }> = {}

addDefinition("RpcRequest", RpcRequestSchema)
addDefinition("RpcNotification", RpcNotificationSchema)
addDefinition("RpcSuccessResponse", RpcSuccessResponseSchema)
addDefinition("RpcErrorResponse", RpcErrorResponseSchema)

for (const [method, definition] of Object.entries(HostMethodSchemas)) addMethod("rust-to-host", method, definition)
for (const [method, definition] of Object.entries(BackendMethodSchemas)) addMethod("host-to-rust", method, definition)

const schema = {
  $schema: "https://json-schema.org/draft/2020-12/schema",
  $id: "https://opencode.ai/schemas/extension-host/protocol-v1.json",
  title: "OpenCode extension host protocol",
  description: "JSON-RPC 2.0 envelopes and method schemas for the standalone OpenCode 1.17.18 Bun extension host.",
  oneOf: [
    { $ref: "#/$defs/RpcRequest" },
    { $ref: "#/$defs/RpcNotification" },
    { $ref: "#/$defs/RpcSuccessResponse" },
    { $ref: "#/$defs/RpcErrorResponse" },
  ],
  $defs: definitions,
  "x-protocol-version": PROTOCOL_VERSION,
  "x-methods": methods,
}

const output = `${JSON.stringify(schema, null, 2)}\n`
const path = new URL("../protocol.schema.json", import.meta.url)
if (process.argv.includes("--check")) {
  const current = await Bun.file(path)
    .text()
    .catch(() => "")
  if (current !== output) {
    console.error("protocol.schema.json is out of date; run bun run generate")
    process.exit(1)
  }
  process.exit(0)
}
await Bun.write(path, output)

function addMethod(
  direction: "rust-to-host" | "host-to-rust",
  method: string,
  definition: { params: z.ZodType; result: z.ZodType },
) {
  const name = method
    .split(".")
    .map((part) => `${part[0]!.toUpperCase()}${part.slice(1)}`)
    .join("")
  const params = `${name}Params`
  const result = `${name}Result`
  addDefinition(params, definition.params)
  addDefinition(result, definition.result)
  methods[method] = {
    direction,
    params: `#/$defs/${params}`,
    result: `#/$defs/${result}`,
  }
}

function addDefinition(name: string, value: z.ZodType) {
  definitions[name] = scopeReferences(z.toJSONSchema(value, { target: "draft-2020-12" }), `#/$defs/${name}`)
}

function scopeReferences(value: unknown, scope: string): unknown {
  if (Array.isArray(value)) return value.map((item) => scopeReferences(item, scope))
  if (typeof value !== "object" || value === null) return value
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => key !== "$schema")
      .map(([key, item]) => {
        if (key !== "$ref" || typeof item !== "string" || !item.startsWith("#")) {
          return [key, scopeReferences(item, scope)]
        }
        return [key, `${scope}${item.slice(1)}`]
      }),
  )
}
