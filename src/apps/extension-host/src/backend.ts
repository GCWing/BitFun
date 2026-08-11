export type RpcConnection = {
  request<Result = unknown>(method: string, params: unknown, options?: { signal?: AbortSignal }): Promise<Result>
  notify(method: string, params: unknown): Promise<void> | void
}

export type StreamDescriptor = {
  streamID: string
  length?: number
}

export type StreamBridge = {
  register(instanceID: string, stream: ReadableStream<Uint8Array>, length?: number): StreamDescriptor
  remote(methodPrefix: "backend" | "host", instanceID: string, descriptor: StreamDescriptor): ReadableStream<Uint8Array>
  cancel(instanceID: string, descriptor: StreamDescriptor): Promise<void>
  cancelAll(instanceID: string): Promise<void>
  cancelRemote?(instanceID: string, descriptor: StreamDescriptor, reason?: string): Promise<void>
}

export type Diagnostic = {
  level: "debug" | "info" | "warn" | "error"
  message: string
  instanceID?: string
  plugin?: {
    id?: string
    spec: string
  }
  operation?: string
  error?: {
    name?: string
    message: string
    stack?: string
    cause?: unknown
  }
}

export async function publishDiagnostic(rpc: RpcConnection, diagnostic: Diagnostic) {
  const { instanceID, ...value } = diagnostic
  await rpc.notify("backend.diagnostic.publish", {
    ...(instanceID ? { instanceID } : {}),
    diagnostic: {
      severity: value.level === "warn" ? "warning" : value.level,
      code: value.operation ?? "extension_host",
      message: value.message,
      plugin: value.plugin?.id ?? value.plugin?.spec,
      method: value.operation,
      data: value.error
        ? {
            ...(value.error.name ? { name: value.error.name } : {}),
            message: value.error.message,
            ...(value.error.stack ? { stack: value.error.stack } : {}),
            ...(value.error.cause === undefined ? {} : { cause: String(value.error.cause) }),
          }
        : undefined,
    },
  })
}
