import type { FlowToolItem } from '../types/flow-chat';

export const DEFERRED_TOOL_GATEWAY_NAME = 'CallDeferredTool';

export interface EffectiveToolInvocation {
  toolName: string;
  input: unknown;
  isDeferred: boolean;
}

export function effectiveToolInvocation(
  wireToolName: string,
  wireInput: unknown,
): EffectiveToolInvocation {
  if (
    wireToolName !== DEFERRED_TOOL_GATEWAY_NAME
    || wireInput === null
    || typeof wireInput !== 'object'
    || Array.isArray(wireInput)
  ) {
    return { toolName: wireToolName, input: wireInput, isDeferred: false };
  }

  const input = wireInput as Record<string, unknown>;
  const keys = Object.keys(input);
  if (
    keys.some(key => key !== 'tool_name' && key !== 'args')
    || typeof input.tool_name !== 'string'
    || input.tool_name.trim().length === 0
    || !Object.prototype.hasOwnProperty.call(input, 'args')
    || input.args === null
    || typeof input.args !== 'object'
    || Array.isArray(input.args)
  ) {
    return { toolName: wireToolName, input: wireInput, isDeferred: false };
  }

  return {
    toolName: input.tool_name,
    input: input.args,
    isDeferred: true,
  };
}

export function getEffectiveToolName(toolItem: Pick<FlowToolItem, 'toolName' | 'toolCall'>): string {
  return effectiveToolInvocation(toolItem.toolName, toolItem.toolCall?.input).toolName;
}

export function projectEffectiveToolItem(toolItem: FlowToolItem): FlowToolItem {
  const effective = effectiveToolInvocation(toolItem.toolName, toolItem.toolCall?.input);
  if (!effective.isDeferred) {
    return toolItem;
  }

  return {
    ...toolItem,
    toolName: effective.toolName,
    toolCall: {
      ...toolItem.toolCall,
      input: effective.input,
    },
  };
}
