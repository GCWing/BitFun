import { describe, expect, test } from "bun:test"
import { requireLoopbackAddress } from "../src/loopback"

describe("RPC loopback validation", () => {
  test.each(["127.0.0.1:1234", "localhost:1234", "[::1]:1234"])("accepts %s", (address) => {
    expect(() => requireLoopbackAddress(address)).not.toThrow()
  })

  test.each(["127.0.0.2:1234", "127.evil:1234", "192.168.1.5:1234"])("rejects %s", (address) => {
    expect(() => requireLoopbackAddress(address)).toThrow("must be loopback")
  })
})
