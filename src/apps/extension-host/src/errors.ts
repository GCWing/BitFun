export class ExtensionHostError extends Error {
  readonly code: number
  readonly data?: unknown

  constructor(code: number, message: string, data?: unknown) {
    super(message)
    this.name = "ExtensionHostError"
    this.code = code
    this.data = data
  }
}

export type SerializedError = {
  name?: string
  message: string
  stack?: string
  cause?: SerializedError | string
}

export function errorData(error: unknown): SerializedError {
  if (!(error instanceof Error)) return { message: String(error) }
  return {
    name: error.name,
    message: error.message,
    stack: error.stack,
    ...(error.cause === undefined
      ? {}
      : { cause: error.cause instanceof Error ? errorData(error.cause) : String(error.cause) }),
  }
}
