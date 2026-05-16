import type { FlowToolItem } from '../types/flow-chat';

export function hasAcpPermissionOptions(toolItem: FlowToolItem): boolean {
  return Boolean(toolItem.acpPermission?.options?.length);
}
