import { isMcpToolName } from '@/infrastructure/mcp/toolName';
import { isUserSelectableToolName } from '@/shared/utils/toolVisibility';

export function isAgentProfileConfigurableToolName(toolName: string): boolean {
  return isUserSelectableToolName(toolName) && !isMcpToolName(toolName);
}
