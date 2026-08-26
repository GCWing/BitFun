import { z } from "zod"

export const PROTOCOL_VERSION = 1
export const OPENCODE_VERSION = "1.17.18"
export const HOST_CAPABILITIES = [
  "config-contributors-v1",
  "config-contributions-v2",
  "generation-fencing-v1",
] as const
export const DEFAULT_MAX_FRAME_BYTES = 16 * 1024 * 1024
export const MAX_MAX_FRAME_BYTES = 64 * 1024 * 1024
export const MAX_STREAM_CHUNK_BYTES = 64 * 1024

export const JsonValueSchema = z.json()
export const JsonObjectSchema = z.record(z.string(), JsonValueSchema)
export const EmptyResultSchema = z.object({}).strict()
export const LogLevelSchema = z.enum(["trace", "debug", "info", "warn", "error", "off"])
export const InstanceParamsSchema = z.object({ instanceID: z.string().min(1) })
export const HeaderSchema = z.array(z.string()).length(2)
export const HeadersSchema = z.array(HeaderSchema)
export const StreamDescriptorSchema = z.object({
  streamID: z.string().min(1),
  length: z.number().int().nonnegative().optional(),
})
export const StreamReadParamsSchema = z.object({
  instanceID: z.string().min(1),
  streamID: z.string().min(1),
  maxBytes: z.number().int().positive().max(MAX_STREAM_CHUNK_BYTES).optional(),
})
export const StreamReadResultSchema = z.object({
  data: z.string().regex(/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/),
  eof: z.boolean(),
})
export const StreamCancelParamsSchema = StreamReadParamsSchema.omit({ maxBytes: true }).extend({
  reason: z.string().optional(),
})
export const CancelResultSchema = z.object({ cancelled: z.boolean() })
export const CloseResultSchema = z.object({ closed: z.boolean() })
export const ReleaseResultSchema = z.object({ released: z.boolean() })

export const RpcErrorObjectSchema = z.object({
  code: z.number().int(),
  message: z.string(),
  data: JsonValueSchema.optional(),
})
export const RpcRequestSchema = z.object({
  jsonrpc: z.literal("2.0"),
  id: z.string().min(1),
  method: z.string().min(1),
  params: JsonValueSchema.optional(),
})
export const RpcNotificationSchema = RpcRequestSchema.omit({ id: true })
export const RpcSuccessResponseSchema = z.object({
  jsonrpc: z.literal("2.0"),
  id: z.string().min(1),
  result: JsonValueSchema,
})
export const RpcErrorResponseSchema = z.object({
  jsonrpc: z.literal("2.0"),
  id: z.string().min(1),
  error: RpcErrorObjectSchema,
})
export const RpcMessageSchema = z.union([
  RpcRequestSchema,
  RpcNotificationSchema,
  RpcSuccessResponseSchema,
  RpcErrorResponseSchema,
])

