import { RemoteToolStatusResponse } from '../../model/RemoteModels';
import { GeneralChatApiTool, GeneralChatStreamEvent } from './GeneralChatApiModels';

export interface GeneralChatProjectionItem {
  id: string;
  type: string;
  content: string;
  tool?: RemoteToolStatusResponse;
}

export interface GeneralChatTurnProjection {
  sessionId: string;
  turnId: string;
  version: number;
  text: string;
  thinking: string;
  status: string;
  errorCode: string;
  capabilityRequired: string;
  tools: RemoteToolStatusResponse[];
  items: GeneralChatProjectionItem[];
}

export class GeneralChatEventMapper {
  static emptyProjection(sessionId: string, turnId: string, version: number = 0): GeneralChatTurnProjection {
    return {
      sessionId,
      turnId,
      version,
      text: '',
      thinking: '',
      status: 'starting',
      errorCode: '',
      capabilityRequired: '',
      tools: [],
      items: []
    };
  }

  static apply(current: GeneralChatTurnProjection, event: GeneralChatStreamEvent): GeneralChatTurnProjection {
    if (event.session_id !== current.sessionId || event.turn_id !== current.turnId) {
      throw new Error('General chat event does not belong to the active turn.');
    }
    if (event.version <= current.version || GeneralChatEventMapper.isTerminal(current.status)) {
      return current;
    }
    if (event.version !== current.version + 1) {
      throw new Error(`General chat event version gap: expected ${current.version + 1}, received ${event.version}.`);
    }

    const next: GeneralChatTurnProjection = {
      sessionId: current.sessionId,
      turnId: current.turnId,
      version: event.version,
      text: current.text,
      thinking: current.thinking,
      status: current.status,
      errorCode: current.errorCode,
      capabilityRequired: current.capabilityRequired,
      tools: current.tools.slice(),
      items: current.items.slice()
    };
    if (event.type === 'turn.started') {
      next.status = 'starting';
    } else if (event.type === 'message.delta') {
      const delta = event.delta || '';
      next.text += delta;
      next.items = GeneralChatEventMapper.appendTextItem(next.items, event.event_id, 'text', delta);
      next.status = 'streaming';
    } else if (event.type === 'thinking.delta') {
      const delta = event.delta || '';
      next.thinking += delta;
      next.items = GeneralChatEventMapper.appendTextItem(next.items, event.event_id, 'thinking', delta);
      next.status = 'thinking';
    } else if (GeneralChatEventMapper.isToolEvent(event.type)) {
      if (event.tool) {
        const mapped = GeneralChatEventMapper.mapTool(event.tool, event.type);
        next.tools = GeneralChatEventMapper.upsertTool(next.tools, mapped);
        next.items = GeneralChatEventMapper.upsertToolItem(next.items, mapped, event.event_id);
      }
      next.status = 'tool_calling';
    } else if (event.type === 'permission.required') {
      if (event.tool) {
        const mapped = GeneralChatEventMapper.mapTool(event.tool, event.type);
        next.tools = GeneralChatEventMapper.upsertTool(next.tools, mapped);
        next.items = GeneralChatEventMapper.upsertToolItem(next.items, mapped, event.event_id);
      }
      next.status = 'pending_confirmation';
    } else if (event.type === 'capability.required') {
      next.capabilityRequired = event.capability || 'unknown';
      next.status = 'capability_required';
    } else if (event.type === 'turn.finishing') {
      next.status = 'finishing';
    } else if (event.type === 'turn.completed') {
      next.status = 'completed';
    } else if (event.type === 'turn.cancelled') {
      next.status = 'cancelled';
    } else if (event.type === 'turn.failed') {
      next.status = 'failed';
      next.errorCode = event.error_code || 'unknown';
    }
    return next;
  }

  private static appendTextItem(
    items: GeneralChatProjectionItem[],
    eventId: string,
    type: string,
    delta: string
  ): GeneralChatProjectionItem[] {
    if (delta.length === 0) {
      return items;
    }
    const next = items.slice();
    const lastIndex = next.length - 1;
    const last = lastIndex >= 0 ? next[lastIndex] : undefined;
    if (last && last.type === type && !last.tool) {
      next[lastIndex] = {
        id: last.id,
        type: last.type,
        content: last.content + delta
      };
      return next;
    }
    next.push({
      id: eventId,
      type,
      content: delta
    });
    return next;
  }

  private static upsertToolItem(
    items: GeneralChatProjectionItem[],
    incoming: RemoteToolStatusResponse,
    eventId: string
  ): GeneralChatProjectionItem[] {
    const toolId = incoming.id || '';
    const index = items.findIndex((item: GeneralChatProjectionItem) => {
      return !!item.tool && (item.tool?.id || '') === toolId;
    });
    const mapped: GeneralChatProjectionItem = {
      id: toolId.length > 0 ? toolId : eventId,
      type: 'tool',
      content: '',
      tool: incoming
    };
    if (index < 0) {
      return items.concat([mapped]);
    }
    const next = items.slice();
    next[index] = mapped;
    return next;
  }

  private static upsertTool(
    tools: RemoteToolStatusResponse[],
    incoming: RemoteToolStatusResponse
  ): RemoteToolStatusResponse[] {
    const incomingId = incoming.id || '';
    const index = tools.findIndex((tool: RemoteToolStatusResponse) => (tool.id || '') === incomingId);
    if (index < 0) {
      return tools.concat([incoming]);
    }
    const next = tools.slice();
    next[index] = incoming;
    return next;
  }

  private static mapTool(incoming: GeneralChatApiTool, eventType: string): RemoteToolStatusResponse {
    let status = incoming.status;
    if (eventType === 'permission.required') {
      status = 'pending_confirmation';
    } else if (status.length === 0 && eventType === 'tool.started') {
      status = 'preparing';
    } else if (status.length === 0 && eventType === 'tool.completed') {
      status = 'completed';
    }
    return {
      id: incoming.id,
      name: incoming.name,
      status,
      input_preview: incoming.input_preview,
      result_preview: incoming.result_preview,
      error_preview: incoming.error_preview
    };
  }

  private static isToolEvent(type: string): boolean {
    return type === 'tool.started' || type === 'tool.updated' || type === 'tool.completed';
  }

  private static isTerminal(status: string): boolean {
    return status === 'completed' || status === 'cancelled' || status === 'failed';
  }
}
