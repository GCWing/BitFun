import { z } from "zod"

export type ToolJsonSchema = boolean | Record<string, unknown>

/** Convert the public plugin tool argument map to the schema sent to Rust. */
export function toolParametersToJsonSchema(args: unknown): ToolJsonSchema {
  const entries = Object.entries(isRecord(args) ? args : {})
  const zodParameters = entries.every((entry) => isZodType(entry[1]))
    ? z.object(Object.fromEntries(entries) as z.ZodRawShape)
    : undefined
  if (!zodParameters) return legacyJsonSchema(entries)

  const result = normalizeZodJsonSchema(
    z.toJSONSchema(zodParameters, { io: "input", metadata: zodMetadataRegistry(zodParameters) }),
  )
  if (!isRecord(result)) throw new Error("plugin tool Zod schema produced a non-object JSON Schema")
  const { $defs, ...rest } = result
  return $defs && isRecord($defs) ? { ...rest, definitions: $defs } : rest
}

export const toolArgsToJsonSchema = toolParametersToJsonSchema

/**
 * Match OpenCode's registry boundary: Zod argument maps parse before execute,
 * while legacy JSON Schema maps remain advisory and pass through unchanged.
 */
export function validateToolArguments(argsDefinition: unknown, value: unknown) {
  const entries = Object.entries(isRecord(argsDefinition) ? argsDefinition : {})
  if (!entries.every((entry) => isZodType(entry[1]))) return value
  return z.object(Object.fromEntries(entries) as z.ZodRawShape).parse(value)
}

function isZodType(value: unknown): value is z.ZodType {
  return typeof value === "object" && value !== null && "_zod" in value
}

function isJsonSchemaDefinition(value: unknown): value is boolean | Record<string, unknown> {
  return typeof value === "boolean" || isRecord(value)
}

function legacyJsonSchema(entries: [string, unknown][]): Record<string, unknown> {
  const properties = Object.fromEntries(
    entries.filter((entry): entry is [string, boolean | Record<string, unknown>] => isJsonSchemaDefinition(entry[1])),
  )
  return {
    type: "object",
    properties,
    required: Object.keys(properties),
  }
}

function zodMetadataRegistry(schema: z.ZodType) {
  const registry = z.registry<Record<string, unknown>>()
  const seen = new WeakSet<object>()
  const collect = (value: unknown) => {
    if (typeof value !== "object" || value === null) return
    if (seen.has(value)) return
    seen.add(value)

    if (isZodType(value)) {
      const metadata = typeof value.meta === "function" ? value.meta() : undefined
      const description = typeof value.description === "string" ? value.description : undefined
      const merged = {
        ...(metadata && typeof metadata === "object" ? metadata : {}),
        ...(description ? { description } : {}),
      }
      if (Object.keys(merged).length) registry.add(value, merged)
      collect(value._zod.def)
      return
    }

    for (const item of Object.values(value)) collect(item)
  }
  collect(schema)
  return registry
}

function normalizeZodJsonSchema(value: unknown): unknown {
  if (Array.isArray(value)) return value.map((item) => normalizeZodJsonSchema(item))
  if (!isRecord(value)) return value
  return Object.fromEntries(
    Object.entries(value)
      .filter((entry) =>
        (entry[0] === "exclusiveMaximum" || entry[0] === "exclusiveMinimum") && typeof entry[1] === "boolean"
          ? false
          : true,
      )
      .map(([key, item]) => [key, normalizeZodJsonSchema(item)]),
  )
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
