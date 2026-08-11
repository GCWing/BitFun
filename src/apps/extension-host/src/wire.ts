export type WireValue = null | boolean | number | string | WireValue[] | { [key: string]: WireValue }

export class WireValueError extends TypeError {
  readonly path: string

  constructor(path: string, message: string) {
    super(`Wire value at ${path} ${message}`)
    this.name = "WireValueError"
    this.path = path
  }
}

/**
 * Validate and detach a value before it crosses the RPC boundary. This is
 * deliberately stricter than JSON.stringify, which otherwise drops functions
 * and undefined values silently and converts non-finite numbers to null.
 */
export function cloneWireValue(value: unknown, path = "$"): WireValue {
  return clone(value, path, new Map())
}

export function assertWireValue(value: unknown, path = "$"): asserts value is WireValue {
  cloneWireValue(value, path)
}

export function isWireValue(value: unknown): value is WireValue {
  try {
    cloneWireValue(value)
    return true
  } catch {
    return false
  }
}

function clone(value: unknown, path: string, ancestors: Map<object, string>): WireValue {
  if (value === null || typeof value === "string" || typeof value === "boolean") return value
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new WireValueError(path, "cannot contain a non-finite number")
    return value
  }
  if (typeof value === "function") throw new WireValueError(path, "cannot contain a function")
  if (typeof value === "bigint") throw new WireValueError(path, "cannot contain a BigInt")
  if (typeof value === "undefined") throw new WireValueError(path, "cannot contain undefined")
  if (typeof value === "symbol") throw new WireValueError(path, "cannot contain a symbol")

  if (typeof value !== "object") throw new WireValueError(path, `has unsupported type ${typeof value}`)
  const previous = ancestors.get(value)
  if (previous !== undefined) throw new WireValueError(path, `contains a cycle referencing ${previous}`)
  ancestors.set(value, path)

  try {
    if (Array.isArray(value)) {
      return Array.from(value, (item, index) =>
        item === undefined ? null : clone(item, `${path}[${index}]`, ancestors),
      )
    }

    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) {
      throw new WireValueError(path, `must be a plain object, received ${objectName(value)}`)
    }

    return Object.fromEntries(
      Object.entries(value)
        .filter((entry) => entry[1] !== undefined)
        .map(([key, item]) => [key, clone(item, propertyPath(path, key), ancestors)]),
    )
  } finally {
    ancestors.delete(value)
  }
}

function propertyPath(parent: string, key: string) {
  return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(key) ? `${parent}.${key}` : `${parent}[${JSON.stringify(key)}]`
}

function objectName(value: object) {
  return Object.prototype.toString.call(value)
}
