import {
  DEFAULT_MAX_FRAME_BYTES,
  JsonValueSchema,
  MAX_MAX_FRAME_BYTES,
  RpcMessageSchema,
  type RpcErrorObject,
} from "./protocol"
import { logEvent, logError, rpcMessageSummary } from "./log"

export type RpcTransport = {
  write(data: Uint8Array): number | void | Promise<number | void>
  end?(): void
  terminate?(): void
}

export type RpcHandler = (params: unknown) => unknown | Promise<unknown>

export type RpcPeerOptions = {
  idPrefix: string
  maxFrameBytes?: number
  onEof?: (error?: Error) => void | Promise<void>
  onError?: (error: Error) => void
  onRequest?: (method: string) => void
}

export type RpcRequestOptions = {
  signal?: AbortSignal
  onResult?: (value: unknown) => void
}

type PendingRequest = {
  resolve(value: unknown): void
  reject(error: Error): void
  cleanup(): void
  onResult?: (value: unknown) => void
}

export class RpcError extends Error {
  readonly code: number
  readonly data?: unknown

  constructor(code: number, message: string, data?: unknown) {
    super(message)
    this.name = "RpcError"
    this.code = code
    this.data = data
  }
}

export class RpcConnectionClosedError extends Error {
  constructor(message = "JSON-RPC connection is closed", options?: ErrorOptions) {
    super(message, options)
    this.name = "RpcConnectionClosedError"
  }
}

export class RpcProtocolError extends RpcError {
  constructor(code: -32700 | -32600, message: string, data?: unknown) {
    super(code, message, data)
    this.name = "RpcProtocolError"
  }
}

export class RpcPeer {
  readonly closed: Promise<void>
  readonly #transport: RpcTransport
  readonly #idPrefix: string
  readonly #handlers = new Map<string, RpcHandler>()
  readonly #pending = new Map<string, PendingRequest>()
  readonly #onEof?: RpcPeerOptions["onEof"]
  readonly #onError?: RpcPeerOptions["onError"]
  readonly #onRequest?: RpcPeerOptions["onRequest"]
  readonly #resolveClosed: () => void
  #buffer = new Uint8Array()
  #sequence = 0
  #ended = false
  #closeError?: Error
  #maxFrameBytes: number
  #writeTail = Promise.resolve()

  constructor(transport: RpcTransport, options: RpcPeerOptions) {
    if (!options.idPrefix) throw new TypeError("JSON-RPC ID prefix must not be empty")
    this.#transport = transport
    this.#idPrefix = options.idPrefix
    this.#maxFrameBytes = validateMaxFrameBytes(options.maxFrameBytes ?? DEFAULT_MAX_FRAME_BYTES)
    this.#onEof = options.onEof
    this.#onError = options.onError
    this.#onRequest = options.onRequest
    const deferred = Promise.withResolvers<void>()
    this.closed = deferred.promise
    this.#resolveClosed = deferred.resolve
  }

  get maxFrameBytes() {
    return this.#maxFrameBytes
  }

  get closeError() {
    return this.#closeError
  }

  setMaxFrameBytes(value: number) {
    this.#maxFrameBytes = validateMaxFrameBytes(value)
  }

  handle<Params = unknown, Result = unknown>(method: string, handler: (params: Params) => Result | Promise<Result>) {
    if (!method) throw new TypeError("JSON-RPC method must not be empty")
    if (this.#handlers.has(method)) throw new Error(`JSON-RPC handler already registered for ${method}`)
    this.#handlers.set(method, handler as RpcHandler)
    return () => {
      if (this.#handlers.get(method) === handler) this.#handlers.delete(method)
    }
  }

  async request<Result = unknown>(method: string, params: unknown = {}, options: RpcRequestOptions = {}) {
    this.#assertOpen()
    if (options.signal?.aborted) throw abortError(options.signal.reason)
    const id = `${this.#idPrefix}:${++this.#sequence}`
    const deferred = Promise.withResolvers<unknown>()
    const abort = () => {
      this.#pending.delete(id)
      deferred.reject(abortError(options.signal?.reason))
    }
    const cleanup = () => options.signal?.removeEventListener("abort", abort)
    this.#pending.set(id, {
      resolve: deferred.resolve,
      reject: deferred.reject,
      cleanup,
      ...(options.onResult ? { onResult: options.onResult } : {}),
    })
    options.signal?.addEventListener("abort", abort, { once: true })

    try {
      await this.#send({ jsonrpc: "2.0", id, method, params })
    } catch (error) {
      const pending = this.#pending.get(id)
      if (pending) {
        this.#pending.delete(id)
        pending.cleanup()
        pending.reject(asError(error))
      }
    }
    return (await deferred.promise) as Result
  }

