import {
  ChatMessage,
  RemoteToolStatusResponse,
  SessionSummary
} from '../model/RemoteModels';

/** Transport-neutral events consumed by the shared conversation store. */
export interface ConversationUserMessageEvent {
  type: 'user_message';
  message: ChatMessage;
  persisted: boolean;
}

export interface ConversationAssistantDeltaEvent {
  type: 'assistant_delta';
  sessionId: string;
  turnId: string;
  delta: string;
}

export interface ConversationAssistantMessageEvent {
  type: 'assistant_message';
  message: ChatMessage;
}

export interface ConversationActiveTurnUpdatedEvent {
  type: 'active_turn_updated';
  sessionId: string;
  message: ChatMessage;
}

export interface ConversationToolEvent {
  type: 'tool_started' | 'tool_finished';
  sessionId: string;
  turnId: string;
  tool: RemoteToolStatusResponse;
}

export interface ConversationTurnStartedEvent {
  type: 'turn_started';
  sessionId: string;
  turnId: string;
}

export interface ConversationTurnFinishedEvent {
  type: 'turn_finished';
  sessionId: string;
  turnId: string;
  message?: ChatMessage;
}

export interface ConversationSessionUpdatedEvent {
  type: 'session_updated';
  session: SessionSummary;
}

export interface ConversationErrorEvent {
  type: 'error';
  message: string;
}

export type ConversationEvent =
  | ConversationUserMessageEvent
  | ConversationAssistantDeltaEvent
  | ConversationAssistantMessageEvent
  | ConversationActiveTurnUpdatedEvent
  | ConversationToolEvent
  | ConversationTurnStartedEvent
  | ConversationTurnFinishedEvent
  | ConversationSessionUpdatedEvent
  | ConversationErrorEvent;