export const DiagnosticSchema = z.object({
  severity: z.enum(["debug", "info", "warning", "error"]),
  code: z.string(),
  message: z.string(),
  plugin: z.string().optional(),
  method: z.string().optional(),
  data: JsonValueSchema.optional(),
})
export const PluginDeclarationSchema = z.object({
  spec: z.string().min(1),
  options: JsonObjectSchema.optional(),
  baseDirectory: z.string().optional(),
})
export const PluginPrepareFailureSchema = z.object({
  spec: z.string(),
  stage: z.enum(["declaration", "resolve", "install", "entry", "compatibility", "load", "shape"]),
  message: z.string(),
})
export const ToolAttachmentSchema = z.object({
  type: z.literal("file"),
  mime: z.string(),
  url: z.string(),
  filename: z.string().optional(),
})
export const ToolResultSchema = z.union([
  z.string(),
  z.object({
    title: z.string().optional(),
    output: z.string(),
    metadata: JsonObjectSchema.optional(),
    attachments: z.array(ToolAttachmentSchema).optional(),
  }),
])
export const ToolRegistrationSchema = z.object({
  registrationID: z.string().min(1),
  id: z.string().min(1),
  plugin: JsonObjectSchema.optional(),
  description: z.string(),
  parameters: JsonValueSchema,
})
export const AuthRuleSchema = z.object({
  key: z.string(),
  op: z.enum(["eq", "neq"]),
  value: z.string(),
})
export const AuthPromptSchema = z.object({
  type: z.enum(["text", "select"]),
  promptIndex: z.number().int().nonnegative(),
  key: z.string(),
  message: z.string(),
  placeholder: z.string().optional(),
  options: z.array(z.object({ label: z.string(), value: z.string(), hint: z.string().optional() })).optional(),
  when: AuthRuleSchema.optional(),
  hasValidate: z.boolean(),
  hasCondition: z.boolean(),
})
export const AuthRegistrationSchema = z.object({
  provider: z.string().min(1),
  plugin: JsonObjectSchema.optional(),
  hasLoader: z.boolean(),
  methods: z.array(
    z.object({
      type: z.enum(["oauth", "api"]),
      label: z.string(),
      methodIndex: z.number().int().nonnegative(),
      hasAuthorize: z.boolean(),
      prompts: z.array(AuthPromptSchema),
    }),
  ),
})
export const ProviderRegistrationSchema = z.object({
  provider: z.string().min(1),
  plugin: JsonObjectSchema.optional(),
  hasModels: z.boolean(),
})
export const WorkspaceRegistrationSchema = z.object({
  registrationID: z.string().min(1),
  type: z.string().min(1),
  plugin: JsonObjectSchema.optional(),
  name: z.string(),
  description: z.string(),
})
export const PluginMetaSchema = z.object({
  id: z.string().optional(),
  spec: z.string().min(1),
  entry: z.string().min(1),
  index: z.number().int().nonnegative(),
})
export const ConfigContributorSchema = z.object({
  plugin: PluginMetaSchema,
  outcome: z.enum(["applied", "failed"]),
})
export const ConfigContributionSchema = ConfigContributorSchema.extend({
  config: JsonObjectSchema,
})
export const AuthSuccessSchema = z.union([
  z.object({
    type: z.literal("success"),
    provider: z.string().optional(),
    refresh: z.string(),
    access: z.string(),
    expires: z.number(),
    accountId: z.string().optional(),
    enterpriseUrl: z.string().optional(),
  }),
  z.object({
    type: z.literal("success"),
    provider: z.string().optional(),
    key: z.string(),
    metadata: z.record(z.string(), z.string()).optional(),
  }),
])
export const AuthFailedSchema = z.object({ type: z.literal("failed") })
export const AuthFetchRequestSchema = z.object({
  url: z.string().url(),
  method: z.string().min(1).optional(),
  headers: HeadersSchema.optional(),
  body: StreamDescriptorSchema.optional(),
})
export const HttpResponseSchema = z.object({
  status: z.number().int().min(100).max(599),
  statusText: z.string().optional(),
  headers: HeadersSchema,
  body: StreamDescriptorSchema.optional(),
})

type MethodDefinition = { params: z.ZodType; result: z.ZodType }
type MethodDefinitions = Record<string, MethodDefinition>

