import { isIP } from "node:net"
import { parseRpcAddress } from "./rpc"

export function requireLoopbackAddress(address: string) {
  const hostname = parseRpcAddress(address).hostname.toLowerCase()
  const normalized = hostname.replace(/^\[|\]$/g, "")
  if (normalized === "localhost" || normalized === "127.0.0.1" || (isIP(normalized) === 6 && normalized === "::1")) return
  throw new Error(`OPENCODE_EXTENSION_HOST_RPC_ADDRESS must be loopback, received ${hostname}`)
}
