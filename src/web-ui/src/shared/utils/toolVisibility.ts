const INTERNAL_GATEWAY_TOOL_NAMES: ReadonlySet<string> = new Set([
  'GetToolSpec',
  'CallDeferredTool',
]);

export function isUserSelectableToolName(toolName: string): boolean {
  return !INTERNAL_GATEWAY_TOOL_NAMES.has(toolName);
}