export const HostMethodSchemas = {
  "host.plugins.prepare": {
    params: z.object({
      plugins: z.array(PluginDeclarationSchema),
      configurationFingerprint: z.string().min(1).optional(),
      defaultBaseDirectory: z.string().optional(),
    }),
    result: z.object({
      configurationFingerprint: z.string().min(1).optional(),
      prepared: z.array(z.object({
        spec: z.string(),
        source: z.enum(["file", "npm"]),
        target: z.string(),
        entry: z.string(),
        cache: z.enum(["hit", "installed", "validated"]),
        version: z.string().optional(),
      })),
      failed: z.array(PluginPrepareFailureSchema),
      diagnostics: z.array(DiagnosticSchema),
    }),
  },
  "host.instance.open": {
    params: z.object({
      instanceID: z.string().min(1),
      generationKey: z.string().min(1).optional(),
      revision: z.string().min(1).optional(),
      project: JsonValueSchema,
      config: JsonObjectSchema,
      directory: z.string(),
      worktree: z.string(),
      plugins: z.array(PluginDeclarationSchema),
      configurationFingerprint: z.string().min(1).optional(),
    }),
    result: z.object({
      instanceID: z.string().min(1),
      generationKey: z.string().min(1).optional(),
      revision: z.string().min(1).optional(),
      config: JsonObjectSchema,
      configContributors: z.array(ConfigContributorSchema),
      configContributions: z.array(ConfigContributionSchema),
      diagnostics: z.array(DiagnosticSchema),
      hooks: z.array(z.string()),
      tools: z.array(ToolRegistrationSchema),
      auth: z.array(AuthRegistrationSchema),
      providers: z.array(ProviderRegistrationSchema),
      workspaces: z.array(WorkspaceRegistrationSchema),
      gatewayURL: z.string().url(),
    }),
  },
  "host.instance.close": { params: InstanceParamsSchema, result: CloseResultSchema },
  "host.log.setLevel": { params: z.object({ level: LogLevelSchema }), result: z.object({ level: LogLevelSchema }) },
  "host.shutdown": { params: EmptyResultSchema, result: CloseResultSchema },
  "host.hook.call": {
    params: z.object({
      instanceID: z.string().min(1),
      generationKey: z.string().min(1).optional(),
      revision: z.string().min(1).optional(),
      hook: z.string().min(1),
      input: JsonValueSchema,
      output: JsonValueSchema,
    }),
    result: z.object({
      instanceID: z.string().min(1).optional(),
      generationKey: z.string().min(1).optional(),
      revision: z.string().min(1).optional(),
      hook: z.string().min(1),
      input: JsonValueSchema,
      output: JsonValueSchema,
    }),
  },
  "host.event.emit": {
    params: z.object({ instanceID: z.string().min(1), event: JsonValueSchema }),
    result: z.object({ accepted: z.literal(true) }),
  },
  "host.tool.execute": {
    params: z.object({
      instanceID: z.string().min(1),
      generationKey: z.string().min(1).optional(),
      revision: z.string().min(1).optional(),
      executionID: z.string().min(1),
      registrationID: z.string().min(1),
      args: JsonValueSchema,
      context: z.object({
        sessionID: z.string(),
        messageID: z.string(),
        agent: z.string(),
        callID: z.string().optional(),
      }),
    }),
    result: z.object({
      instanceID: z.string().min(1).optional(),
      generationKey: z.string().min(1).optional(),
      revision: z.string().min(1).optional(),
      executionID: z.string().min(1),
      result: ToolResultSchema,
    }),
  },
  "host.tool.cancel": {
    params: z.object({
      instanceID: z.string().min(1),
      generationKey: z.string().min(1).optional(),
      revision: z.string().min(1).optional(),
      executionID: z.string().min(1),
      reason: z.string().optional(),
    }),
    result: CancelResultSchema,
  },
  "host.auth.prompt.evaluate": {
    params: z.object({
      instanceID: z.string().min(1),
      provider: z.string().min(1),
      methodIndex: z.number().int().nonnegative(),
      promptIndex: z.number().int().nonnegative(),
      operation: z.enum(["validate", "condition"]),
      value: z.string().optional(),
      inputs: z.record(z.string(), z.string()),
    }),
    result: z.union([
      z.object({ operation: z.literal("validate"), error: z.string().optional() }),
      z.object({ operation: z.literal("condition"), active: z.boolean() }),
    ]),
  },
  "host.auth.authorize": {
    params: z.object({
      instanceID: z.string().min(1),
      provider: z.string().min(1),
      methodIndex: z.number().int().nonnegative(),
      inputs: z.record(z.string(), z.string()).optional(),
    }),
    result: z.union([
      z.object({
        type: z.literal("oauth"),
        flowID: z.string().min(1),
        url: z.string().url(),
        instructions: z.string(),
        method: z.enum(["auto", "code"]),
      }),
      z.object({ type: z.literal("api"), result: z.union([AuthSuccessSchema, AuthFailedSchema]).optional() }),
    ]),
  },
  "host.auth.callback": {
    params: z.object({ instanceID: z.string().min(1), flowID: z.string().min(1), code: z.string().optional() }),
    result: z.union([AuthSuccessSchema, AuthFailedSchema]),
  },
  "host.auth.flow.cancel": {
    params: z.object({ instanceID: z.string().min(1), flowID: z.string().min(1), reason: z.string().optional() }),
    result: CancelResultSchema,
  },
  "host.auth.loader": {
    params: z.object({
      instanceID: z.string().min(1),
      provider: z.string().min(1),
      providerInfo: JsonValueSchema,
    }),
    result: z.object({ value: JsonObjectSchema, fetchID: z.string().min(1).optional() }),
  },
  "host.auth.fetch": {
    params: z.object({
      instanceID: z.string().min(1),
      fetchID: z.string().min(1),
      requestID: z.string().min(1),
      request: AuthFetchRequestSchema,
    }),
    result: HttpResponseSchema,
  },
  "host.auth.fetch.cancel": {
    params: z.object({ instanceID: z.string().min(1), requestID: z.string().min(1), reason: z.string().optional() }),
    result: CancelResultSchema,
  },
  "host.auth.fetch.release": {
    params: z.object({ instanceID: z.string().min(1), fetchID: z.string().min(1) }),
    result: ReleaseResultSchema,
  },
  "host.provider.models": {
    params: z.object({
      instanceID: z.string().min(1),
      providerID: z.string().min(1),
      provider: JsonValueSchema,
      auth: JsonValueSchema.optional(),
    }),
    result: z.object({ models: JsonObjectSchema }),
  },
  "host.workspace.configure": {
    params: z.object({ instanceID: z.string().min(1), registrationID: z.string().min(1), config: JsonValueSchema }),
    result: z.object({ config: JsonValueSchema }),
  },
  "host.workspace.create": {
    params: z.object({
      instanceID: z.string().min(1),
      registrationID: z.string().min(1),
      config: JsonValueSchema,
      env: z.record(z.string(), z.string().nullable()),
      from: JsonValueSchema.optional(),
    }),
    result: EmptyResultSchema,
  },
  "host.workspace.remove": {
    params: z.object({ instanceID: z.string().min(1), registrationID: z.string().min(1), config: JsonValueSchema }),
    result: EmptyResultSchema,
  },
  "host.workspace.target": {
    params: z.object({ instanceID: z.string().min(1), registrationID: z.string().min(1), config: JsonValueSchema }),
    result: z.object({
      target: z.union([
        z.object({ type: z.literal("local"), directory: z.string() }),
        z.object({ type: z.literal("remote"), url: z.string().url(), headers: HeadersSchema.optional() }),
      ]),
    }),
  },
  "host.stream.read": { params: StreamReadParamsSchema, result: StreamReadResultSchema },
  "host.stream.cancel": { params: StreamCancelParamsSchema, result: CancelResultSchema },
} satisfies MethodDefinitions

