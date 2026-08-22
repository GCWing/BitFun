export const LOG_LEVELS = ["trace", "debug", "info", "warn", "error", "off"] as const
export type LogLevel = (typeof LOG_LEVELS)[number]

const DEFAULT_LOG_LEVEL: LogLevel = "debug"
const LOG_LEVEL_RANK: Readonly<Record<LogLevel, number>> = {
  trace: 0,
  debug: 1,
  info: 2,
  warn: 3,
  error: 4,
  off: 5,
}

let currentLogLevel = parseLogLevel(process.env.OPENCODE_EXTENSION_HOST_LOG_LEVEL) ?? DEFAULT_LOG_LEVEL

export function setLogLevel(level: string): LogLevel {
  const parsed = parseLogLevel(level)
  if (!parsed) throw new TypeError(`Invalid extension host log level: ${level}`)
  currentLogLevel = parsed
  return currentLogLevel
}

export function getLogLevel(): LogLevel {
  return currentLogLevel
}

export function logEvent(event: string, fields: Record<string, unknown> = {}, level: LogLevel = "info") {
  if (!shouldLog(level)) return
  const record = {
    timestamp: new Date().toISOString(),
    level,
    event,
    ...fields,
  }
  console.error(`[extension-host] ${JSON.stringify(record)}`)
}

function parseLogLevel(value: string | undefined): LogLevel | undefined {
  if (!value) return undefined
  return LOG_LEVELS.find((candidate) => candidate === value.trim().toLowerCase())
}

function shouldLog(level: LogLevel): boolean {
  return currentLogLevel !== "off" && LOG_LEVEL_RANK[level] >= LOG_LEVEL_RANK[currentLogLevel]
}

export function logError(event: string, error: unknown, fields: Record<string, unknown> = {}) {
  const value = error instanceof Error ? error : new Error(String(error))
  logEvent(
    event,
    {
      ...fields,
      error_name: value.name,
      error_message: value.message,
    },
    "error",
  )
}

export function rpcMessageSummary(message: unknown) {
  if (!message || typeof message !== "object") return { kind: "invalid" }
  const value = message as Record<string, unknown>
  const id = typeof value.id === "string" ? { request_id: value.id } : {}
  if (typeof value.method === "string") {
    return {
      ...id,
      kind: value.id === undefined ? "notification" : "request",
      method: value.method,
    }
  }
  return {
    ...id,
    kind: "error" in value ? "error_response" : "response",
  }
}
