const warnedProps = new Set<string>();

export function canvasArrayProp<T>(
  component: string,
  prop: string,
  value: readonly T[] | null | undefined | unknown,
): readonly T[] {
  if (value === undefined || value === null) return [];
  if (Array.isArray(value)) return value as readonly T[];
  const warningKey = `${component}.${prop}`;
  if (!warnedProps.has(warningKey)) {
    warnedProps.add(warningKey);
    const message = `${component}.${prop} expected an array; the invalid value was ignored`;
    console.warn(message);
    window.parent?.postMessage({
      type: 'openbitfun-canvas-prop-warning',
      component,
      prop,
      message,
    }, '*');
  }
  return [];
}
