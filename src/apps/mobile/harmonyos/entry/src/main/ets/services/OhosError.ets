/**
 * Converts SDK `BusinessError` values into `Error` so ArkTS sees the throw as
 * handled at the call site, while callers still get a real failure.
 */
export class OhosError {
  static wrap(operation: string, err: Object): Error {
    if (err instanceof Error) {
      return err;
    }
    return new Error(`${operation} failed: ${OhosError.reason(err)}`);
  }

  static reason(err: Object): string {
    if (err instanceof Error && err.message.length > 0) {
      return err.message;
    }
    try {
      const text = JSON.stringify(err);
      return text.length > 0 && text !== '{}' ? text : 'unknown error';
    } catch (_jsonErr) {
      return `${err}`;
    }
  }
}
