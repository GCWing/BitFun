import {
  MAX_STREAM_CHUNK_BYTES,
  StreamReadResultSchema,
  type StreamDescriptor,
  type StreamReadResult,
} from "./protocol"

type StreamEntry = {
  reader: {
    read(): Promise<{ done: boolean; value?: Uint8Array }>
    cancel(reason?: unknown): Promise<void>
    releaseLock(): void
  }
  remainder?: Uint8Array
  tail: Promise<void>
  done: boolean
  released: boolean
}

export type StreamRpcPeer = {
  request<Result = unknown>(method: string, params: unknown): Promise<Result>
}

export class StreamRegistry {
  readonly #prefix: string
  readonly #streams = new Map<string, StreamEntry>()
  #sequence = 0

  constructor(prefix = "host") {
    if (!prefix) throw new TypeError("Stream ID prefix must not be empty")
    this.#prefix = prefix
  }

  get size() {
    return this.#streams.size
  }

  add(stream: ReadableStream<Uint8Array>, length?: number): StreamDescriptor {
    if (length !== undefined && (!Number.isSafeInteger(length) || length < 0)) {
      throw new RangeError("Stream length must be a non-negative safe integer")
    }
    const streamID = `${this.#prefix}-stream:${++this.#sequence}`
    this.#streams.set(streamID, { reader: stream.getReader(), tail: Promise.resolve(), done: false, released: false })
    return { streamID, ...(length === undefined ? {} : { length }) }
  }

  register(stream: ReadableStream<Uint8Array>, length?: number) {
    return this.add(stream, length)
  }

  async read(input: { streamID: string; maxBytes?: number }): Promise<StreamReadResult> {
    const maxBytes = input.maxBytes ?? MAX_STREAM_CHUNK_BYTES
    if (!Number.isInteger(maxBytes) || maxBytes < 1 || maxBytes > MAX_STREAM_CHUNK_BYTES) {
      throw new RangeError(`maxBytes must be an integer between 1 and ${MAX_STREAM_CHUNK_BYTES}`)
    }
    const entry = this.#streams.get(input.streamID)
    if (!entry) return { data: "", eof: true }

    return this.#serialized(entry, async () => {
      if (entry.done) return { data: "", eof: true }
      if (entry.remainder?.byteLength) return this.#take(entry, maxBytes)

      while (true) {
        let result: { done: boolean; value?: Uint8Array }
        try {
          result = await entry.reader.read()
        } catch (error) {
          entry.done = true
          this.#streams.delete(input.streamID)
          throw error
        }
        if (result.done) {
          entry.done = true
          this.#streams.delete(input.streamID)
          this.#release(entry)
          return { data: "", eof: true }
        }
        if (!result.value || result.value.byteLength === 0) continue
        entry.remainder = result.value
        return this.#take(entry, maxBytes)
      }
    })
  }

  async cancel(input: { streamID: string; reason?: string }) {
    const entry = this.#streams.get(input.streamID)
    if (!entry) return { cancelled: false }
    this.#streams.delete(input.streamID)
    if (entry.done) return { cancelled: false }
    entry.done = true
    entry.remainder = undefined
    try {
      await entry.reader.cancel(input.reason)
      await entry.tail
    } finally {
      this.#release(entry)
    }
    return { cancelled: true }
  }

  async cancelAll(reason = "Stream registry closed") {
    await Promise.all(Array.from(this.#streams, ([streamID]) => this.cancel({ streamID, reason })))
  }

  #take(entry: StreamEntry, maxBytes: number): StreamReadResult {
    const value = entry.remainder!
    const data = value.byteLength <= maxBytes ? value : value.subarray(0, maxBytes)
    entry.remainder = value.byteLength <= maxBytes ? undefined : value.subarray(maxBytes)
    return { data: Buffer.from(data).toString("base64"), eof: false }
  }

  #release(entry: StreamEntry) {
    if (entry.released) return
    entry.released = true
    entry.reader.releaseLock()
  }

  async #serialized<Result>(entry: StreamEntry, operation: () => Promise<Result>) {
    const previous = entry.tail
    const deferred = Promise.withResolvers<void>()
    entry.tail = deferred.promise
    await previous
    try {
      return await operation()
    } finally {
      deferred.resolve()
    }
  }
}

export function remoteReadable(
  peer: StreamRpcPeer,
  methodPrefix: "backend" | "host",
  descriptor: StreamDescriptor,
  params: Record<string, unknown> = {},
) {
  let released = false
  return new ReadableStream<Uint8Array>({
    async pull(controller) {
      try {
        const result = StreamReadResultSchema.parse(
          await peer.request(`${methodPrefix}.stream.read`, {
            ...params,
            streamID: descriptor.streamID,
            maxBytes: MAX_STREAM_CHUNK_BYTES,
          }),
        )
        if (result.data) controller.enqueue(Buffer.from(result.data, "base64"))
        if (!result.eof) return
        released = true
        controller.close()
      } catch (error) {
        controller.error(error)
        if (released) return
        released = true
        void peer
          .request(`${methodPrefix}.stream.cancel`, {
            ...params,
            streamID: descriptor.streamID,
            reason: error instanceof Error ? error.message : String(error),
          })
          .catch(() => {})
      }
    },
    async cancel(reason) {
      if (released) return
      released = true
      await peer.request(`${methodPrefix}.stream.cancel`, {
        ...params,
        streamID: descriptor.streamID,
        ...(reason === undefined ? {} : { reason: reason instanceof Error ? reason.message : String(reason) }),
      })
    },
  })
}