export const BackendMethodSchemas = {
  "backend.handshake": {
    params: z.object({
      token: z.string().min(1),
      protocolVersion: z.literal(PROTOCOL_VERSION),
      opencodeVersion: z.literal(OPENCODE_VERSION),
      maxFrameBytes: z.number().int().positive().max(MAX_MAX_FRAME_BYTES),
      capabilities: z.array(z.string().min(1)).optional(),
    }),
    result: z.object({
      protocolVersion: z.literal(PROTOCOL_VERSION),
      maxFrameBytes: z.number().int().positive().max(MAX_MAX_FRAME_BYTES),
      cacheDirectory: z.string(),
      capabilities: z.array(z.string().min(1)).optional(),
    }),
  },
  "backend.http.request": {
    params: z.object({
      instanceID: z.string().min(1),
      requestID: z.string().min(1),
      method: z.string().min(1),
      path: z.string(),
      headers: HeadersSchema,
      body: StreamDescriptorSchema.optional(),
    }),
    result: HttpResponseSchema,
  },
  "backend.auth.get": {
    params: z.object({ instanceID: z.string().min(1), providerID: z.string().min(1) }),
    result: z.object({ auth: JsonValueSchema.nullable() }),
  },
  "backend.tool.ask": {
    params: z.object({
      instanceID: z.string().min(1),
      generationKey: z.string().min(1).optional(),
      revision: z.string().min(1).optional(),
      executionID: z.string().min(1),
      permission: z.string(),
      patterns: z.array(z.string()),
      always: z.array(z.string()),
      metadata: JsonObjectSchema,
    }),
    result: EmptyResultSchema,
  },
  "backend.tool.metadata": {
    params: z.object({
      instanceID: z.string().min(1),
      generationKey: z.string().min(1).optional(),
      revision: z.string().min(1).optional(),
      executionID: z.string().min(1),
      title: z.string().optional(),
      metadata: JsonObjectSchema.optional(),
    }),
    result: EmptyResultSchema,
  },
  "backend.diagnostic.publish": {
    params: z.object({ instanceID: z.string().min(1).optional(), diagnostic: DiagnosticSchema }),
    result: EmptyResultSchema,
  },
  "backend.stream.read": { params: StreamReadParamsSchema, result: StreamReadResultSchema },
  "backend.stream.cancel": { params: StreamCancelParamsSchema, result: CancelResultSchema },
} satisfies MethodDefinitions

export type HostMethod = keyof typeof HostMethodSchemas
export type BackendMethod = keyof typeof BackendMethodSchemas
export type MethodParams<T extends MethodDefinitions, K extends keyof T> = z.input<T[K]["params"]>
export type MethodResult<T extends MethodDefinitions, K extends keyof T> = z.output<T[K]["result"]>
export type HostMethodParams<K extends HostMethod> = MethodParams<typeof HostMethodSchemas, K>
export type HostMethodResult<K extends HostMethod> = MethodResult<typeof HostMethodSchemas, K>
export type BackendMethodParams<K extends BackendMethod> = MethodParams<typeof BackendMethodSchemas, K>
export type BackendMethodResult<K extends BackendMethod> = MethodResult<typeof BackendMethodSchemas, K>
export type StreamDescriptor = z.infer<typeof StreamDescriptorSchema>
export type StreamReadParams = z.infer<typeof StreamReadParamsSchema>
export type StreamReadResult = z.infer<typeof StreamReadResultSchema>
export type RpcErrorObject = z.infer<typeof RpcErrorObjectSchema>
