export function isSessionRowPointerTarget(
  row: HTMLElement,
  target: EventTarget | null,
): target is Node {
  return target instanceof Node && row.contains(target);
}
