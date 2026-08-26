import { mkdir, mkdtemp, rm } from "node:fs/promises"
import path from "node:path"
import { tmpdir } from "node:os"
import { DEFAULT_MAX_FRAME_BYTES, HOST_CAPABILITIES, OPENCODE_VERSION, PROTOCOL_VERSION } from "../../src/protocol"
import { RpcPeer } from "../../src/rpc"

type Handshake = {
  token: string
  protocolVersion: number
  opencodeVersion: string
  maxFrameBytes: number
  capabilities: string[]
}

export async function launchExtensionHost(
  input: {
    token?: string
    acceptedToken?: string
    maxFrameBytes?: number
    logLevel?: string
    handshakeGate?: Promise<void>
  } = {},
) {
  const root = await mkdtemp(path.join(tmpdir(), "opencode-extension-host-process-"))
  const cacheDirectory = path.join(root, "cache")
  await mkdir(cacheDirectory, { recursive: true })

  const accepted = Promise.withResolvers<{
    peer: RpcPeer
    write(data: Uint8Array): void
  }>()
  const handshake = Promise.withResolvers<Handshake>()
  const peers = new WeakMap<object, RpcPeer>()
  const maxFrameBytes = input.maxFrameBytes ?? DEFAULT_MAX_FRAME_BYTES
  const server = Bun.listen({
    hostname: "127.0.0.1",
    port: 0,
    socket: {
      open(socket) {
        const peer = new RpcPeer(socket, { idPrefix: "backend" })
        peers.set(socket, peer)
        peer.handle("backend.handshake", async (value) => {
          const params = value as Handshake
          handshake.resolve(params)
          if (params.token !== (input.acceptedToken ?? "test-rpc-token")) {
            throw Object.assign(new Error("Invalid extension host RPC token"), {
              code: -32001,
              data: { kind: "authentication_failed" },
            })
          }
          await input.handshakeGate
          peer.setMaxFrameBytes(maxFrameBytes)
          return { protocolVersion: PROTOCOL_VERSION, maxFrameBytes, cacheDirectory }
        })
        accepted.resolve({
          peer,
          write(data) {
            socket.write(data)
          },
        })
      },
      data(socket, data) {
        peers.get(socket)?.receive(data)
      },
      close(socket) {
        peers.get(socket)?.end()
      },
      error(socket, error) {
        peers.get(socket)?.end(error)
      },
    },
  })
  const extensionHostDirectory = path.resolve(import.meta.dir, "..", "..")
  const child = Bun.spawn({
    cmd: [process.execPath, path.join(extensionHostDirectory, "src", "main.ts")],
    cwd: extensionHostDirectory,
    env: {
      ...process.env,
      OPENCODE_EXTENSION_HOST_RPC_ADDRESS: `127.0.0.1:${server.port}`,
      OPENCODE_EXTENSION_HOST_RPC_TOKEN: input.token ?? "test-rpc-token",
      OPENCODE_EXTENSION_HOST_LOG_LEVEL: input.logLevel ?? "debug",
    },
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  })
  const stdout = new Response(child.stdout).text()
  const stderr = new Response(child.stderr).text()

  try {
    const connection = await deadline(accepted.promise, 5_000, "extension host did not connect")
    const seenHandshake = await deadline(handshake.promise, 5_000, "extension host did not handshake")
    return {
      root,
      cacheDirectory,
      peer: connection.peer,
      write: connection.write,
      handshake: seenHandshake,
      child,
      stdout,
      stderr,
      waitForExit(timeout = 5_000) {
        return deadline(child.exited, timeout, "extension host did not exit")
      },
      async cleanup() {
        connection.peer.close()
        const exited = await Promise.race([child.exited.then(() => true), Bun.sleep(250).then(() => false)])
        if (!exited) child.kill()
        await child.exited
        server.stop(true)
        await rm(root, { recursive: true, force: true })
      },
    }
  } catch (error) {
    child.kill()
    await child.exited
    server.stop(true)
    await rm(root, { recursive: true, force: true })
    throw error
  }
}

export function rawFrame(payload: string) {
  const bytes = new TextEncoder().encode(payload)
  const frame = new Uint8Array(bytes.byteLength + 4)
  new DataView(frame.buffer).setUint32(0, bytes.byteLength, false)
  frame.set(bytes, 4)
  return frame
}

export function oversizedFrameHeader(length: number) {
  const frame = new Uint8Array(4)
  new DataView(frame.buffer).setUint32(0, length, false)
  return frame
}

export function expectedHandshake(token = "test-rpc-token") {
  return {
    token,
    protocolVersion: PROTOCOL_VERSION,
    opencodeVersion: OPENCODE_VERSION,
    maxFrameBytes: DEFAULT_MAX_FRAME_BYTES,
    capabilities: [...HOST_CAPABILITIES],
  }
}

async function deadline<T>(promise: Promise<T>, milliseconds: number, message: string) {
  const timeout = Promise.withResolvers<never>()
  const timer = setTimeout(() => timeout.reject(new Error(message)), milliseconds)
  try {
    return await Promise.race([promise, timeout.promise])
  } finally {
    clearTimeout(timer)
  }
}