  notify(method: string, params: unknown = {}) {
    this.#assertOpen()
    return this.#send({ jsonrpc: "2.0", method, params })
  }

  receive(data: Uint8Array) {
    if (this.#ended || data.byteLength === 0) return
    this.#buffer = concatBytes(this.#buffer, data)

    while (this.#buffer.byteLength >= 4) {
      const length = new DataView(this.#buffer.buffer, this.#buffer.byteOffset, 4).getUint32(0, false)
      if (length === 0) {
        this.#fail(new RpcProtocolError(-32600, "JSON-RPC frame must not be empty"))
        return
      }
      if (length > this.#maxFrameBytes) {
        this.#fail(
          new RpcProtocolError(-32600, `JSON-RPC frame length ${length} exceeds limit ${this.#maxFrameBytes}`, {
            length,
            maxFrameBytes: this.#maxFrameBytes,
          }),
        )
        return
      }
      if (this.#buffer.byteLength < length + 4) return
      const payload = this.#buffer.slice(4, length + 4)
      this.#buffer = this.#buffer.slice(length + 4)
      this.#receivePayload(payload)
      if (this.#ended) return
    }
  }

  end(error?: Error) {
    this.#finish(error)
  }

  close(error?: Error) {
    if (this.#ended) return
    try {
      this.#transport.end?.()
    } catch (cause) {
      error ??= asError(cause)
    }
    this.#finish(error)
  }

  async flushAndClose(error?: Error) {
    await this.#writeTail
    this.close(error)
  }

  #receivePayload(payload: Uint8Array) {
    let value: unknown
    try {
      value = JSON.parse(new TextDecoder().decode(payload))
    } catch (error) {
      this.#fail(new RpcProtocolError(-32700, "Invalid JSON-RPC JSON payload", errorDetails(error)))
      return
    }

    const parsed = RpcMessageSchema.safeParse(value)
    if (!parsed.success) {
      const id = responseID(value)
      if (!id) {
        this.#fail(new RpcProtocolError(-32600, "Invalid JSON-RPC message", { issues: parsed.error.issues }))
        return
      }
      void this.#sendError(id, {
        code: -32600,
        message: "Invalid JSON-RPC message",
        data: safeErrorData({ issues: parsed.error.issues }),
      })
      return
    }

    const message = parsed.data
    logEvent("rpc.receive", { ...rpcMessageSummary(message), frame_bytes: payload.byteLength }, "debug")
    if ("method" in message) {
      void this.#dispatch(message.method, message.params, "id" in message ? message.id : undefined)
      return
    }

    const pending = this.#pending.get(message.id)
    if (!pending) return
    this.#pending.delete(message.id)
    pending.cleanup()
    if ("error" in message) {
      pending.reject(new RpcError(message.error.code, message.error.message, message.error.data))
      return
    }
    try {
      pending.onResult?.(message.result)
    } catch (error) {
      pending.reject(asError(error))
      return
    }
    pending.resolve(message.result)
  }

  async #dispatch(method: string, params: unknown, id?: string) {
    try {
      this.#onRequest?.(method)
    } catch (error) {
      if (id) await this.#sendError(id, rpcErrorObject(error))
      else this.#reportError(asError(error))
      return
    }
    const handler = this.#handlers.get(method)
    if (!handler) {
      if (id) await this.#sendError(id, { code: -32601, message: `Method not found: ${method}` })
      return
    }

    let result: unknown
    try {
      result = await handler(params)
    } catch (error) {
      if (id) {
        await this.#sendError(id, rpcErrorObject(error))
        return
      }
      this.#reportError(asError(error))
      return
    }
    if (!id) return

    try {
      await this.#send({ jsonrpc: "2.0", id, result: result === undefined ? null : result })
    } catch (error) {
      if (error instanceof RangeError) {
        await this.#sendError(id, {
          code: -32000,
          message: "JSON-RPC response exceeds the negotiated frame limit",
          data: { kind: "response_too_large", maxFrameBytes: this.#maxFrameBytes },
        })
      } else if (!this.#ended) {
        this.#fail(asError(error))
      }
    }
  }

  async #sendError(id: string, error: RpcErrorObject) {
    try {
      await this.#send({ jsonrpc: "2.0", id, error })
    } catch (cause) {
      this.#reportError(asError(cause))
    }
  }

  async #send(message: unknown) {
    this.#assertOpen()
    const frame = encodeFrame(message, this.#maxFrameBytes)
    logEvent("rpc.send", { ...rpcMessageSummary(message), frame_bytes: frame.byteLength - 4 }, "debug")
    const write = this.#writeTail.then(async () => {
      this.#assertOpen()
      const written = await this.#transport.write(frame)
      if (typeof written === "number" && written < frame.byteLength) {
        throw new Error(`JSON-RPC transport accepted ${written} of ${frame.byteLength} bytes`)
      }
    })
    this.#writeTail = write.catch(() => {})
    try {
      await write
    } catch (error) {
      this.#fail(asError(error))
      throw error
    }
  }

  #assertOpen() {
    if (this.#ended) throw new RpcConnectionClosedError(undefined, { cause: this.#closeError })
  }

  #fail(error: Error) {
    if (this.#ended) return
    this.#finish(error)
    try {
      if (this.#transport.terminate) this.#transport.terminate()
      if (!this.#transport.terminate) this.#transport.end?.()
    } catch {
      // The original protocol or transport failure remains authoritative.
    }
    this.#reportError(error)
  }

  #finish(error?: Error) {
    if (this.#ended) return
    this.#ended = true
    this.#closeError = error
    this.#buffer = new Uint8Array()
    const reason = new RpcConnectionClosedError(undefined, { cause: error })
    for (const pending of this.#pending.values()) {
      pending.cleanup()
      pending.reject(reason)
    }
    this.#pending.clear()
    this.#resolveClosed()
    if (this.#onEof) void Promise.resolve(this.#onEof(error)).catch((cause) => this.#reportError(asError(cause)))
  }

  #reportError(error: Error) {
    if (this.#onError) {
      this.#onError(error)
      return
    }
    logError("rpc.failure", error)
  }
}

export function encodeFrame(message: unknown, maxFrameBytes = DEFAULT_MAX_FRAME_BYTES) {
  const limit = validateMaxFrameBytes(maxFrameBytes)
  const text = JSON.stringify(message, (_key, value: unknown) => {
    if (typeof value === "bigint") throw new TypeError("JSON-RPC values cannot contain BigInt")
    if (typeof value === "function" || typeof value === "symbol") {
      throw new TypeError(`JSON-RPC values cannot contain ${typeof value}`)
    }
    if (typeof value === "number" && !Number.isFinite(value)) {
      throw new TypeError("JSON-RPC values cannot contain non-finite numbers")
    }
    return value
  })
  if (text === undefined) throw new TypeError("JSON-RPC message is not serializable")
  const payload = new TextEncoder().encode(text)
  if (payload.byteLength === 0 || payload.byteLength > limit) {
    throw new RangeError(`JSON-RPC payload length ${payload.byteLength} exceeds limit ${limit}`)
  }
  const frame = new Uint8Array(payload.byteLength + 4)
  new DataView(frame.buffer).setUint32(0, payload.byteLength, false)
  frame.set(payload, 4)
  return frame
}

export async function connectRpcPeer(
  address: string,
  options: Omit<RpcPeerOptions, "idPrefix"> & { idPrefix?: string } = {},
) {
  const target = parseRpcAddress(address)
  let socket: Bun.Socket
  let drain: ReturnType<typeof Promise.withResolvers<void>> | undefined
  const peer = new RpcPeer(
    {
      async write(data) {
        let offset = 0
        while (offset < data.byteLength) {
          const written = socket.write(data, offset, data.byteLength - offset)
          if (written < 0) throw new RpcConnectionClosedError("JSON-RPC socket closed while writing")
          offset += written
          if (offset === data.byteLength) return offset
          drain ??= Promise.withResolvers<void>()
          await drain.promise
        }
        return offset
      },
      end: () => socket.end(),
      terminate: () => socket.terminate(),
    },
    { ...options, idPrefix: options.idPrefix ?? "host" },
  )
  socket = await Bun.connect({
    hostname: target.hostname,
    port: target.port,
    socket: {
      data(_socket, data) {
        peer.receive(data)
      },
      drain() {
        drain?.resolve()
        drain = undefined
      },
      close() {
        drain?.reject(new RpcConnectionClosedError())
        drain = undefined
        peer.end()
      },
      error(_socket, error) {
        drain?.reject(error)
        drain = undefined
        peer.end(error)
      },
    },
  })
  return peer
}

export function parseRpcAddress(address: string) {
  const url = new URL(address.includes("://") ? address : `tcp://${address}`)
  if (url.protocol !== "tcp:") throw new TypeError(`Unsupported RPC address protocol: ${url.protocol}`)
  if (!url.hostname || !url.port) throw new TypeError(`RPC address must include a host and port: ${address}`)
  const port = Number(url.port)
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new TypeError(`Invalid RPC port: ${url.port}`)
  return { hostname: url.hostname, port }
}

function validateMaxFrameBytes(value: number) {
  if (!Number.isInteger(value) || value < 1 || value > MAX_MAX_FRAME_BYTES) {
    throw new RangeError(`maxFrameBytes must be an integer between 1 and ${MAX_MAX_FRAME_BYTES}`)
  }
  return value
}

function rpcErrorObject(error: unknown): RpcErrorObject {
  if (hasNumericCode(error)) {
    return {
      code: error.code,
      message: error instanceof Error ? error.message : String(Reflect.get(error, "message") ?? "JSON-RPC error"),
      ...(error.data === undefined ? {} : { data: safeErrorData(error.data) }),
    }
  }
  const value = asError(error)
  return {
    code: -32603,
    message: value.message || "Internal error",
    data: {
      name: value.name,
      message: value.message,
      ...(value.stack ? { stack: value.stack } : {}),
    },
  }
}

function hasNumericCode(value: unknown): value is { code: number; data?: unknown } {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof Reflect.get(value, "code") === "number" &&
    Number.isInteger(Reflect.get(value, "code"))
  )
}

function safeErrorData(value: unknown) {
  const parsed = JsonValueSchema.safeParse(value)
  if (parsed.success) return parsed.data
  return { kind: "invalid_error_data", message: "Thrown JSON-RPC error data was not JSON-compatible" }
}

function responseID(value: unknown) {
  if (typeof value !== "object" || value === null) return undefined
  const id = Reflect.get(value, "id")
  return typeof id === "string" && id ? id : undefined
}

function errorDetails(error: unknown) {
  const value = asError(error)
  return { name: value.name, message: value.message }
}

function abortError(reason: unknown) {
  if (reason instanceof Error) return reason
  return new DOMException(typeof reason === "string" ? reason : "The operation was aborted", "AbortError")
}

function asError(error: unknown) {
  return error instanceof Error ? error : new Error(String(error))
}

function concatBytes(left: Uint8Array, right: Uint8Array) {
  if (left.byteLength === 0) return right.slice()
  const result = new Uint8Array(left.byteLength + right.byteLength)
  result.set(left)
  result.set(right, left.byteLength)
  return result
}
