/** Mirrors `FILE_TOOL_GUIDANCE_PREFIX` in file_tool_guidance.rs */
export const FILE_TOOL_GUIDANCE_PREFIX = '[guidance] ';

export function isFileToolGuidanceMessage(message: unknown): boolean {
  return typeof message === 'string' && message.startsWith(FILE_TOOL_GUIDANCE_PREFIX);
}

export function displayFileToolGuidanceMessage(message: unknown): string {
  if (typeof message !== 'string') {
    return '';
  }
  return message.startsWith(FILE_TOOL_GUIDANCE_PREFIX)
    ? message.slice(FILE_TOOL_GUIDANCE_PREFIX.length)
    : message;
}
