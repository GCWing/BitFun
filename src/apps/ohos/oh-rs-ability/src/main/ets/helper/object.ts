export function objectAssign(
  target: Record<string, Object>,
  ...source: Object[]
): Record<string, Object> {
  for (const items of source) {
    for (const key of Object.getOwnPropertyNames(Object.getPrototypeOf(items))) {
      target[key] = Reflect.get(items, key);
    }
  }
  return target;
}
