import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import type { ToolInfo } from '@/shared/types/agent-api';

const toolInfoCache = new Map<string, Promise<ToolInfo | null>>();

export function getCachedToolInfo(toolName: string): Promise<ToolInfo | null> {
  const cached = toolInfoCache.get(toolName);
  if (cached) {
    return cached;
  }

  const request = toolAPI.getToolInfo(toolName).catch((error) => {
    toolInfoCache.delete(toolName);
    throw error;
  });

  toolInfoCache.set(toolName, request);
  return request;
}
