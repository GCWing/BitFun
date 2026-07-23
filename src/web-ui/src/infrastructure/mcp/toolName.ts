export const MCP_TOOL_PREFIX = 'mcp__';

export interface ParsedMcpToolName {
  serverId: string;
  toolName: string;
}

export function isMcpToolName(name: string): boolean {
  return name.startsWith(MCP_TOOL_PREFIX);
}

export function parseMcpToolName(name: string): ParsedMcpToolName | null {
  if (!isMcpToolName(name)) {
    return null;
  }

  const rest = name.slice(MCP_TOOL_PREFIX.length);
  const delimiterIndex = rest.indexOf('__');
  if (delimiterIndex <= 0 || delimiterIndex >= rest.length - 2) {
    return null;
  }

  return {
    serverId: rest.slice(0, delimiterIndex),
    toolName: rest.slice(delimiterIndex + 2),
  };
}
