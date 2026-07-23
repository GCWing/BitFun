/**
 * Decode minified React production errors for support logs (desktop webview.log).
 */

const REACT_MINIFIED = /Minified React error #(\d+)/;

const REACT_HINTS: Record<string, string> = {
  '300':
    'Rendered fewer hooks than expected; often an early return before hooks, or conditional hooks.',
  '301': 'Too many re-renders; possible infinite setState/useEffect loop.',
  '310': 'Rendered more hooks than during the previous render; conditional hooks or hook order change.',
};

export function parseMinifiedReactError(message: string): {
  code: string;
  decoderUrl: string;
  hint?: string;
} | null {
  const m = message.match(REACT_MINIFIED);
  if (!m?.[1]) return null;
  const code = m[1];
  return {
    code,
    decoderUrl: `https://react.dev/errors/${code}`,
    hint: REACT_HINTS[code],
  };
}

export function isMinifiedReactErrorMessage(message: string): boolean {
  return REACT_MINIFIED.test(message);
}

/** Only keep serializable fields from React's errorInfo. */
export function safeReactErrorInfo(info: unknown): { componentStack?: string } {
  if (!info || typeof info !== 'object') return {};
  const cs = (info as { componentStack?: unknown }).componentStack;
  if (typeof cs === 'string') return { componentStack: cs };
  return {};
}

export function buildReactCrashLogPayload(error: Error, errorInfo?: unknown): Record<string, unknown> {
  const message = error.message ?? '';
  const diag = parseMinifiedReactError(message);
  return {
    error: {
      name: error.name,
      message,
      stack: error.stack,
    },
    ...safeReactErrorInfo(errorInfo),
    ...(diag
      ? {
          reactInvariant: diag.code,
          reactDecoderUrl: diag.decoderUrl,
          ...(diag.hint ? { reactHint: diag.hint } : {}),
        }
      : {}),
  };
}
